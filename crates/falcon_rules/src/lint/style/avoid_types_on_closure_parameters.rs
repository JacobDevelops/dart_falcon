//! Flags type annotations on closure parameters whose types come from context.
//!
//! When a function expression is passed where a function type is expected,
//! its parameter types are inferred from that type, so annotating them is
//! redundant. Closures without a known function-typed context are left alone
//! because their annotations carry information.
//!
//! ```dart
//! names.map((String name) => name.length); // flagged
//! names.map((name) => name.length);        // ok
//! ```
//!
//! Adopted from the official `avoid_types_on_closure_parameters` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidTypesOnClosureParameters;

impl Rule for AvoidTypesOnClosureParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::FuncExpr { params, .. } = node else {
            return;
        };
        if !matches!(state.expected(), Some(ResolvedType::Function { .. })) {
            return;
        }
        for param in &params.params {
            // A function-typed formal (`int f()`) is reported whole; otherwise
            // only a named type other than `dynamic` is redundant.
            let span = if param.function_params.is_some() {
                param.span
            } else {
                match &param.param_type {
                    Some(DartType::Named(named)) => named.span,
                    Some(DartType::Void { span }) => *span,
                    Some(
                        DartType::Dynamic { .. } | DartType::Function(_) | DartType::Record(_),
                    )
                    | None => continue,
                }
            };
            self.diags.push(self.ctx.diagnostic(
                "avoid-types-on-closure-parameters",
                "Unnecessary type annotation on a function expression parameter.",
                span,
            ));
        }
    }
}
