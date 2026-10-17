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

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::expressions;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidCatchingErrors;

impl Rule for AvoidCatchingErrors {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diagnostics: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        for catch in statement.catches() {
            let Some(caught) = catch.exception_type() else {
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
                let name = expressions::normalized(self.ctx, &caught.span()).unwrap_or_default();
                format!(
                    "The type '{name}' should not be caught because it is a subclass of 'Error'."
                )
            };
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-catching-errors",
                message,
                catch.span(),
            ));
        }
    }
}
