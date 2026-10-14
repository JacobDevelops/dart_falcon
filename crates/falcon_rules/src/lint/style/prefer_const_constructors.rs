//! Prefers `const` when invoking a `const` constructor with constant arguments.
//!
//! When a constructor is declared `const` and every argument is a constant
//! expression, invoking it with `const` produces a canonicalized instance
//! instead of allocating a new one. The rule flags such invocations outside a
//! constant context (where `const` is already implied). It stays silent when
//! the constructor cannot be resolved to a declaration known to be `const`,
//! when an argument cannot be proven constant, for `Object()` (often used to
//! create unique identity tokens), for constructors of deferred-imported
//! classes, and for a generic class invoked without type arguments whose
//! context type mentions a type parameter (making it `const` would change the
//! inferred type arguments).
//!
//! ```dart
//! class A {
//!   const A();
//! }
//!
//! final a = A();       // BAD
//! final b = const A(); // GOOD
//! ```
//!
//! Adopted from the official `prefer_const_constructors` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::const_eval::{ConstContexts, constant_in_context, mentions_type_parameter};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct PreferConstConstructors;

impl Rule for PreferConstConstructors {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            contexts: ConstContexts::of(program),
            deferred_prefixes: program
                .imports
                .iter()
                .filter(|import| import.is_deferred)
                .filter_map(|import| import.as_name.as_ref())
                .map(|prefix| prefix.name.as_str())
                .collect(),
            ctx,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    contexts: ConstContexts,
    deferred_prefixes: Vec<&'a str>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

/// A non-const constructor invocation with its resolved target.
struct Invocation<'a> {
    identity: DeclarationIdentity,
    constructor: &'a str,
    type_args: &'a [DartType],
    args: &'a ArgList,
    /// Dot shorthands take their type, and so their type arguments, from the
    /// context; there is nothing for upward inference to change.
    shorthand: bool,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Some(invocation) = self.invocation(node, state) else {
            return;
        };
        if self.contexts.contains(node.span())
            || invocation.identity == DeclarationIdentity::OBJECT
            || !state
                .signatures
                .is_const_constructor(&invocation.identity, invocation.constructor)
        {
            return;
        }
        let Some(signature) = state
            .signatures
            .constructor(&invocation.identity, invocation.constructor)
        else {
            return;
        };
        if !invocation.shorthand
            && !signature.owner_parameters.is_empty()
            && invocation.type_args.is_empty()
            && state.expected().is_some_and(mentions_type_parameter)
        {
            return;
        }
        let type_args_constant = invocation.type_args.iter().all(|ty| {
            !mentions_type_parameter(&state.model.resolve_type_in(ty, &state.type_parameters))
        });
        let args_constant = invocation
            .args
            .positional
            .iter()
            .chain(invocation.args.named.iter().map(|argument| &argument.value))
            .all(|argument| constant_in_context(argument, state));
        if type_args_constant && args_constant {
            self.diags.push(self.ctx.diagnostic(
                "prefer-const-constructors",
                "Use 'const' with the constructor to improve performance.",
                *node.span(),
            ));
        }
    }
}

impl Collector<'_> {
    fn invocation<'e>(&self, node: &'e Expr, state: &SemanticState<'_>) -> Option<Invocation<'e>> {
        match node {
            Expr::New {
                is_const: false,
                dart_type: DartType::Named(named),
                constructor_name,
                args,
                ..
            } => {
                if named.segments.len() > 1 && self.is_deferred(&named.segments[0].name) {
                    return None;
                }
                let (identity, constructor) = crate::lint::flutter::new_constructor(
                    named,
                    constructor_name.as_ref(),
                    &state.model,
                )?;
                Some(Invocation {
                    identity,
                    constructor,
                    type_args: &named.type_args,
                    args,
                    shorthand: false,
                })
            }
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => match callee.as_ref() {
                Expr::DotShorthand {
                    is_const: false,
                    name,
                    ..
                } => {
                    let Some(ResolvedType::Interface { identity, .. }) = state.expected() else {
                        return None;
                    };
                    Some(Invocation {
                        identity: identity.clone(),
                        constructor: name.name.as_str(),
                        type_args,
                        args,
                        shorthand: true,
                    })
                }
                callee => {
                    let segments = falcon_analyze::expression_segments(callee, false)?;
                    if state.environment.is_bound(segments[0])
                        || (segments.len() > 1 && self.is_deferred(segments[0]))
                    {
                        return None;
                    }
                    let (identity, constructor) =
                        falcon_analyze::resolve_call_constructor(callee, &state.model, false)?;
                    Some(Invocation {
                        identity,
                        constructor,
                        type_args,
                        args,
                        shorthand: false,
                    })
                }
            },
            _ => None,
        }
    }

    fn is_deferred(&self, prefix: &str) -> bool {
        self.deferred_prefixes.contains(&prefix)
    }
}
