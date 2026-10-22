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

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberResult, ResolvedType, Rule, RulePreparation,
};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::TokenKind;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UnnecessaryNullAwareOperatorOnExtensionOnNullable;

impl Rule for UnnecessaryNullAwareOperatorOnExtensionOnNullable {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            assigned: Vec::new(),
        }))
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

impl<'a> Collector<'a> {
    /// Whether the extension supplying `member` to `target` is on a nullable type.
    fn extension_on_nullable(
        &self,
        target: Expr<'a>,
        member: &str,
        state: &SemanticState<'a>,
    ) -> bool {
        if let Expr::Call(call) = target.unparenthesized()
            && call.args().positional().len() == 1
            && call.args().named().is_empty()
            && let Some(identity) = state.named_extension(call.callee())
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
        self.ctx.types().is_some_and(|types| {
            matches!(
                types.member_lookup_identity(identity, member),
                MemberResult::Found(_)
            )
        })
    }

    fn report(&mut self, from: usize, to: usize, kind: TokenKind, len: usize) {
        let Some(token) = self
            .ctx
            .tree()
            .tokens_in(self.ctx.source(), Span::new(from, to))
            .find(|token| token.kind() == kind)
        else {
            return;
        };
        self.diags.push(self.ctx.diagnostic("Remove the unnecessary null-aware operator when invoking this extension method on a nullable type.", DiagSpan {
            start: token.offset(),
            end: token.offset() + len,
        }));
    }

    /// Reports the `?..` or `..` operator opening the cascade section at `start`.
    fn report_cascade_operator(&mut self, start: usize) {
        let Some(token) = self
            .ctx
            .tree()
            .tokens_in(
                self.ctx.source(),
                Span::new(start.saturating_sub(1), start + 3),
            )
            .find(|token| {
                matches!(token.kind(), TokenKind::DotDot | TokenKind::DotDotQmark)
                    && token.offset() <= start
                    && start < token.offset() + token.len()
            })
        else {
            return;
        };
        self.diags.push(self.ctx.diagnostic("Remove the unnecessary null-aware operator when invoking this extension method on a nullable type.", DiagSpan {
            start: token.offset(),
            end: token.offset() + token.len(),
        }));
    }

    fn is_assigned(&self, span: &Span) -> bool {
        self.assigned.contains(span)
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Assign(assignment) => self.assigned.push(assignment.target().span()),
            Expr::Field(access) if access.is_null_safe() => {
                let member = if self.is_assigned(&access.span()) {
                    format!("{}=", access.field().name())
                } else {
                    access.field().name().to_owned()
                };
                if self.extension_on_nullable(access.object(), &member, state) {
                    self.report(
                        access.object().span().end,
                        access.field().span().start,
                        TokenKind::QmarkDot,
                        2,
                    );
                }
            }
            Expr::Cascade(cascade) if cascade.is_null_aware() => {
                let mut setter = String::new();
                for section in cascade.sections() {
                    let mut ops = section.ops();
                    let member = match ops.next() {
                        Some(CascadeOp::Field(field)) => {
                            if matches!(ops.next(), Some(CascadeOp::Assign(_)))
                                && ops.next().is_none()
                            {
                                setter.clear();
                                setter.reserve(field.name().name().len() + 1);
                                setter.push_str(field.name().name());
                                setter.push('=');
                                setter.as_str()
                            } else {
                                field.name().name()
                            }
                        }
                        Some(CascadeOp::Call(call)) => call.name().name(),
                        Some(CascadeOp::IndexAssign(_)) => "[]=",
                        Some(CascadeOp::Index(_)) => "[]",
                        _ => continue,
                    };
                    if self.extension_on_nullable(cascade.object(), member, state) {
                        self.report_cascade_operator(section.span().start);
                    }
                }
            }
            Expr::Index(access) if access.is_null_safe() => {
                let member = if self.is_assigned(&access.span()) {
                    "[]="
                } else {
                    "[]"
                };
                if self.extension_on_nullable(access.object(), member, state) {
                    self.report(
                        access.object().span().end,
                        access.index().span().start,
                        TokenKind::QmarkLBracket,
                        1,
                    );
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn nullable_extension_getter_is_not_an_instance_member() {
        let source = "class Target {}\n\
                      extension NullableValues on Target? { int get value => 1; }\n\
                      void read(Target? target) { target?.value; }\n";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let path = std::path::Path::new("/project/lib/main.dart");
        let indexes = crate::lint::test_support::TestIndexes::new(&[(path, &parsed)]);
        let config = falcon_config::FalconConfig::default();
        let context = indexes.context(path, &config, &parsed);
        let diagnostics = UnnecessaryNullAwareOperatorOnExtensionOnNullable.analyze(
            UnnecessaryNullAwareOperatorOnExtensionOnNullable::NAME,
            parsed.program(),
            &context,
        );
        let actual: Vec<_> = diagnostics.iter().map(|diagnostic| (
            diagnostic.rule.to_string(),
            source[..diagnostic.span.start].bytes().filter(|&byte| byte == b'\n').count() + 1,
        )).collect();
        assert_eq!(actual, [(UnnecessaryNullAwareOperatorOnExtensionOnNullable::NAME.to_owned(), 3)]);
    }
}
