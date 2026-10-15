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

use falcon_analyze::{AnalyzeContext, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::semantic_type_operations::nullability;

#[derive(Default)]
pub struct CastNullableToNonNullable;

impl Rule for CastNullableToNonNullable {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diagnostics: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::As {
            expr,
            dart_type,
            span,
        } = node
        else {
            return;
        };
        let target = state
            .model
            .resolve_type_in(dart_type, &state.type_parameters);
        if nullability(&target, &state.signatures, self.ctx.types) != Truth::No {
            return;
        }
        if nullability(&state.infer(expr), &state.signatures, self.ctx.types) != Truth::Yes {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "cast-nullable-to-non-nullable",
            "Don't cast a nullable value to a non-nullable type.",
            *span,
        ));
    }
}
