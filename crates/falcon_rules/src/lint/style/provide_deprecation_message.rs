//! Flags `@deprecated` and `@Deprecated()` annotations that carry no message.
//!
//! A deprecation is only useful if it tells callers what to do instead — which
//! replacement to adopt, or since when the API is going away. The bare
//! `@deprecated` constant can never carry that guidance, and `@Deprecated()`
//! with no argument (or an empty/whitespace-only string) is just as unhelpful,
//! so both should be replaced with `@Deprecated("message")`. A `@Deprecated`
//! whose positional argument is any non-empty expression is accepted. The rule
//! also inspects field-level annotations, which the generic syntax-tree walk would
//! otherwise skip.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ProvideDeprecationMessage;

impl Rule for ProvideDeprecationMessage {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: falcon_analyze::SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: falcon_analyze::SemanticModel<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_annotation(&mut self, node: Annotation<'a>) -> StructuralTraversal {
        let model = self.model;
        let flagged = if crate::lint::support::annotations::is_deprecated_constant(node, model) {
            true
        } else if crate::lint::support::annotations::is_deprecated_constructor(node, model) {
            match node.args() {
                None => true,
                Some(args) => match args.positional().first().map(Expr::unparenthesized) {
                    None => true,
                    Some(Expr::StringLit(s)) => s.value().trim().is_empty(),
                    Some(_) => false,
                },
            }
        } else {
            false
        };
        if flagged {
            self.diags.push(self.ctx.diagnostic(
                "Provide a deprecation message via `@Deprecated(\"message\")`.",
                node.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }
}
}
