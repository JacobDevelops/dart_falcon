//! Flags a null-aware access whose member comes from an extension on a nullable type.
//!
//! An extension declared `on String?` already accepts `null`, so `s?.member`
//! skips a call the extension could handle, and `s.member` is what the author
//! means. The rule fires on `?.` property and method access and on `?[]`
//! index access when the target is an extension override (`E(s)?.member`) of
//! an extension on a nullable type, or when the target's static type is
//! nullable and every visible extension supplying that member to the type is
//! on a nullable type. Members the target's own class declares win over
//! extensions and are not checked; for SDK classes falcon uses the loaded SDK
//! sources, or without them its curated `dart:core` table (e.g.
//! `String.length`, `String.[]`).
//!
//! Every section of a null-aware cascade (`x?..m()..n()`) is null-aware too;
//! each section's leading member is checked against the cascade target. An
//! extension on `dynamic` (or `void`) is on a nullable type.
//!
//! Imported extensions participate even through prefixed imports, subject to
//! show/hide combinators and exports. Explicit overrides resolve the extension
//! by declaration identity, including qualified names. The rule stays silent
//! when it cannot tell which extensions apply.
//!
//! Adopted from the official `unnecessary_null_aware_operator_on_extension_on_nullable`
//! lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, MemberResult, ResolvedType, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::TokenKind;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UnnecessaryNullAwareOperatorOnExtensionOnNullable;

impl Rule for UnnecessaryNullAwareOperatorOnExtensionOnNullable {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            assigned: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Spans of assignment targets; a null-aware access there writes a setter.
    assigned: Vec<Span>,
}

/// Whether `ty` is written nullable, as `T?` or `Null`; `dynamic` and unknown
/// types are not proof.
fn explicitly_nullable(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Null => true,
        ResolvedType::Interface { nullable, .. }
        | ResolvedType::TypeParameter { nullable, .. }
        | ResolvedType::Function { nullable, .. }
        | ResolvedType::Record { nullable, .. } => *nullable,
        ResolvedType::Unknown
        | ResolvedType::Dynamic
        | ResolvedType::Void
        | ResolvedType::Never
        | ResolvedType::NonNullTypeParameter { .. } => false,
    }
}

/// Whether an extension's `on` type is nullable to the type system:
/// `dynamic` and `void` are, besides written-nullable types.
fn extended_type_nullable(ty: &ResolvedType) -> bool {
    explicitly_nullable(ty) || matches!(ty, ResolvedType::Dynamic | ResolvedType::Void)
}

