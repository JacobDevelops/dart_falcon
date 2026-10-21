//! Flags the type `Future<void> Function()` in favor of Flutter's `AsyncCallback`.
//!
//! Flutter's `foundation` library defines `AsyncCallback` as a typedef for exactly
//! `Future<void> Function()`, the standard signature for an asynchronous, argument-less
//! callback. Spelling the type out longhand is more verbose and less recognizable than
//! the named alias that the rest of the framework uses. The rule matches only the
//! zero-parameter function type returning `Future<void>`; any parameters or a different
//! return type leave it untouched.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferAsyncCallback;

impl Rule for PreferAsyncCallback {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if is_future_void_function(node) {
            let span = node.span();
            self.diags.push(self.ctx.diagnostic(
                "Use AsyncCallback instead of Future<void> Function().",
                span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}

/// True for the type `Future<void> Function()` with no parameters.
fn is_future_void_function(t: DartType<'_>) -> bool {
    let DartType::Function(ft) = t else {
        return false;
    };
    if !ft.params().is_empty() {
        return false;
    }
    let Some(DartType::Named(nt)) = ft.return_type() else {
        return false;
    };
    nt.segments().last().map(|s| s.name()) == Some("Future")
        && nt.type_args().len() == 1
        && matches!(nt.type_args().next(), Some(DartType::Void(_)))
}
