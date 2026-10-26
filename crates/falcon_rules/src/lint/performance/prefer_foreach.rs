//! Prefers `forEach` to a `for`-in loop that only passes each element to a function.
//!
//! `for (final e in items) f(e);` applies `f` to every element and does nothing
//! else, which `items.forEach(f)` states directly. The rule fires on a `for`-in
//! loop that declares its loop variable and whose body (optionally a block with
//! a single statement) is one function or method call taking exactly that
//! variable as its only positional argument. A method call whose receiver
//! mentions the loop variable (`e.add(e)`) is not a candidate, and neither is a
//! constructor invocation or a call whose argument is implicitly torn off as
//! `e.call` (a callable object passed where a function is expected).
//!
//! falcon must prove the callee is a function or method rather than a
//! constructor, so calls through unresolved names are skipped; the analyzer
//! also reports calls to undefined functions.
//!
//! Adopted from the official `prefer_foreach` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, NameIdentity, QualifiedName, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr_chain};
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct PreferForeach;

impl Rule for PreferForeach {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            import_prefixes: program
                .imports()
                .filter_map(|import| import.as_name().map(|name| name.name()))
                .collect(),
            pending: HashMap::new(),
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    import_prefixes: Vec<&'a str>,
    /// Candidate loops, keyed by their call argument's start: the argument's
    /// context decides whether it is implicitly torn off.
    pending: HashMap<usize, Span>,
}

/// Whether `expr` mentions the identifier `name` anywhere.
fn mentions(expr: Expr<'_>, name: &str) -> bool {
    struct Finder<'n> {
        name: &'n str,
        found: bool,
    }
    impl<'a> Visitor<'a> for Finder<'_> {
        fn visit_expr(&mut self, node: Expr<'a>) {
            walk_expr_chain(self, node, |finder, node| {
                if let Expr::Ident(identifier) = node
                    && identifier.name() == finder.name
                {
                    finder.found = true;
                }
                true
            });
        }
    }
    let mut finder = Finder { name, found: false };
    finder.visit_expr(expr);
    finder.found
}

impl Collector<'_> {
    /// Whether an unqualified name in call position denotes a function (or a
    /// callable variable) rather than a class.
    fn is_function_name(&self, name: &str, state: &SemanticState<'_>) -> bool {
        if state.environment.is_bound(name) {
            return true;
        }
        if let Some(receiver) = &state.enclosing_type
            && state
                .signatures
                .resolved_member_facts(receiver, name, &state.model)
                .is_some()
        {
            return true;
        }
        state.model.name_identity(name) == NameIdentity::Value
            || state.model.resolve_sdk_member(name).is_some()
    }

    /// Whether `callee(arg)` is a function or method invocation, not a
    /// constructor call.
    fn is_invocation(
        &self,
        callee: Expr<'_>,
        loop_variable: &str,
        state: &SemanticState<'_>,
    ) -> bool {
        match callee.unparenthesized() {
            Expr::Ident(identifier) => self.is_function_name(identifier.name(), state),
            Expr::Field(field) => {
                let object = field.object();
                if mentions(object, loop_variable) {
                    return false;
                }
                match object.unparenthesized() {
                    Expr::Ident(prefix) if !state.environment.is_bound(prefix.name()) => {
                        if self.import_prefixes.contains(&prefix.name()) {
                            return state.model.name_identity(QualifiedName::prefixed(
                                prefix.name(),
                                field.field().name(),
                            )) == NameIdentity::Value;
                        }
                        match state.model.name_identity(prefix.name()) {
                            // `Type.member(e)`: a static method, unless it is a
                            // named constructor.
                            NameIdentity::Type => state
                                .model
                                .resolve_name(prefix.name())
                                .is_some_and(|identity| {
                                    state
                                        .signatures
                                        .member(&identity, field.field().name())
                                        .is_some()
                                        && state
                                            .signatures
                                            .constructor(&identity, field.field().name())
                                            .is_none()
                                }),
                            NameIdentity::Value => true,
                            NameIdentity::Unknown => self.is_function_name(prefix.name(), state),
                        }
                    }
                    _ => true,
                }
            }
            Expr::New(_) => false,
            _ => true,
        }
    }

    fn check(&mut self, statement: ForStmt<'_>, state: &SemanticState<'_>) {
        let Some(ForInit::ForIn(init)) = statement.header().init() else {
            return;
        };
        if !init.declares_variable() {
            return;
        }
        let name = init.name();
        let body = match statement.body() {
            Stmt::Block(block) => {
                let mut statements = block.stmts();
                match (statements.next(), statements.next()) {
                    (Some(only), None) => only,
                    _ => return,
                }
            }
            other => other,
        };
        let Stmt::Expr(expression) = body else {
            return;
        };
        let Expr::Call(call) = expression.expr().unparenthesized() else {
            return;
        };
        let mut arguments = call.args().positional();
        let (Some(argument), None) = (arguments.next(), arguments.next()) else {
            return;
        };
        let Expr::Ident(argument) = argument.unparenthesized() else {
            return;
        };
        if !call.args().named().is_empty()
            || argument.name() != name.name()
            || !self.is_invocation(call.callee(), name.name(), state)
        {
            return;
        }
        self.pending.insert(argument.span().start, statement.span());
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::For(statement) = node {
            self.check(statement, state);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Ident(argument) = node
            && let Some(span) = self.pending.remove(&argument.span().start)
            && !state.implicit_call_tearoff(node)
        {
            self.diags.push(self.ctx.diagnostic("Use `forEach` and a tear-off rather than a `for` loop to apply a function to every element.", span));
        }
    }
}
