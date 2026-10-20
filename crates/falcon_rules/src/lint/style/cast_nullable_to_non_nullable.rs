//! Flags `e as T` casts of a nullable value to a non-nullable type.
//!
//! Casting a nullable expression straight to a non-nullable type hides the null
//! check inside the cast; the null assertion operator `!` states the intent
//! directly. The cast is reported only when the operand's static type is proven
//! nullable (never `dynamic` or unresolved) and the target type is proven
//! non-nullable (not a potentially nullable type parameter, `dynamic`, `void`,
//! `Null`, or `FutureOr` of a nullable type). Operand types include the
//! promotions proven on the current control-flow path.
//!
//! ```dart
//! String? s;
//! var a = s as String; // flagged
//! var b = s!;          // ok
//! ```
//!
//! Adopted from the official `cast_nullable_to_non_nullable` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::semantic_type_operations::nullability;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct CastNullableToNonNullable;

impl Rule for CastNullableToNonNullable {
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
        let Expr::As(cast) = node else {
            return;
        };
        let target = state
            .model
            .resolve_type_in(cast.dart_type(), &state.type_parameters);
        if nullability(&target, &state.signatures, self.ctx.types()) != Truth::No {
            return;
        }
        if nullability(
            &state.infer(cast.expr()),
            &state.signatures,
            self.ctx.types(),
        ) != Truth::Yes
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "cast-nullable-to-non-nullable",
            "Don't cast a nullable value to a non-nullable type.",
            cast.span(),
        ));
    }
}
