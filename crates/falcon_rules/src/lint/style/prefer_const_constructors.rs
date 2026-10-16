//! Prefers `const` when invoking a `const` constructor with constant arguments.
//!
//! When a constructor is declared `const` and the invocation would be a valid
//! constant expression (constant arguments the constructor accepts, set
//! elements and map keys with primitive equality, no type parameters in type
//! arguments), invoking it with `const` produces a canonicalized instance
//! instead of allocating a new one. The rule flags such invocations outside a
//! constant context (where `const` is already implied). It stays silent when
//! the constructor cannot be resolved to a declaration known to be `const`,
//! when an argument cannot be proven constant, for `Object()` (often used to create
//! unique identity tokens), for constructors of deferred-imported classes, and
//! for a generic class invoked without type arguments whose approximate context
//! contains type parameters. As in the analyzer, assignments use their result
//! type rather than their target type.
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

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::const_eval::{ConstContexts, constant_invocation, deferred_prefixes};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::invocation_type_args;

#[derive(Default)]
pub struct PreferConstConstructors;

impl Rule for PreferConstConstructors {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            contexts: ConstContexts::of(program),
            deferred_prefixes: deferred_prefixes(program),
            assignment_values: HashSet::new(),
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
    assignment_values: HashSet<Span>,
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
        if let Expr::Assign {
            op: AssignOp::Eq,
            value,
            ..
        } = node
        {
            let mut value = value.as_ref();
            loop {
                match value {
                    Expr::Paren { expr, .. } => value = expr,
                    // The upstream approximation also climbs cascade targets.
                    Expr::Cascade { object, .. } => value = object,
                    _ => break,
                }
            }
            self.assignment_values.insert(*value.span());
        }
        if matches!(node, Expr::Paren { .. }) {
            return;
        }
        let Some(invocation) = self.invocation(node, state) else {
            return;
        };
        if self.contexts.contains(node.span())
            || invocation.identity == DeclarationIdentity::OBJECT
            || self.is_literal(&invocation, state)
            || self.inference_depends_on_context(node, &invocation, state)
        {
            return;
        }
        if constant_invocation(
            &invocation.identity,
            invocation.constructor,
            invocation.type_args,
            invocation.args,
            &self.deferred_prefixes,
            state,
        ) {
            self.diags.push(self.ctx.diagnostic(
                "prefer-const-constructors",
                "Use 'const' with the constructor to improve performance.",
                *node.span(),
            ));
        }
    }
}

impl Collector<'_> {
    /// package:meta's `@literal` constructors are left to the analyzer's
    /// `non_const_call_to_literal_constructor` warning.
    fn is_literal(&self, invocation: &Invocation<'_>, state: &SemanticState<'_>) -> bool {
        state
            .signatures
            .declaration(&invocation.identity)
            .is_some_and(|facts| {
                facts.constructors.iter().any(|constructor| {
                    constructor.name == invocation.constructor
                        && constructor
                            .annotations
                            .iter()
                            .any(|annotation| annotation.is_package("meta", "literal"))
                })
            })
    }

    /// Match the analyzer's approximate context: an assignment contributes its
    /// result type (the inferred RHS), while a return contributes its declared
    /// return type. A supertype return context must not be inferred backwards.
    fn inference_depends_on_context(
        &self,
        node: &Expr,
        invocation: &Invocation<'_>,
        state: &SemanticState<'_>,
    ) -> bool {
        if invocation.shorthand || !invocation.type_args.is_empty() {
            return false;
        }
        let generic = state
            .signatures
            .constructor(&invocation.identity, invocation.constructor)
            .is_some_and(|signature| !signature.owner_parameters.is_empty());
        if !generic {
            return false;
        }
        let assignment_type;
        let context = if self.assignment_values.contains(node.span()) {
            assignment_type = state.infer_with_context(node, state.expected());
            Some(&assignment_type)
        } else {
            state.expected()
        };
        context
            .and_then(|context| {
                state
                    .signatures
                    .as_instance_of(context, &invocation.identity, &state.model)
            })
            .is_some_and(|instance| {
                instance
                    .arguments()
                    .iter()
                    .any(|argument| matches!(argument, ResolvedType::TypeParameter { .. }))
            })
    }

    fn invocation<'e>(&self, node: &'e Expr, state: &SemanticState<'_>) -> Option<Invocation<'e>> {
        match node.unparenthesized() {
            Expr::New {
                is_const: false,
                dart_type: dart_type @ DartType::Named(named),
                constructor_name,
                args,
                ..
            } => {
                if named.segments.len() > 1 && self.is_deferred(&named.segments[0].name) {
                    return None;
                }
                let (identity, constructor) = falcon_analyze::resolve_new_constructor(
                    dart_type,
                    constructor_name.as_ref(),
                    &state.model,
                )?;
                Some(Invocation {
                    identity: state.model.unaliased(identity),
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
            } => match callee.unparenthesized() {
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
                    let segments = falcon_analyze::expression_segments(callee, true)?;
                    if state.environment.is_bound(segments[0])
                        || (segments.len() > 1 && self.is_deferred(segments[0]))
                    {
                        return None;
                    }
                    let (identity, constructor) =
                        falcon_analyze::resolve_call_constructor(callee, &state.model, true)?;
                    Some(Invocation {
                        identity: state.model.unaliased(identity),
                        constructor,
                        type_args: invocation_type_args(callee, type_args),
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
