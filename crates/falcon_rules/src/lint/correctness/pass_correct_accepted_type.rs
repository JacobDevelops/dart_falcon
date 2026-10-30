//! Checks invocation arguments against `@AcceptedTypes({String, bool})` metadata.
//!
//! Parameters may declare a union of accepted nominal types even when their Dart
//! type is `Object`. Exact types, subclasses and implementations are accepted.
//! Initializing and super formals inherit annotations from their fields or super parameters.
//! Calls, explicit construction, constructor forwarding and cascade calls use the
//! shared semantic declaration resolver. Missing types or declarations are skipped
//! rather than treating incomplete semantic information as a mismatch.
//!
//! For example, `accept(1)` is reported for
//! `void accept(@AcceptedTypes({String, bool}) Object value) {}`;
//! `accept('text')` and `accept(true)` are valid.

use falcon_analyze::{
    AnalyzeContext, ConstantContext, ConstantValue, DeclarationIdentity, MemberSelector,
    MemberSelectorKind, QualifiedName, ResolvedType, ResolvedTypeView, Rule, RulePreparation,
    SemanticModel, SemanticRuleVisitor, SemanticState, Truth, TypeParameterScope, TypeRelation,
    evaluate_constant, resolve_new_constructor, resolved_constant_type,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use rustc_hash::FxHashMap;

/// Requires invocation arguments to satisfy `AcceptedTypes` annotation unions.
///
/// Metadata is recognized by its `dart_code_metrics_annotations` package identity,
/// and constructor formals inherit field and super-parameter annotations.
#[derive(Default)]
pub struct PassCorrectAcceptedType;

impl Rule for PassCorrectAcceptedType {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
            targets: FxHashMap::default(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
    targets: FxHashMap<DeclarationIdentity, Vec<AcceptedTarget>>,
}

struct AcceptedTarget {
    kind: MemberSelectorKind,
    is_static: bool,
    name: String,
    parameters: Vec<AcceptedParameter>,
}

impl AcceptedTarget {
    fn matches(&self, selector: MemberSelector<'_>) -> bool {
        self.kind == selector.kind
            && self.is_static == selector.is_static
            && self.name == selector.name
    }
}

struct AcceptedParameter {
    position: Option<usize>,
    name: String,
    expected: ResolvedType,
    accepted: Vec<ResolvedType>,
}

/// Borrows the effective member list, including primary constructors.
fn members(declaration: TopLevelDecl<'_>) -> Option<EffectiveMembers<'_>> {
    match declaration {
        TopLevelDecl::Class(node) => Some(node.element_members()),
        TopLevelDecl::MixinClass(node) => Some(node.element_members()),
        TopLevelDecl::Mixin(node) => Some(EffectiveMembers::declared(node.members())),
        TopLevelDecl::Enum(node) => Some(node.element_members()),
        TopLevelDecl::ExtensionType(node) => Some(node.element_members()),
        TopLevelDecl::Extension(node) => Some(EffectiveMembers::declared(node.members())),
        _ => None,
    }
}

/// Selects formals from an already resolved source declaration, not from spelling.
fn parameters<'a>(
    declaration: TopLevelDecl<'a>,
    selector: MemberSelector<'_>,
) -> Option<FormalParamList<'a>> {
    if selector == MemberSelector::DECLARATION {
        if let TopLevelDecl::Function(function) = declaration {
            return function.params();
        }
        return None;
    }
    let mut members = members(declaration)?;
    if selector.kind == MemberSelectorKind::Constructor {
        return members
            .constructors()
            .find(|constructor| {
                constructor
                    .constructor_name
                    .map_or("new", |name| name.name())
                    == selector.name
            })
            .map(|constructor| constructor.params);
    }
    members.declared.find_map(|member| match member {
        ClassMember::Method(method)
            if method.name().name() == selector.name
                && method.is_static() == selector.is_static =>
        {
            Some(method.params())
        }
        _ => None,
    })
}

fn accepted_parameters(
    ctx: &AnalyzeContext<'_>,
    owner: &DeclarationIdentity,
    selector: MemberSelector<'_>,
    state: &SemanticState<'_>,
) -> Vec<AcceptedParameter> {
    inherited_accepted_parameters(ctx, owner, selector, state, 0)
}

fn inherited_accepted_parameters(
    ctx: &AnalyzeContext<'_>,
    owner: &DeclarationIdentity,
    selector: MemberSelector<'_>,
    state: &SemanticState<'_>,
    depth: usize,
) -> Vec<AcceptedParameter> {
    let mut accepted = declared_accepted_parameters(ctx, owner, selector, state);
    if depth >= 32 || selector.kind != MemberSelectorKind::Constructor {
        return accepted;
    }
    let Some((_, _, declaration)) = ctx
        .identities()
        .and_then(|identities| identities.source_declaration(owner))
    else {
        return accepted;
    };
    let Some(mut constructor) = members(declaration).and_then(|members| {
        members.constructors().find(|constructor| {
            constructor
                .constructor_name
                .map_or("new", |name| name.name())
                == selector.name
        })
    }) else {
        return accepted;
    };
    if !constructor
        .params
        .positional()
        .chain(constructor.params.named())
        .any(|parameter| parameter.initializing() == Some(InitializingFormal::Super))
    {
        return accepted;
    }
    let Some(ResolvedTypeView::Interface { identity, .. }) =
        state.signatures.superclass(owner).map(ResolvedType::view)
    else {
        return accepted;
    };
    let name = constructor
        .initializers
        .find_map(|initializer| match initializer {
            ConstructorInitializer::SuperCall(call) => {
                Some(call.call_name().map_or("new", |name| name.name()))
            }
            _ => None,
        })
        .unwrap_or("new");
    let inherited = inherited_accepted_parameters(
        ctx,
        identity,
        MemberSelector::new(MemberSelectorKind::Constructor, false, name),
        state,
        depth + 1,
    );
    let signature = state.signatures.constructor(owner, selector.name);
    let params = constructor.params;
    let mut ordinal = 0;
    for (position, parameter) in params
        .positional()
        .enumerate()
        .map(|(index, parameter)| (Some(index), parameter))
        .chain(params.named().map(|parameter| (None, parameter)))
    {
        if parameter.initializing() != Some(InitializingFormal::Super) {
            continue;
        }
        let super_position = position.map(|_| {
            let index = ordinal;
            ordinal += 1;
            index
        });
        if accepted
            .iter()
            .any(|target| target.name == parameter.name().name())
        {
            continue;
        }
        if let Some(target) = inherited.iter().find(|target| {
            target.position == super_position
                && (super_position.is_some() || target.name == parameter.name().name())
        }) {
            accepted.push(AcceptedParameter {
                position,
                name: parameter.name().name().to_owned(),
                expected: signature
                    .and_then(|signature| match position {
                        Some(index) => signature.positional.get(index),
                        None => signature.named.get(parameter.name().name()),
                    })
                    .cloned()
                    .unwrap_or_else(|| target.expected.clone()),
                accepted: target.accepted.clone(),
            });
        }
    }
    accepted
}

fn declared_accepted_parameters(
    ctx: &AnalyzeContext<'_>,
    owner: &DeclarationIdentity,
    selector: MemberSelector<'_>,
    state: &SemanticState<'_>,
) -> Vec<AcceptedParameter> {
    let Some(identities) = ctx.identities() else {
        return Vec::new();
    };
    let Some((path, parsed, declaration)) = identities.source_declaration(owner) else {
        return Vec::new();
    };
    let Some(parameters) = parameters(declaration, selector) else {
        return Vec::new();
    };
    let model = SemanticModel::new(path, parsed, identities, ctx.types());
    let signature = if selector == MemberSelector::DECLARATION {
        state.signatures.function(owner)
    } else if selector.kind == MemberSelectorKind::Constructor {
        state.signatures.constructor(owner, selector.name)
    } else {
        None
    };
    parameters
        .positional()
        .enumerate()
        .map(|(index, parameter)| (Some(index), parameter))
        .chain(parameters.named().map(|parameter| (None, parameter)))
        .filter_map(|(position, parameter)| {
            let annotation = parameter_annotation(parameter, declaration, model)?;
            let types = annotation.args()?.positional().next()?;
            let mut context = ConstantContext::new(&model, &state.signatures);
            context.owner = Some(owner);
            context.cache = Some(ctx.constant_cache());
            let elements = match evaluate_constant(types, context)? {
                ConstantValue::Set(_, elements) => elements,
                ConstantValue::Map(_, elements) if elements.is_empty() => Vec::new(),
                _ => return None,
            };
            let accepted = elements
                .into_iter()
                .map(|element| {
                    let ConstantValue::Type(accepted) = element else {
                        return None;
                    };
                    Some(resolved_constant_type(&accepted))
                })
                .collect::<Option<Vec<_>>>()?;
            Some(AcceptedParameter {
                position,
                name: parameter.name().name().to_owned(),
                expected: signature
                    .and_then(|signature| match position {
                        Some(index) => signature.positional.get(index),
                        None => signature.named.get(parameter.name().name()),
                    })
                    .cloned()
                    .unwrap_or_else(|| {
                        model.resolve_formal_parameter_in(parameter, &TypeParameterScope::default())
                    }),
                accepted,
            })
        })
        .collect()
}

fn parameter_annotation<'a>(
    parameter: FormalParam<'a>,
    declaration: TopLevelDecl<'a>,
    model: SemanticModel<'_>,
) -> Option<Annotation<'a>> {
    parameter
        .annotations()
        .find(|annotation| accepted_annotation(*annotation, model))
        .or_else(|| {
            if parameter.initializing() != Some(InitializingFormal::This) {
                return None;
            }
            members(declaration)?
                .declared
                .find_map(|member| {
                    let ClassMember::Field(field) = member else {
                        return None;
                    };
                    if !field
                        .declarators()
                        .any(|field| field.name().name() == parameter.name().name())
                    {
                        return None;
                    }
                    field
                        .annotations()
                        .find(|annotation| accepted_annotation(*annotation, model))
                })
                .or_else(|| {
                    members(declaration)?
                        .declaring_params()
                        .find(|field| field.name().name() == parameter.name().name())?
                        .annotations()
                        .find(|annotation| accepted_annotation(*annotation, model))
                })
        })
}

