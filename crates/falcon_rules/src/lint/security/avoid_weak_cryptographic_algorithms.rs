//! Flags references to MD5, SHA-1, DES, triple DES, and RC4 package APIs.
//!
//! Weak hashes and ciphers cannot reliably protect sensitive data; use a modern
//! algorithm instead. The rule recognizes `md5` and `sha1` from `crypto`, `DES`
//! and `DES3` from `dart_des`, and `RC4` from `simple_rc4`, including through
//! prefixed imports and re-exports. Unrelated APIs with the same spelling are
//! not reported. Algorithm uses and constructor references are checked, but
//! type annotations alone are not.

use std::collections::HashSet;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
    resolve_new_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidWeakCryptographicAlgorithms;

impl Rule for AvoidWeakCryptographicAlgorithms {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(WeakCrypto {
            ctx,
            diagnostics: Vec::new(),
            extensions: program
                .declarations()
                .filter_map(|declaration| match declaration {
                    TopLevelDecl::Extension(extension) => {
                        let mut members = extension.members();
                        let span = extension.span();
                        let declaration_span = Span::new(
                            extension
                                .annotations()
                                .next()
                                .map_or(span.start, |annotation| {
                                    annotation.span().start.min(span.start)
                                }),
                            span.end,
                        );
                        let first = members.next().map_or(
                            Span::new(declaration_span.end, declaration_span.end),
                            |member| member.span(),
                        );
                        let end = members
                            .next_back()
                            .map_or(first.end, |member| member.span().end);
                        Some((
                            declaration_span,
                            Span::new(first.start, end),
                            crate::lint::declarations::member_names(extension.members()),
                        ))
                    }
                    _ => None,
                })
                .collect(),
        }))
    }
}

struct WeakCrypto<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
    extensions: Vec<(Span, Span, HashSet<&'a str>)>,
}

impl<'a> SemanticRuleVisitor<'a> for WeakCrypto<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let target = match node {
            Expr::Ident(identifier) => {
                let name = identifier.name();
                if !weak_name(name) || self.shadowed(name, node.span(), state) {
                    return;
                }
                state.model.resolve_imported_member(name)
            }
            Expr::Field(access) => {
                let field = access.field();
                if !weak_name(field.name()) {
                    return;
                }
                let Expr::Ident(prefix) = access.object().unparenthesized() else {
                    return;
                };
                if self.shadowed(prefix.name(), node.span(), state)
                    || state.model.resolve_value(prefix.name()).is_some()
                    || state.model.resolve_name(prefix.name()).is_some()
                {
                    return;
                }
                state
                    .model
                    .resolve_imported_member(QualifiedName::prefixed(prefix.name(), field.name()))
            }
            Expr::New(construction) => {
                let DartType::Named(named) = construction.dart_type() else {
                    return;
                };
                let Some(head) = named.segments().next() else {
                    return;
                };
                if self.shadowed(head.name(), node.span(), state)
                    || state.model.resolve_value(head.name()).is_some()
                {
                    return;
                }
                resolve_new_constructor(
                    construction.dart_type(),
                    construction.constructor_name(),
                    &state.model,
                )
                .map(|(identity, _)| identity)
            }
            _ => return,
        };
        if let Some(identity) = target
            && self.is_weak(&identity)
        {
            self.diagnostics.push(self.ctx.diagnostic("Avoid weak cryptographic algorithms. Use a modern hash or cipher instead.", node.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}

impl WeakCrypto<'_> {
    fn is_weak(&self, identity: &DeclarationIdentity) -> bool {
        let package = match identity.name() {
            "md5" | "sha1" => "crypto",
            "DES" | "DES3" => "dart_des",
            "RC4" => "simple_rc4",
            _ => return false,
        };
        self.ctx.identities().is_some_and(|identities| {
            identities.is_package_declaration(identity, package, identity.name())
        })
    }

    fn shadowed(&self, name: &str, span: Span, state: &SemanticState<'_>) -> bool {
        if state.environment.is_bound(name) || state.type_parameters.contains(name) {
            return true;
        }
        // An extension's on-type is not its lexical member namespace.
        if let Some((_, body, members)) = self
            .extensions
            .iter()
            .find(|(extension, _, _)| extension.contains(span))
        {
            return body.contains(span) && members.contains(name);
        }
        state.constant_owner.as_ref().is_some_and(|owner| {
            !state
                .signatures
                .declared_member_facts(owner, name)
                .is_empty()
        }) || matches!(state.enclosing_type.as_ref().map(ResolvedType::view), Some(ResolvedTypeView::Interface { identity, .. })
            if !state.signatures.declared_member_facts(identity, name).is_empty())
    }
}

fn weak_name(name: &str) -> bool {
    matches!(name, "md5" | "sha1" | "DES" | "DES3" | "RC4")
}
