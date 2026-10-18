//! Requires immutable keys when invoking Riverpod provider families.
//!
//! Checks calls to package-resolved Riverpod family objects, including subclasses
//! and aliases. Non-constant lists, sets, maps, and classes with known non-final
//! instance fields are mutable; constant arguments and `@immutable` types are
//! accepted. Records are checked recursively. Unknown types and unresolved field
//! layouts are not diagnosed.

use crate::lint::const_eval::{constant_in_context, deferred_prefixes};
use crate::lint::riverpod::is_identity;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferImmutableProviderArguments;

impl Rule for PreferImmutableProviderArguments {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            deferred: deferred_prefixes(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    deferred: Vec<&'a str>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn mutable(ty: &ResolvedType, state: &SemanticState<'_>) -> bool {
        match ty {
            ResolvedType::Interface { identity, .. } => {
                if state.signatures.has_immutable_supertype(identity, false) == Truth::Yes {
                    return false;
                }
                ["List", "Set", "Map"]
                    .iter()
                    .any(|name| ty.interface("dart:core", name))
                    || state.signatures.has_non_final_field(identity) == Some(true)
            }
            ResolvedType::Record {
                positional, named, ..
            } => {
                positional.iter().any(|ty| Self::mutable(ty, state))
                    || named.iter().any(|(_, ty)| Self::mutable(ty, state))
            }
            _ => false,
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = expression else {
            return;
        };
        let ResolvedType::Interface { identity, .. } = state.infer(call.callee()) else {
            return;
        };
        let family = |identity: &falcon_analyze::DeclarationIdentity| {
            identity.name().ends_with("Family")
                && is_identity(self.ctx, identity, &[identity.name()])
        };
        if !family(&identity)
            && state.signatures.supertype_matches(&identity, &family) != Truth::Yes
        {
            return;
        }
        for argument in call
            .args()
            .positional()
            .chain(call.args().named().map(|argument| argument.value()))
        {
            if !Self::mutable(&state.infer(argument), state) {
                continue;
            }
            let constant = match argument.unparenthesized() {
                Expr::List(list) => list.is_const(),
                Expr::Set(set) => set.is_const(),
                Expr::Map(map) => map.is_const(),
                Expr::New(construction) => construction.is_const(),
                Expr::Ident(_) | Expr::Field(_) => {
                    constant_in_context(argument, &self.deferred, state)
                }
                _ => false,
            };
            if !constant {
                self.diagnostics.push(self.ctx.diagnostic(
                    "prefer-immutable-provider-arguments",
                    "Use an immutable provider family argument so its cache key cannot change.",
                    argument.span(),
                ));
            }
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
