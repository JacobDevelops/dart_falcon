//! Prefer super parameters when a constructor only forwards parameters to its superclass.
//!
//! Super parameters remove repeated parameter declarations and explicit forwarding
//! arguments. This rule reports a constructor once when its positional parameters
//! can be converted together, or when an individual named parameter can be
//! converted without changing its type contract. Parameters used in the constructor
//! body and field-initializing formals are not suggested.
//!
//! Available starting with Dart 2.17. For example, replace
//! `Child(int value) : super(value);` with `Child(super.value);`.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, LanguageVersion, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct UseSuperParameters;

impl Rule for UseSuperParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if ctx.language_version() < LanguageVersion::new(2, 17) {
            return Vec::new();
        }
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector::default();
        visit_program(&mut collector, program, state);
        collector
            .constructors
            .into_iter()
            .filter_map(|constructor| {
                let referenced = |parameter: Span| {
                    collector
                        .references
                        .get(&parameter)
                        .is_some_and(|(_, used)| *used)
                };
                let mut positional = !constructor.positional.is_empty();
                for check in constructor.positional {
                    match check {
                        PositionalCheck::Invalid => return None,
                        PositionalCheck::Blocked => {
                            positional = false;
                            break;
                        }
                        PositionalCheck::Reference(parameter) if referenced(parameter) => {
                            positional = false;
                            break;
                        }
                        PositionalCheck::Reference(_) => {}
                    }
                }
                let convertible =
                    positional || constructor.named.iter().any(|span| !referenced(*span));
                convertible.then(|| {
                    ctx.diagnostic(
                        "use-super-parameters",
                        "Use super parameters instead of forwarding parameters to super.",
                        constructor.anchor,
                    )
                })
            })
            .collect()
    }
}

#[derive(Default)]
struct Collector {
    references: HashMap<Span, (Span, bool)>,
    constructors: Vec<Conversion>,
}

struct Conversion {
    anchor: Span,
    positional: Vec<PositionalCheck>,
    named: Vec<Span>,
}

enum PositionalCheck {
    Invalid,
    Blocked,
    Reference(Span),
}

impl SemanticRuleVisitor for Collector {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if let Expr::Ident(identifier) = node
            && let Some(binding) = state.environment.binding_declaration(&identifier.name)
            && let Some((body, used)) = self.references.get_mut(&binding)
            && body.start <= identifier.span.start
            && identifier.span.start < body.end
        {
            *used = true;
        }
    }

    fn visit_constructor_ref(&mut self, ctor: ConstructorRef<'_>, state: &SemanticState<'_>) {
        let Some((args, name)) = ctor.initializers.iter().rev().find_map(|init| match init {
            ConstructorInitializer::SuperCall {
                args, call_name, ..
            } => Some((
                args,
                call_name.as_ref().map_or("new", |name| name.name.as_str()),
            )),
            _ => None,
        }) else {
            return;
        };
        let Some(superclass) = &state.superclass else {
            return;
        };
        let Some((signature, substitutions)) =
            state.signatures.resolved_constructor(superclass, name)
        else {
            return;
        };
        let mut conversion = Conversion {
            anchor: if ctor.is_primary {
                Span::new(
                    ctor.span.start,
                    ctor.constructor_name.map_or_else(
                        || {
                            ctor.span.start
                                + if ctor.is_const {
                                    5
                                } else {
                                    ctor.name_span.end - ctor.name_span.start
                                }
                        },
                        |name| name.span.end,
                    ),
                )
            } else {
                Span::new(
                    ctor.name_span.start,
                    ctor.constructor_name
                        .map_or(ctor.name_span.end, |name| name.span.end),
                )
            },
            positional: Vec::with_capacity(ctor.params.params.len()),
            named: Vec::with_capacity(args.named.len()),
        };
        // Every positional super argument must be a distinct positional formal,
        // in declaration order. Other constructor parameters may precede it.
        if args
            .positional
            .iter()
            .all(|arg| matches!(arg, Expr::Ident(_)))
        {
            let mut seen = HashSet::with_capacity(args.positional.len());
            let mut previous = 0;
            for argument in &args.positional {
                let Expr::Ident(identifier) = argument else {
                    unreachable!()
                };
                let Some(index) = ctor
                    .params
                    .params
                    .iter()
                    .position(|param| param.name.name == identifier.name)
                else {
                    conversion.positional.push(PositionalCheck::Invalid);
                    break;
                };
                let parameter = &ctor.params.params[index];
                if matches!(parameter.kind, ParamKind::Named { .. }) || !seen.insert(index) {
                    conversion.positional.push(PositionalCheck::Invalid);
                    break;
                }
                if index < previous {
                    conversion.positional.push(PositionalCheck::Blocked);
                    break;
                }
                for parameter in &ctor.params.params[previous..=index] {
                    conversion.positional.push(match parameter.initializing {
                        Some(InitializingFormal::Super) => PositionalCheck::Invalid,
                        Some(InitializingFormal::This) => PositionalCheck::Blocked,
                        None => PositionalCheck::Reference(parameter.name.span),
                    });
                }
                previous = index + 1;
            }
        }
        if let Some(body) = ctor.body {
            for parameter in &ctor.params.params {
                self.references
                    .entry(parameter.name.span)
                    .or_insert((*body.span(), false));
            }
        }
        for parameter in ctor.params.named() {
            if parameter.initializing == Some(InitializingFormal::This)
                || (ctor.is_primary && (parameter.is_final || parameter.is_var))
            {
                continue;
            }
            let Some(super_type) = signature.named.get(&parameter.name.name) else {
                continue;
            };
            if !args.named.iter().any(|argument| {
                argument.name.name == parameter.name.name
                    && matches!(&argument.value, Expr::Ident(identifier) if identifier.name == parameter.name.name)
            }) { continue; }
            let super_type = state.model.substitute(super_type, &substitutions);
            let this_type = state.environment.declared_type(&parameter.name.name);
            if matches!(super_type, ResolvedType::Dynamic)
                || matches!(this_type, ResolvedType::Dynamic)
                || state
                    .signatures
                    .subtype(&super_type, &this_type, &state.model)
                    == Truth::Yes
            {
                conversion.named.push(parameter.name.span);
            }
        }
        self.constructors.push(conversion);
    }
}
