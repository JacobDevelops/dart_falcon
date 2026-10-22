//! Prefers SizedBox.square for an unnamed Flutter SizedBox whose explicit width
//! and height are stable variable reads or numeric literals with identical token
//! spelling. Calls, getters, indexing, missing/null dimensions and existing named
//! constructors are excluded. No automatic fix is offered.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferSizedBoxSquare;

impl Rule for PreferSizedBoxSquare {
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
        let Some(index) = self.ctx.identities() else {
            return;
        };
        if !index.is_package_declaration(&identity, "flutter", "SizedBox") {
            return;
        }
        let matches = {
            let dimension = |name| {
                args.named()
                    .find(|arg| arg.name().name() == name)
                    .map(|arg| arg.value())
            };
            match (dimension("width"), dimension("height")) {
                (Some(width), Some(height)) => {
                    (match width.unparenthesized() {
                        Expr::Ident(identifier) => state.is_variable_read(identifier.name()),
                        Expr::IntLit(_) | Expr::DoubleLit(_) => true,
                        _ => false,
                    }) && crate::lint::expressions::equivalent(width, height, self.ctx)
                }
                _ => false,
            }
        };
        if matches {
            self.diagnostics.push(self.ctx.diagnostic("Use `SizedBox.square` when `width` and `height` are the same.", crate::lint::flutter::constructor_span(node)));
        }
    }
}
