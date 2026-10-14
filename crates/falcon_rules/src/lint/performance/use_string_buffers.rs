//! Prefers a `StringBuffer` to growing a `String` with `+` inside a loop.
//!
//! Strings are immutable, so `s += piece` in a loop copies the whole string on
//! every iteration; a `StringBuffer` appends in amortized constant time. The rule
//! inspects the statements directly in the body of a `for`, `while` or
//! `do`-`while` loop (and plain nested blocks). It reports `s += ...` when `s` is
//! a `String` variable declared outside that body, and reports `s` in
//! `s = s + ...` or `s = '$s...'`, where the right-hand side starts with `s`.
//!
//! Adopted from the official `use_string_buffers` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UseStringBuffers;

impl Rule for UseStringBuffers {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// Whether `expr` begins with the identifier `name`: the leftmost operand of a
/// `+` chain, or the first interpolation of a string that starts with it.
fn starts_with_identifier(expr: &Expr, name: &str, source: &str) -> bool {
    let mut current = expr;
    loop {
        match current {
            Expr::Binary {
                op: BinaryOp::Add,
                left,
                ..
            } => current = left,
            Expr::Ident(identifier) => return identifier.name == name,
            Expr::StringLit(literal) => {
                let Some(delimiter) = literal.delimiter() else {
                    return false;
                };
                let Some(first) = literal.interpolations.first() else {
                    return false;
                };
                let content_start = literal.span.start + delimiter.content_offset;
                // The interpolation must open the string: `$s...` or `${s}...`.
                let prefix = source.get(content_start..first.span.start);
                if !matches!(prefix, Some("$" | "${")) {
                    return false;
                }
                current = &first.expr;
            }
            _ => return false,
        }
    }
}

impl Collector<'_> {
    fn check_loop_body(&mut self, body: &Stmt, state: &SemanticState<'_>) {
        // Names declared directly in the loop body, with whether each is known
        // to be a `String`.
        let mut locals = HashMap::new();
        let mut pending = vec![body];
        while let Some(statement) = pending.pop() {
            match statement {
                Stmt::Block(block) => pending.extend(block.stmts.iter().rev()),
                Stmt::LocalVar(declaration) => {
                    let declared_string = declaration.var_type.as_ref().map(|ty| {
                        matches!(state.model.resolve_type(ty), ResolvedType::Interface { identity, nullable: false, .. }
                            if identity.is_sdk("dart:core", "String"))
                    });
                    locals.extend(declaration.declarators.iter().map(|declarator| {
                        let is_string = declared_string
                            .unwrap_or(matches!(declarator.initializer, Some(Expr::StringLit(_))));
                        (declarator.name.name.as_str(), is_string)
                    }));
                }
                Stmt::Expr(statement) => self.check_assignment(&statement.expr, &locals, state),
                _ => {}
            }
        }
    }

    fn check_assignment(
        &mut self,
        expr: &Expr,
        locals: &HashMap<&str, bool>,
        state: &SemanticState<'_>,
    ) {
        let Expr::Assign {
            target,
            op,
            value,
            span,
        } = expr
        else {
            return;
        };
        let Expr::Ident(identifier) = target.as_ref() else {
            return;
        };
        if !matches!(op, AssignOp::PlusEq | AssignOp::Eq) {
            return;
        }
        let local = locals.get(identifier.name.as_str()).copied();
        let is_string =
            local.unwrap_or_else(|| state.infer(target).interface("dart:core", "String"));
        if !is_string {
            return;
        }
        match op {
            AssignOp::PlusEq if local.is_none() => {
                self.report(*span);
            }
            AssignOp::Eq if starts_with_identifier(value, &identifier.name, self.ctx.source) => {
                self.report(identifier.span);
            }
            _ => {}
        }
    }

    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(
            "use-string-buffers",
            "Use a string buffer rather than '+' to compose strings.",
            span,
        ));
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let body = match node {
            Stmt::For(statement) => &statement.body,
            Stmt::While(statement) => &statement.body,
            Stmt::DoWhile(statement) => &statement.body,
            _ => return,
        };
        self.check_loop_body(body, state);
    }
}