impl Collector<'_> {
    fn arguments(
        &mut self,
        owner: &DeclarationIdentity,
        selector: MemberSelector<'_>,
        args: ArgList<'_>,
        state: &SemanticState<'_>,
    ) {
        if !self
            .targets
            .get(owner)
            .is_some_and(|targets| targets.iter().any(|target| target.matches(selector)))
        {
            let parameters = accepted_parameters(self.ctx, owner, selector, state);
            self.targets
                .entry(owner.clone())
                .or_default()
                .push(AcceptedTarget {
                    kind: selector.kind,
                    is_static: selector.is_static,
                    name: selector.name.to_owned(),
                    parameters,
                });
        }
        let Some(target) = self
            .targets
            .get(owner)
            .and_then(|targets| targets.iter().find(|target| target.matches(selector)))
        else {
            return;
        };
        for parameter in &target.parameters {
            let argument = match parameter.position {
                Some(position) => args.positional().nth(position),
                None => args
                    .named()
                    .find(|argument| argument.name().name() == parameter.name)
                    .map(NamedArg::value),
            };
            let Some(argument) = argument else { continue };
            let actual = state.infer_with_context(argument, Some(&parameter.expected));
            if matches!(
                actual.view(),
                ResolvedTypeView::Unknown | ResolvedTypeView::Dynamic
            ) || parameter.accepted.iter().any(|accepted| {
                state
                    .signatures
                    .relation(&actual, TypeRelation::SubtypeOf(accepted), &state.model)
                    != Truth::No
            }) {
                continue;
            }
            self.diagnostics.push(self.ctx.diagnostic(
                "This type does not match any of the `@AcceptedTypes` types.",
                argument.span(),
            ));
        }
    }

    fn constructor(
        &mut self,
        receiver: &ResolvedType,
        name: &str,
        args: ArgList<'_>,
        state: &SemanticState<'_>,
    ) {
        if let ResolvedTypeView::Interface { identity, .. } = receiver.view() {
            self.arguments(
                identity,
                MemberSelector::new(MemberSelectorKind::Constructor, false, name),
                args,
                state,
            );
        }
    }
}