impl Collector<'_> {
    /// Whether the extension supplying `member` to `target` is on a nullable type.
    fn extension_on_nullable(
        &self,
        target: &Expr,
        member: &str,
        state: &SemanticState<'_>,
    ) -> bool {
        if let Expr::Call { callee, args, .. } = target.unparenthesized()
            && args.positional.len() == 1
            && args.named.is_empty()
            && let Some(identity) = state.named_extension(callee)
            && let Some(on_type) = state.signatures.extension_on_type(&identity)
        {
            return extended_type_nullable(on_type);
        }
        let receiver = state.infer(target);
        if !explicitly_nullable(&receiver) || matches!(receiver, ResolvedType::Null) {
            return false;
        }
        let non_null = receiver.with_nullable(false);
        let lookup = match &non_null {
            ResolvedType::TypeParameter { bound, .. } => bound.with_nullable(false),
            ResolvedType::Interface { .. } => non_null.clone(),
            _ => return false,
        };
        let getter = member.strip_suffix('=').unwrap_or(member);
        let declared_by_class = state
            .signatures
            .resolved_member_owner(&lookup, getter, &state.model)
            .is_some()
            || state
                .signatures
                .resolved_field(&lookup, getter, &state.model)
                .is_some()
            || state
                .signatures
                .resolved_writable_field(&lookup, getter, &state.model)
                .is_some()
            || self.core_type_declares(&lookup, getter);
        if declared_by_class {
            return false;
        }
        state
            .signatures
            .applicable_extension_on_types(&non_null, member, &state.model)
            .is_some_and(|on_types| {
                !on_types.is_empty() && on_types.into_iter().all(extended_type_nullable)
            })
    }

    /// Whether the type index records `member` on an SDK receiver: loaded SDK
    /// sources when present, otherwise the curated `dart:core` table.
    fn core_type_declares(&self, receiver: &ResolvedType, member: &str) -> bool {
        let ResolvedType::Interface {
            identity: identity @ DeclarationIdentity::Sdk { .. },
            ..
        } = receiver
        else {
            return false;
        };
        self.ctx.types.is_some_and(|types| {
            matches!(
                types.member_lookup_identity(identity, member),
                MemberResult::Found(_)
            )
        })
    }

    fn report(&mut self, from: usize, to: usize, kind: TokenKind, len: usize) {
        let Some(token) = self
            .ctx
            .syntax
            .tokens_in(Span::new(from, to))
            .find(|token| token.kind == kind)
        else {
            return;
        };
        self.diags.push(self.ctx.diagnostic(
            "unnecessary-null-aware-operator-on-extension-on-nullable",
            "Unnecessary use of a null-aware operator to invoke an extension method on a nullable type.",
            DiagSpan {
                start: token.offset(),
                end: token.offset() + len,
            },
        ));
    }

    /// Reports the `?..` or `..` operator opening the cascade section at `start`.
    fn report_cascade_operator(&mut self, start: usize) {
        let Some(token) = self
            .ctx
            .syntax
            .tokens_in(Span::new(start.saturating_sub(1), start + 3))
            .find(|token| {
                matches!(token.kind, TokenKind::DotDot | TokenKind::DotDotQmark)
                    && token.offset() <= start
                    && start < token.offset() + token.len()
            })
        else {
            return;
        };
        self.diags.push(self.ctx.diagnostic(
            "unnecessary-null-aware-operator-on-extension-on-nullable",
            "Unnecessary use of a null-aware operator to invoke an extension method on a nullable type.",
            DiagSpan {
                start: token.offset(),
                end: token.offset() + token.len(),
            },
        ));
    }

    fn is_assigned(&self, span: &Span) -> bool {
        self.assigned.contains(span)
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Assign { target, .. } => self.assigned.push(*target.span()),
            Expr::Field {
                object,
                field,
                is_null_safe: true,
                span,
            } => {
                let member = if self.is_assigned(span) {
                    format!("{}=", field.name)
                } else {
                    field.name.clone()
                };
                if self.extension_on_nullable(object, &member, state) {
                    self.report(object.span().end, field.span.start, TokenKind::QmarkDot, 2);
                }
            }
            Expr::Cascade {
                object,
                sections,
                is_null_aware: true,
                ..
            } => {
                for section in sections {
                    let member = match section.ops.as_slice() {
                        [CascadeOp::Field { name, .. }, CascadeOp::Assign(..)] => {
                            format!("{}=", name.name)
                        }
                        [CascadeOp::Field { name, .. }, ..] | [CascadeOp::Call(name, ..), ..] => {
                            name.name.clone()
                        }
                        [CascadeOp::IndexAssign(..), ..] => "[]=".to_string(),
                        [CascadeOp::Index { .. }, ..] => "[]".to_string(),
                        _ => continue,
                    };
                    if self.extension_on_nullable(object, &member, state) {
                        self.report_cascade_operator(section.span.start);
                    }
                }
            }
            Expr::Index {
                object,
                index,
                is_null_safe: true,
                span,
            } => {
                let member = if self.is_assigned(span) { "[]=" } else { "[]" };
                if self.extension_on_nullable(object, member, state) {
                    self.report(
                        object.span().end,
                        index.span().start,
                        TokenKind::QmarkLBracket,
                        1,
                    );
                }
            }
            _ => {}
        }
    }
}
