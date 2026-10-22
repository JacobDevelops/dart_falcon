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

use falcon_analyze::{
    AnalyzeContext, LanguageVersion, ResolvedTypeView, Rule, RulePreparation, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UseSuperParameters;

impl Rule for UseSuperParameters {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if ctx.language_version() < LanguageVersion::new(2, 17) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let collector = Collector {
            ctx,
            references: HashMap::new(),
            constructors: Vec::new(),
        };
        RulePreparation::Semantic(Box::new(collector))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
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

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let collector = *self;
        let ctx = collector.ctx;
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
                    ctx.diagnostic("Use `super` parameters instead of forwarding parameters to `super`.", constructor.anchor)
                })
            })
            .collect()
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Ident(identifier) = node
            && let Some(binding) = state.environment.binding_declaration(identifier.name())
            && let Some((body, used)) = self.references.get_mut(&binding)
            && body.contains_offset(identifier.span().start)
        {
            *used = true;
        }
    }

    fn visit_constructor_ref(&mut self, ctor: ConstructorRef<'a>, state: &SemanticState<'a>) {
        let Some((args, name)) = ctor.initializers.clone().rev().find_map(|init| match init {
            ConstructorInitializer::SuperCall(call) => Some((
                call.args(),
                call.call_name().map_or("new", |name| name.name()),
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
                        |name| name.span().end,
                    ),
                )
            } else {
                Span::new(
                    ctor.name_span.start,
                    ctor.constructor_name
                        .map_or(ctor.name_span.end, |name| name.span().end),
                )
            },
            positional: Vec::with_capacity(ctor.params.params().len()),
            named: Vec::with_capacity(args.named().len()),
        };
        // Every positional super argument must be a distinct positional formal,
        // in declaration order. Other constructor parameters may precede it.
        if !args.positional().is_empty()
            && args.positional().all(|arg| matches!(arg, Expr::Ident(_)))
        {
            let mut parameters_by_name = HashMap::with_capacity(ctor.params.params().len());
            for (index, parameter) in ctor.params.params().enumerate() {
                parameters_by_name
                    .entry(parameter.name().name())
                    .or_insert((index, parameter));
            }
            let mut parameters = ctor.params.params();
            let mut seen = HashSet::with_capacity(args.positional().len());
            let mut previous = 0;
            for argument in args.positional() {
                let Expr::Ident(identifier) = argument else {
                    unreachable!()
                };
                let Some(&(index, parameter)) = parameters_by_name.get(identifier.name()) else {
                    conversion.positional.push(PositionalCheck::Invalid);
                    break;
                };
                if matches!(parameter.kind(), ParamKind::Named { .. }) || !seen.insert(index) {
                    conversion.positional.push(PositionalCheck::Invalid);
                    break;
                }
                if index < previous {
                    conversion.positional.push(PositionalCheck::Blocked);
                    break;
                }
                for parameter in parameters.by_ref().take(index + 1 - previous) {
                    conversion.positional.push(match parameter.initializing() {
                        Some(InitializingFormal::Super) => PositionalCheck::Invalid,
                        Some(InitializingFormal::This) => PositionalCheck::Blocked,
                        None => PositionalCheck::Reference(parameter.name().span()),
                    });
                }
                previous = index + 1;
            }
        }
        for parameter in ctor.params.named() {
            if parameter.initializing() == Some(InitializingFormal::This)
                || (ctor.is_primary && (parameter.is_final() || parameter.is_var()))
            {
                continue;
            }
            let Some(super_type) = signature.named.get(parameter.name().name()) else {
                continue;
            };
            if !args.named().any(|argument| {
                argument.name().name() == parameter.name().name()
                    && matches!(argument.value(), Expr::Ident(identifier) if identifier.name() == parameter.name().name())
            }) { continue; }
            let super_type = state.model.substitute(super_type, &substitutions);
            let this_type = state.environment.declared_type(parameter.name().name());
            if matches!(super_type.view(), ResolvedTypeView::Dynamic)
                || matches!(this_type.view(), ResolvedTypeView::Dynamic)
                || state
                    .signatures
                    .relation(&super_type, TypeRelation::SubtypeOf(&this_type), &state.model)
                    == Truth::Yes
            {
                conversion.named.push(parameter.name().span());
            }
        }
        if let Some(body) = ctor.body {
            for parameter in conversion
                .positional
                .iter()
                .filter_map(|check| match check {
                    PositionalCheck::Reference(parameter) => Some(*parameter),
                    _ => None,
                })
                .chain(conversion.named.iter().copied())
            {
                self.references
                    .entry(parameter)
                    .or_insert((body.span(), false));
            }
        }
        self.constructors.push(conversion);
    }
}
