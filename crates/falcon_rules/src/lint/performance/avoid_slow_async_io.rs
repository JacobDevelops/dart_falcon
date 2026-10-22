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
//! `exists` and `stat`, and the static `FileSystemEntity.isDirectory`,
//! `isFile`, `isLink`, and `type` (also through an import prefix). Instance
//! calls count when the invoked member is the `dart:io` declaration, also
//! through a project class or extension type implementing `File`,
//! `Directory`, `Link`, or `FileSystemEntity` that does not declare it.
//! Receivers whose type is unresolved stay silent.
//!
//! Adopted from the official `avoid_slow_async_io` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, ResolvedTypeView, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct AvoidSlowAsyncIo;

impl Rule for AvoidSlowAsyncIo {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic("Avoid using asynchronous `dart:io` methods here.", span));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) => {
                let Expr::Field(field) = call.callee().unparenthesized() else {
                    return;
                };
                if is_static_entity_query(field.object(), field.field().name(), state)
                    || slow_instance_method(
                        &state.infer(field.object()),
                        field.field().name(),
                        state,
                    )
                {
                    self.report(call.span());
                }
            }
            Expr::Cascade(cascade) => {
                let receiver = state.infer(cascade.object());
                for section in cascade.sections() {
                    if let Some(CascadeOp::Call(call)) = section.ops().next()
                        && slow_instance_method(&receiver, call.name().name(), state)
                    {
                        self.report(section.span());
                    }
                }
            }
            _ => {}
        }
    }
}

/// `FileSystemEntity.isDirectory(path)` and the other static type queries.
fn is_static_entity_query(object: Expr<'_>, method: &str, state: &SemanticState<'_>) -> bool {
    let Some(segments) = falcon_analyze::expression_segments(object, false) else {
        return false;
    };
    matches!(method, "isDirectory" | "isFile" | "isLink" | "type")
        && !state.environment.is_bound(segments[0])
        && state
            .model
            .resolve_dotted(&segments)
            .is_some_and(|identity| identity.is_sdk("dart:io", "FileSystemEntity"))
}

/// Whether `method` on `receiver` resolves to `File.lastModified` or to
/// `FileSystemEntity.exists` / `FileSystemEntity.stat`.
fn slow_instance_method(receiver: &ResolvedType, method: &str, state: &SemanticState<'_>) -> bool {
    let ResolvedTypeView::Interface { identity, .. } = receiver.view() else {
        return false;
    };
    if let Some(slow) = slow_sdk_method(identity, method) {
        return slow;
    }
    if !matches!(method, "lastModified" | "exists" | "stat") {
        return false;
    }
    let signatures = &state.signatures;
    match signatures.resolved_member_owner(receiver, method, &state.model) {
        // A declaration of the receiver's own hierarchy, or a loaded SDK one.
        Some(owner) => slow_sdk_method(&owner, method).unwrap_or(false),
        // The member is inherited from an SDK type whose source is not loaded.
        None => {
            let implements =
                |name| signatures.implements_sdk(receiver, "dart:io", name) == Truth::Yes;
            if method == "lastModified" {
                implements("File")
            } else {
                ["File", "Directory", "Link", "FileSystemEntity"]
                    .into_iter()
                    .any(implements)
            }
        }
    }
}

/// For a `dart:io` entity type, whether `method` invoked on it is one of the
/// slow queries; `None` for other types.
fn slow_sdk_method(identity: &DeclarationIdentity, method: &str) -> Option<bool> {
    let DeclarationIdentity::Sdk { library, name } = identity else {
        return None;
    };
    if library.as_str() != "dart:io" {
        return None;
    }
    match name.as_str() {
        "File" => Some(matches!(method, "lastModified" | "exists" | "stat")),
        "Directory" | "FileSystemEntity" | "Link" => Some(matches!(method, "exists" | "stat")),
        _ => None,
    }
}
