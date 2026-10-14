//! Prefers `throwsA` matchers over `try { ...; fail(...); } catch (...) {}`.
//!
//! A `try` statement with a single `catch` clause, no `finally`, and a body
//! that ends in a call to the test framework's `fail` hand-rolls what
//! `expect(() => ..., throwsA(...))` states directly, and with worse failure
//! messages. Only a `fail` that resolves to the one exported by `package:test`,
//! `package:test_api`, or `package:flutter_test` counts; a local or member
//! function named `fail` does not.
//!
//! ```dart
//! try {
//!   parse('');
//!   fail('Expected a FormatException');  // BAD
//! } on FormatException {}
//!
//! expect(() => parse(''), throwsA(isA<FormatException>())); // GOOD
//! ```
//!
//! Adopted from the official `use_test_throws_matchers` lint.

use falcon_analyze::{AnalyzeContext, QualifiedName, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UseTestThrowsMatchers;

impl Rule for UseTestThrowsMatchers {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        if statement.catches.len() != 1 || statement.finally.is_some() {
            return;
        }
        if let Some(Stmt::Expr(last)) = statement.body.stmts.last()
            && is_test_fail(&last.expr, state)
        {
            self.diags.push(self.ctx.diagnostic(
                "use-test-throws-matchers",
                "Use the 'throwsA' matcher instead of using 'fail' when there is no exception thrown.",
                last.span,
            ));
        }
    }
}

/// Whether `expression` invokes `fail` from the test framework.
fn is_test_fail(expression: &Expr, state: &SemanticState<'_>) -> bool {
    let Expr::Call { callee, .. } = expression else {
        return false;
    };
    let name = match callee.as_ref() {
        Expr::Ident(name) => {
            let shadowed = state.environment.is_bound(&name.name)
                || state.enclosing_type.as_ref().is_some_and(|receiver| {
                    state
                        .signatures
                        .resolved_member(receiver, &name.name, &state.model)
                        .is_some()
                });
            if shadowed {
                return false;
            }
            QualifiedName::from(&name.name)
        }
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } => match object.as_ref() {
            Expr::Ident(prefix) if !state.environment.is_bound(&prefix.name) => {
                QualifiedName::prefixed(&prefix.name, &field.name)
            }
            _ => return false,
        },
        _ => return false,
    };
    state
        .model
        .resolve_imported_member(name)
        .is_some_and(|identity| {
            ["test", "test_api", "flutter_test"]
                .iter()
                .any(|package| identity.is_package(package, "fail"))
        })
}
