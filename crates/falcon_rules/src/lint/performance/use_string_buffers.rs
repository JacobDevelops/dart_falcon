//! Prefers a `StringBuffer` to growing a `String` with `+` inside a loop.
//!
//! Strings are immutable, so `s += piece` in a loop copies the whole string on
//! every iteration; a `StringBuffer` appends in amortized constant time. The rule
//! inspects the statements directly in the body of a `for`, `while` or
//! `do`-`while` loop (and plain nested blocks). It reports `s += ...` when `s` is
//! a `String` variable declared outside that body, and reports `s` in
//! `s = s + ...` or `s = '$s...'` (parentheses aside), where the right-hand
//! side starts with `s` and `s` is a local variable or parameter: for a field
//! or top-level variable the analyzer compares the setter written with the
//! getter read, which never match.
//!
//! Adopted from the official `use_string_buffers` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UseStringBuffers;

impl Rule for UseStringBuffers {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

/// Whether `expr` begins with the identifier `name`: the leftmost operand of a
/// `+` chain, or the first interpolation of a string that starts with it.
fn starts_with_identifier(expr: Expr<'_>, name: &str) -> bool {
    let mut current = expr;
    loop {
        match current.unparenthesized() {
            Expr::Binary(binary) if binary.op() == BinaryOp::Add => current = binary.left(),
            Expr::Ident(identifier) => return identifier.name() == name,
            Expr::StringLit(literal) => {
                let Some(delimiter) = literal.delimiter() else {
                    return false;
                };
                let Some(first) = literal.interpolations().next() else {
                    return false;
                };
                let content_start = literal.span().start + delimiter.content_offset;
                let opens = first.span().start == content_start + 1
                    || (first.span().start == content_start + 2
                        && first.syntax().first_token().is_some_and(|token| {
                            token.kind() == falcon_syntax::TokenKind::InterpolationStart
                        }));
                if !opens {
                    return false;
                }
                current = first.expr();
            }
            _ => return false,
        }
    }
}

impl<'a> Collector<'a> {
    fn check_loop_body(&mut self, body: Stmt<'a>, state: &SemanticState<'a>) {
        // Names declared directly in the loop body, with whether each is known
        // to be a `String`.
        let mut locals = HashMap::new();
        let mut pending = vec![body];
        while let Some(statement) = pending.pop() {
            match statement {
                Stmt::Block(block) => pending.extend(block.stmts().rev()),
                Stmt::LocalVar(declaration) => {
                    let declared_string = declaration.var_type().map(|ty| {
                        matches!(state.model.resolve_type(ty), ResolvedType::Interface { identity, nullable: false, .. }
                            if identity.is_sdk("dart:core", "String"))
                    });
                    locals.extend(declaration.declarators().map(|declarator| {
                        let is_string = declared_string.unwrap_or(matches!(
                            declarator.initializer().map(Expr::unparenthesized),
                            Some(Expr::StringLit(_))
                        ));
                        (declarator.name().name(), is_string)
                    }));
                }
                Stmt::Expr(statement) => self.check_assignment(statement.expr(), &locals, state),
                _ => {}
            }
        }
    }

    fn check_assignment(
        &mut self,
        expr: Expr<'a>,
        locals: &HashMap<&str, bool>,
        state: &SemanticState<'a>,
    ) {
        let Expr::Assign(assignment) = expr.unparenthesized() else {
            return;
        };
        let target = assignment.target();
        let op = assignment.op();
        let value = assignment.value();
        let span = assignment.span();
        let Expr::Ident(identifier) = target.unparenthesized() else {
            return;
        };
        if !matches!(op, AssignOp::PlusEq | AssignOp::Eq) {
            return;
        }
        let local = locals.get(identifier.name()).copied();
        let is_string =
            local.unwrap_or_else(|| state.infer(target).interface("dart:core", "String"));
        if !is_string {
            return;
        }
        match op {
            AssignOp::PlusEq if local.is_none() => {
                self.report(span);
            }
            AssignOp::Eq
                if (local.is_some()
                    // Fields are bound in the root scope; locals and parameters below it.
                    || state
                        .environment
                        .binding_depth(identifier.name())
                        .is_some_and(|depth| depth > 0))
                    && starts_with_identifier(value, identifier.name()) =>
            {
                self.report(identifier.span());
            }
            _ => {}
        }
    }

    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic("Use a string buffer rather than '+' to compose strings.", span));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let body = match node {
            Stmt::For(statement) => statement.body(),
            Stmt::While(statement) => statement.body(),
            Stmt::DoWhile(statement) => statement.body(),
            _ => return,
        };
        self.check_loop_body(body, state);
    }
}

#[cfg(test)]
mod tests {
    use super::starts_with_identifier;
    use falcon_syntax::cst::TopLevelDecl;

    #[test]
    fn interpolation_must_open_the_literal() {
        for (literal, expected) in [
            ("'$s'", true),
            ("'${s}'", true),
            ("'${ (s) }'", true),
            ("'${/* comment */ s}'", true),
            ("'x$s'", false),
            ("'x${s}'", false),
            ("'${other}$s'", false),
            ("'${\"$s\"}'", true),
        ] {
            let source = format!("var value = {literal};");
            let parsed = falcon_dart_parser::parse(&source);
            let TopLevelDecl::Variable(declaration) =
                parsed.tree.program(&source).declarations().next().unwrap()
            else {
                panic!("expected variable");
            };
            let expression = declaration
                .declarators()
                .next()
                .unwrap()
                .initializer()
                .unwrap();
            assert_eq!(
                starts_with_identifier(expression, "s"),
                expected,
                "{literal}"
            );
        }
    }
}
