//! Keeps Riverpod notifier initialization in `build`, rather than constructors.
//!
//! Non-empty constructor bodies, initializing formals, field initializers,
//! assertions, and constructor calls with arguments are reported. Empty bodies
//! (including comments), argument-free delegation, and redirecting factories
//! contain no initialization logic of their own and are allowed. Factory bodies
//! are checked just like generative constructor bodies.
//!
//! The check follows resolved Riverpod inheritance, including asynchronous,
//! stream, auto-dispose, family, and generated notifier bases. Unrelated classes
//! with the same names and legacy `StateNotifier` classes are not checked.
//!
//! ```dart
//! class Counter extends Notifier<int> {
//!   Counter() { state = 1; } // Move this logic into build.
//!   int build() => 0;
//! }
//! class CleanCounter extends Notifier<int> {
//!   CleanCounter(); // An empty constructor is allowed.
//!   int build() => 1;
//! }
//! ```

use crate::lint::riverpod::NOTIFIER_BASES;
use falcon_analyze::{
    AnalyzeContext, ResolvedTypeView, Rule, RulePreparation, SemanticRuleVisitor, SemanticState,
    Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{ConstructorInitializer, ConstructorRef, FunctionBody, Program};

/// Warns when a Riverpod notifier constructor contains initialization logic.
///
/// Move initialization to `build` instead. Non-empty bodies, initializing
/// formals, field initializers, assertions, and delegation with arguments are
/// reported. Empty constructors and redirecting factories are allowed.
#[derive(Default)]
pub struct AvoidNotifierConstructors;

impl Rule for AvoidNotifierConstructors {
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
    fn visit_constructor_ref(&mut self, mut node: ConstructorRef<'a>, state: &SemanticState<'a>) {
        let Some(owner) = state.enclosing_type.as_ref() else {
            return;
        };
        let ResolvedTypeView::Interface { identity, .. } = owner.view() else {
            return;
        };
        if state
            .signatures
            .member_supertype_matches(identity, &|identity| {
                crate::lint::riverpod::is_identity(self.ctx, identity, NOTIFIER_BASES)
            })
            != Truth::Yes
        {
            return;
        }
        let has_logic = node.body.is_some_and(|body| match body {
            FunctionBody::Block(block) => block.stmts().next().is_some(),
            FunctionBody::Arrow(_) | FunctionBody::Native(_) => true,
        }) || node.params.params().any(|param| {
            param.initializing().is_some()
                || node.is_primary && (param.is_var() || param.is_final())
        }) || node.initializers.any(|initializer| match initializer {
            ConstructorInitializer::FieldInit(_) | ConstructorInitializer::Assert(_) => true,
            ConstructorInitializer::SuperCall(call) => {
                call.args().positional().next().is_some() || call.args().named().next().is_some()
            }
            ConstructorInitializer::ThisCall(call) => {
                call.args().positional().next().is_some() || call.args().named().next().is_some()
            }
        });
        if has_logic {
            self.diagnostics.push(self.ctx.diagnostic(
                "Avoid initialization logic in notifier constructors; move it to `build`.",
                node.name_span,
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
