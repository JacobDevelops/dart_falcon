//! Flags positional field accesses on records.
//!
//! Positional getters such as `$1` and `$2` convey nothing about what each field
//! means and break silently when fields are reordered. Prefer named fields so
//! each access documents its intent. Record declarations and literals alone
//! are not reported.
//!
//! Unlike `pyramid_lint`, this also reports getters on simple identifier targets,
//! such as `record.$1`; it does not depend on the analyzer's property-access AST shape.

use falcon_analyze::{
    AnalyzeContext, ResolvedTypeView, Rule, RulePreparation, SemanticRuleVisitor, SemanticState,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidPositionalFieldsInRecords;

impl Rule for AvoidPositionalFieldsInRecords {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

const MESSAGE: &str = "Avoid positional fields in records. Use named fields instead.";

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Field(access) = node
            && access
                .field()
                .name()
                .strip_prefix('$')
                .is_some_and(|index| {
                    !index.is_empty() && index.bytes().all(|byte| byte.is_ascii_digit())
                })
            && matches!(
                state.infer(access.object()).view(),
                ResolvedTypeView::Record { .. }
            )
        {
            self.diags.push(self.ctx.diagnostic(MESSAGE, access.span()));
        }
    }
}
