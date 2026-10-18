//! Flags SingleChildScrollView directly containing a Flutter Column with no
//! arguments other than children. Column-specific layout settings and keys are
//! retained instead of suggesting a ListView that cannot directly preserve them.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferUsingListView;

impl Rule for PreferUsingListView {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some((identity, "new", args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities else {
            return;
        };
        if !index.is_package_declaration(&identity, "flutter", "SingleChildScrollView") {
            return;
        }
        if args
            .named()
            .any(|arg| arg.name().name() == "scrollDirection")
        {
            return;
        }
        let matches = {
            args.named()
                .find(|arg| arg.name().name() == "child")
                .and_then(|arg| {
                    crate::lint::flutter::scoped_constructor_invocation(arg.value(), state)
                })
                .is_some_and(|(child, constructor, child_args)| {
                    constructor == "new"
                        && index.is_package_declaration(&child, "flutter", "Column")
                        && child_args.positional().is_empty()
                        && child_args
                            .named()
                            .all(|arg| arg.name().name() == "children")
                })
        };
        if matches {
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-using-list-view",
                "Use ListView instead of a SingleChildScrollView containing a Column.",
                crate::lint::flutter::constructor_span(node),
            ));
        }
    }
}
