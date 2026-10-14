//! Avoid the slow asynchronous `dart:io` file-system queries.
//!
//! These methods are much slower than their synchronous counterparts because
//! each one round-trips through the event loop for a tiny amount of work:
//!
//! ```dart
//! if (await file.exists()) {} // BAD
//! if (file.existsSync()) {} // GOOD
//! ```
//!
//! Reported calls: `File.lastModified`, the `FileSystemEntity` instance methods
//! `exists` and `stat` (reached through `File`, `Directory`, `Link`, or
//! `FileSystemEntity` receivers), and the static `FileSystemEntity.isDirectory`,
//! `isFile`, `isLink`, and `type`. Receivers whose type is unresolved or a
//! project subtype stay silent.
//!
//! Adopted from the official `avoid_slow_async_io` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidSlowAsyncIo;

impl Rule for AvoidSlowAsyncIo {
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

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-slow-async-io",
            "Use of an async 'dart:io' method.",
            span,
        ));
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Call { callee, span, .. } => {
                let Expr::Field { object, field, .. } = callee.as_ref() else {
                    return;
                };
                if is_static_entity_query(object, &field.name, state)
                    || slow_instance_method(&state.infer(object), &field.name)
                {
                    self.report(*span);
                }
            }
            Expr::Cascade {
                object, sections, ..
            } => {
                let receiver = state.infer(object);
                for section in sections {
                    if let [CascadeOp::Call(name, _, _), ..] = section.ops.as_slice()
                        && slow_instance_method(&receiver, &name.name)
                    {
                        self.report(section.span);
                    }
                }
            }
            _ => {}
        }
    }
}

/// `FileSystemEntity.isDirectory(path)` and the other static type queries.
fn is_static_entity_query(object: &Expr, method: &str, state: &SemanticState<'_>) -> bool {
    let Expr::Ident(target) = object else {
        return false;
    };
    matches!(method, "isDirectory" | "isFile" | "isLink" | "type")
        && !state.environment.is_bound(&target.name)
        && state
            .model
            .resolve_name(target.name.as_str())
            .is_some_and(|identity| identity.is_sdk("dart:io", "FileSystemEntity"))
}

/// Whether `method` on `receiver` resolves to `File.lastModified` or to
/// `FileSystemEntity.exists` / `FileSystemEntity.stat`.
fn slow_instance_method(receiver: &ResolvedType, method: &str) -> bool {
    let ResolvedType::Interface {
        identity:
            DeclarationIdentity::Sdk {
                library: "dart:io",
                name,
            },
        ..
    } = receiver
    else {
        return false;
    };
    match *name {
        "File" => matches!(method, "lastModified" | "exists" | "stat"),
        "Directory" | "FileSystemEntity" | "Link" => matches!(method, "exists" | "stat"),
        _ => false,
    }
}
