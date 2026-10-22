//! Requires stable value equality for freshly created Riverpod family keys.
//!
//! Checks package-resolved Riverpod family objects, including subclasses and
//! aliases. Non-constant collections, closures, and fresh instances inheriting
//! Object's equality are rejected. Records are checked recursively; existing
//! values, constants, and unresolved equality implementations are accepted.

use crate::lint::flutter::scoped_constructor_invocation;
use crate::lint::riverpod::is_identity;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferImmutableProviderArguments;

impl Rule for PreferImmutableProviderArguments {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

struct ArgumentEquality<'s, 'a> {
    state: &'s SemanticState<'a>,
    unstable: bool,
}

impl<'a> Visitor<'a> for ArgumentEquality<'_, 'a> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        if self.unstable {
            return;
        }
        match expression {
            Expr::Record(record) if !record.is_const() => walk_expr(self, expression),
            Expr::Conditional(_) | Expr::Paren(_) | Expr::As(_) | Expr::NullAssert(_) => {
                walk_expr(self, expression);
            }
            _ => self.unstable = Collector::unstable(expression, self.state),
        }
    }
}

impl Collector<'_> {
    fn unstable(argument: Expr<'_>, state: &SemanticState<'_>) -> bool {
        let argument = argument.unparenthesized();
        match argument {
            Expr::List(list) => !list.is_const(),
            Expr::Set(set) => !set.is_const(),
            Expr::Map(map) => !map.is_const(),
            Expr::FuncExpr(_) => true,
            Expr::New(construction) if construction.is_const() => false,
            Expr::New(_) | Expr::Call(_) => {
                let Some((identity, constructor, _)) =
                    scoped_constructor_invocation(argument, state)
                else {
                    return false;
                };
                if !state.signatures.has_constructor(&identity, constructor)
                    || matches!(state.infer(argument), ResolvedType::Interface { extension_type: true, .. })
                    // Factories may return a cached value or a subtype with its own equality.
                    || state.signatures.declaration(&identity).is_some_and(|declaration| {
                        declaration.constructors.iter().any(|fact| {
                            fact.name == constructor && fact.is_factory
                        })
                    })
                    || state
                        .signatures
                        .declared_member_facts(&identity, "==")
                        .iter()
                        .any(|member| !member.is_static && !member.is_abstract)
                {
                    return false;
                }
                state
                    .signatures
                    .inherited_concrete_member(&identity, "==")
                    .is_some_and(|member| {
                        member.is_none_or(|member| {
                            member.declaring_type == DeclarationIdentity::OBJECT
                        })
                    })
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
            let mut equality = ArgumentEquality {
                state,
                unstable: false,
            };
            equality.visit_expr(argument);
            if equality.unstable {
                self.diagnostics.push(self.ctx.diagnostic("Use a provider family argument with stable value equality.", argument.span()));
            }
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
