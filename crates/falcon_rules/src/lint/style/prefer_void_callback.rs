//! Prefers Flutter's `VoidCallback` alias over a nongeneric `void Function()` type.
//!
//! Import `VoidCallback` from `package:flutter/foundation.dart` and use
//! `VoidCallback?` for nullable callbacks. Functions with parameters, type
//! parameters, or a non-void return type are not covered.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferVoidCallback;

impl Rule for PreferVoidCallback {
    fn prepare<'a>(&self, _: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_function_type(&mut self, node: FunctionType<'a>) -> StructuralTraversal {
        if node.syntax().kind() == NodeKind::FunctionType
            && matches!(node.return_type(), Some(DartType::Void(_)))
            && node.params().next().is_none()
            && node.type_params().next().is_none()
        {
            self.diagnostics.push(self.ctx.diagnostic("prefer-void-callback", "This type is void Function(); use Flutter's VoidCallback alias (or VoidCallback? when nullable).", node.span()));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }
}
}
