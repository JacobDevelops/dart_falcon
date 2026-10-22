//! Flags a `final` or `const` variable given a nullable type but initialized to a
//! provably non-null value.
//!
//! A `final int? x = 3;` can never hold null: it is initialized once, at the
//! declaration, to a value that is obviously non-null. The `?` widens the static
//! type for no reason, forcing needless null checks at every use. The rule is
//! deliberately conservative about "provably non-null" — only literals (numbers,
//! strings, booleans, collections, records), constructor invocations, and function
//! expressions qualify; anything whose value flows from another binding is left
//! alone. It applies to final/const top-level variables, static fields, and local
//! variables. Drop the `?` from the declared type.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryNullableForFinalVariableDeclarations;

impl Rule for UnnecessaryNullableForFinalVariableDeclarations {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

use crate::lint::expressions::is_provably_non_null;

impl<'a> Collector<'a> {
    fn check(
        &mut self,
        keyword: VarKeyword,
        var_type: Option<DartType<'a>>,
        declarators: AstChildren<'a, VarDeclarator<'a>>,
    ) {
        if keyword == VarKeyword::Var {
            return;
        }
        let Some(t) = var_type else { return };
        if !t.is_nullable() {
            return;
        }
        // The type is shared by every declarator, so it is only unnecessarily
        // nullable when *all* of them are provably non-null.
        if !declarators.clone().all(|d| {
            d.initializer().is_some_and(|expr| {
                is_provably_non_null(expr) || matches!(expr.unparenthesized(), Expr::FuncExpr(_))
            })
        }) {
            return;
        }
        for d in declarators {
            self.diags.push(self.ctx.diagnostic("Remove the unnecessary nullable type from this final variable declaration.", d.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Variable(v) = node {
            self.check(v.keyword(), v.var_type(), v.declarators());
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        if node.is_static() {
            self.check(node.keyword(), node.field_type(), node.declarators());
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_local_var(&mut self, lv: LocalVarDecl<'a>) -> StructuralTraversal {
        self.check(lv.keyword(), lv.var_type(), lv.declarators());
        StructuralTraversal::Continue
    }
}
}
