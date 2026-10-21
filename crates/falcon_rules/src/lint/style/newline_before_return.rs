//! Flags `return` statements that are not preceded by a blank line.
//!
//! Separating a `return` from the statements above it with a blank line makes
//! the exit point of a block visually distinct, so readers can spot where
//! control leaves the function without parsing the surrounding code. The rule
//! only checks returns that follow another statement in the same block.
//! Switch-case statement lists are not blocks. A blank line contains two or more
//! newlines and may appear above attached comments; spaces and tabs do not matter.

use super::newline_spacing::has_blank_line_before;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{StatementListKind, body_list, member_list, stmt_lists};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct NewlineBeforeReturn;

impl Rule for NewlineBeforeReturn {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
    fn check(&mut self, stmts: AstChildren<'_, Stmt<'_>>, kind: StatementListKind) {
        if kind == StatementListKind::Block {
            check_stmts_for_return(stmts, &mut self.diags, self.ctx);
        }
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        body_list(node.body(), &mut |stmts, kind| self.check(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        member_list(node, &mut |stmts, kind| self.check(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        stmt_lists(node, &mut |stmts, kind| self.check(stmts, kind));
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            body_list(Some(function.body()), &mut |stmts, kind| self.check(stmts, kind));
        }
        StructuralTraversal::Continue
    }
}
}

fn check_stmts_for_return(
    stmts: AstChildren<'_, Stmt<'_>>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    for stmt in stmts.skip(1) {
        if let Stmt::Return(ret) = stmt
            && !has_blank_line_before(ret.syntax())
        {
            diags.push(ctx.diagnostic("Add a blank line before return statement", ret.span()));
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
            let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
                source: source.into(),
                tree: parsed.tree,
            });
            let program = parsed.program();
            let config = FalconConfig::default();
            let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
            let diagnostics = NewlineBeforeReturn.analyze(NewlineBeforeReturn::NAME, program, &ctx);
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