fn accepted_annotation(annotation: Annotation<'_>, model: SemanticModel<'_>) -> bool {
    QualifiedName::from_identifiers(annotation.name())
        .and_then(|name| model.resolve_name(name))
        .is_some_and(|identity| {
            model.identities().is_package_declaration(
                &model.unaliased(identity),
                "dart_code_metrics_annotations",
                "AcceptedTypes",
            )
        })
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) => {
                if let Some(reference) = state.referenced_declaration(call.callee(), false) {
                    let owner = state.model.unaliased(reference.owner);
                    let mut selector = reference.selector;
                    if selector == MemberSelector::DECLARATION
                        && state.signatures.constructor(&owner, "new").is_some()
                    {
                        selector =
                            MemberSelector::new(MemberSelectorKind::Constructor, false, "new");
                    }
                    self.arguments(&owner, selector, call.args(), state);
                } else if let Expr::DotShorthand(shorthand) = call.callee().unparenthesized()
                    && let Some(receiver) = state.expected()
                {
                    self.constructor(receiver, shorthand.name().name(), call.args(), state);
                }
            }
            Expr::New(construction) => {
                if let Some((owner, name)) = resolve_new_constructor(
                    construction.dart_type(),
                    construction.constructor_name(),
                    &state.model,
                ) {
                    self.arguments(
                        &owner,
                        MemberSelector::new(MemberSelectorKind::Constructor, false, name),
                        construction.args(),
                        state,
                    );
                }
            }
            _ => {}
        }
    }

    fn visit_constructor_ref(&mut self, node: ConstructorRef<'a>, state: &SemanticState<'a>) {
        for initializer in node.initializers {
            match initializer {
                ConstructorInitializer::SuperCall(call) => {
                    if let Some(receiver) = &state.superclass {
                        self.constructor(
                            receiver,
                            call.call_name().map_or("new", |name| name.name()),
                            call.args(),
                            state,
                        );
                    }
                }
                ConstructorInitializer::ThisCall(call) => {
                    if let Some(receiver) = &state.enclosing_type {
                        self.constructor(
                            receiver,
                            call.call_name().map_or("new", |name| name.name()),
                            call.args(),
                            state,
                        );
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'_>,
    ) {
        if let CascadeOp::Call(call) = operation
            && let Some(owner) = state.referenced_member_owner(receiver, call.name().name(), false)
        {
            self.arguments(
                &owner,
                MemberSelector::new(MemberSelectorKind::Read, false, call.name().name()),
                call.args(),
                state,
            );
        }
    }

    fn visit_call_arguments(
        &mut self,
        args: ArgList<'a>,
        _positional: &[ResolvedType],
        _named: &std::collections::HashMap<String, ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        let Some(parent) = args.syntax().parent() else {
            return;
        };
        if let Some(annotation) = Annotation::cast(parent) {
            let names = annotation.name();
            let explicit = annotation.constructor_name();
            let resolve = |name| state.model.resolve_name(name);
            let target = QualifiedName::from_identifiers(names.clone())
                .and_then(resolve)
                .map(|identity| (identity, explicit.map_or("new", |name| name.name())))
                .or_else(|| {
                    if explicit.is_some() || names.clone().count() < 2 {
                        return None;
                    }
                    let constructor = names.clone().last()?;
                    let identity =
                        QualifiedName::from_identifiers(names.clone().take(names.count() - 1))
                            .and_then(resolve)?;
                    Some((identity, constructor.name()))
                });
            if let Some((owner, name)) = target {
                self.arguments(
                    &state.model.unaliased(owner),
                    MemberSelector::new(MemberSelectorKind::Constructor, false, name),
                    args,
                    state,
                );
            }
        } else if let Some(variant) = EnumVariant::cast(parent)
            && let Some(receiver) = &state.enclosing_type
        {
            self.constructor(
                receiver,
                variant.constructor_name().map_or("new", |name| name.name()),
                args,
                state,
            );
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
