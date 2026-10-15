//! Don't explicitly catch `Error` or types that implement it.
//!
//! An `Error` signals a programming bug that should crash and be fixed, not be
//! handled at runtime:
//!
//! ```dart
//! try { work(); } on StateError catch (e) { recover(); } // BAD
//! try { work(); } on FormatException catch (e) { recover(); } // GOOD
//! ```
//!
//! Only `on` types whose resolved supertype chain provably reaches `dart:core`
//! `Error` are reported.
//!
//! Adopted from the official `avoid_catching_errors` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::expressions;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidCatchingErrors;

impl Rule for AvoidCatchingErrors {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        for catch in &statement.catches {
            let Some(caught) = &catch.exception_type else {
                continue;
            };
            let ty = state.model.resolve_type_in(caught, &state.type_parameters);
            if state
                .signatures
                .interface_check_sdk(&ty, "dart:core", "Error", &state.model)
                != Truth::Yes
            {
                continue;
            }
            let message = if matches!(&ty, ResolvedType::Interface { identity, .. } if identity.is_sdk("dart:core", "Error"))
            {
                "The type 'Error' should not be caught.".to_string()
            } else {
                let name =
                    expressions::normalized(self.ctx.source, caught.span()).unwrap_or_default();
                format!(
                    "The type '{name}' should not be caught because it is a subclass of 'Error'."
                )
            };
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-catching-errors",
                message,
                catch.span,
            ));
        }
    }
}
