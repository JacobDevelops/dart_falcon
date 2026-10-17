//! Flags `return` statements that are not preceded by a blank line.
//!
//! Separating a `return` from the statements above it with a blank line makes
//! the exit point of a block visually distinct, so readers can spot where
//! control leaves the function without parsing the surrounding code. The rule
//! only checks returns that follow another statement in the same block.
//! Switch-case statement lists are not blocks. A blank line is any run of
//! whitespace containing two or more newlines; spaces and tabs do not matter.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{StatementListKind, body_list, member_list, stmt_lists};

#[derive(Default)]
pub struct NewlineBeforeReturn;

impl Rule for NewlineBeforeReturn {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, stmts: &[Stmt], kind: StatementListKind) {
        if kind == StatementListKind::Block {
            check_stmts_for_return(stmts, &mut self.diags, self.ctx);
        }
    }
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        body_list(node.body.as_ref(), &mut |stmts, kind| self.check(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        member_list(node, &mut |stmts, kind| self.check(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        stmt_lists(node, &mut |stmts, kind| self.check(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::FuncExpr { body, .. } = node {
            body_list(Some(body), &mut |stmts, kind| self.check(stmts, kind));
        }
        StructuralTraversal::Continue
    }
}
}

fn has_blank_line_before(source: &str, pos: usize) -> bool {
    // Scan backwards from pos looking for two consecutive newlines (blank line).
    // Skips spaces and tabs between newlines.
    let bytes = &source.as_bytes()[..pos.min(source.len())];
    let mut i = bytes.len();
    let mut newlines = 0usize;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b'\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            b' ' | b'\t' | b'\r' => {}
            _ => break,
        }
    }
    false
}

fn check_stmts_for_return(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in stmts.iter().skip(1) {
        if let Stmt::Return(ret) = stmt
            && !has_blank_line_before(ctx.source, ret.span.start)
        {
            diags.push(ctx.diagnostic(
                "newline-before-return",
                "Add a blank line before return statement",
                ret.span,
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn preserves_statement_list_diagnostic_order() {
        for (source, expected) in [
            (
                "int f(bool c) { try { if (c) { ping(); return 1; } ping(); return 2; } catch (_) { ping(); return 3; } finally { ping(); return 4; } }",
                vec!["return 2;", "return 3;", "return 4;", "return 1;"],
            ),
            (
                "class C { final int value; C() : value = (() { ping(); return 5; })() { ping(); return; } }",
                vec!["return;", "return 5;"],
            ),
        ] {
            let parsed = falcon_dart_parser::parse(source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            let config = FalconConfig::default();
            let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
            let diagnostics = NewlineBeforeReturn.analyze(&parsed.program, &ctx);
            let actual: Vec<_> = diagnostics
                .iter()
                .map(|diagnostic| diagnostic.span.start)
                .collect();
            let expected: Vec<_> = expected
                .iter()
                .map(|text| source.find(text).unwrap())
                .collect();
            assert_eq!(actual, expected, "{source}");
        }
    }
}
