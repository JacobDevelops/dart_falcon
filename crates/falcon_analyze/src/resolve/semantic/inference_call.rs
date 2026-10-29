use super::*;
use crate::resolve::semantic::ResolvedTypeKind;

/// Whether a selector/call chain starts with an implicit dot-shorthand receiver.
/// Argument expressions do not establish the receiver's inference context.
pub fn expression_has_dot_shorthand_base(mut expression: Expr<'_>) -> bool {
    loop {
        expression = match expression.unparenthesized() {
            Expr::DotShorthand(_) => return true,
            Expr::Field(node) => node.object(),
            Expr::Call(node) => node.callee(),
            Expr::GenericInstantiation(node) => node.target(),
            _ => return false,
        };
    }
}

/// Whether a null-aware selector (`?.`, `?[`) in `expression`'s own
/// selector chain null-shorts it, which makes its static type nullable
/// (`a?.b.c`, `a?.b!`, `a?[0].c()`). Explicit parentheses end a chain.
pub fn null_shorted(expression: Expr<'_>) -> bool {
    let mut current = expression;
    loop {
        current = match current {
            Expr::Field(node) => {
                if node.is_null_safe() {
                    return true;
                }
                node.object()
            }
            Expr::Index(node) => {
                if node.is_null_safe() {
                    return true;
                }
                node.object()
            }
            Expr::NullAssert(node) => node.operand(),
            Expr::Call(node) => node.callee(),
            Expr::GenericInstantiation(node) => node.target(),
            _ => return false,
        };
    }
}

/// Shared immutable inputs for generic call inference.
pub struct InferenceContext<'a, 'model, 'lookup, F: Fn(&str) -> Option<ResolvedType>> {
    /// Semantic model for type resolution and substitution.
    pub model: &'a SemanticModel<'model>,
    /// Indexed callable signatures.
    pub signatures: &'a SignatureIndex,
    /// Type parameters in lexical scope.
    pub type_parameters: &'a TypeParameterScope,
    /// Resolves unbound identifiers from the caller's lexical context.
    pub lookup: &'lookup F,
    /// Optional exact argument type when ordinary static inference has no
    /// information. Known types, including dynamic, never use this fallback.
    pub argument_fallback: Option<&'lookup crate::constant_value::ArgumentTypeFallback<'lookup>>,
}

pub(super) fn constructed_identity(
    expression: Expr<'_>,
    model: &SemanticModel<'_>,
) -> Option<DeclarationIdentity> {
    match expression.unparenthesized() {
        Expr::New(new) => resolve_new_constructor(new.dart_type(), new.constructor_name(), model)
            .map(|(identity, _)| identity),
        Expr::Call(call) => {
            resolve_call_constructor(call.callee(), model, false).map(|(identity, _)| identity)
        }
        _ => None,
    }
}

pub fn expression_segments<'a>(
    expression: Expr<'a>,
    allow_generic_instantiation: bool,
) -> Option<Vec<&'a str>> {
    let mut current = expression;
    let mut segments = Vec::new();
    loop {
        match current.unparenthesized() {
            Expr::Ident(identifier) => {
                segments.push(identifier.name());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field(field) => {
                segments.push(field.field().name());
                current = field.object();
            }
            Expr::GenericInstantiation(instantiation) if allow_generic_instantiation => {
                current = instantiation.target();
            }
            _ => return None,
        }
    }
}

/// Borrow a declaration qualifier without flattening arbitrary receiver chains.
pub(super) fn expression_qualified_name<'a>(
    expression: Expr<'a>,
    allow_generic_instantiation: bool,
) -> Option<QualifiedName<'a>> {
    let mut current = expression;
    let mut name = None;
    loop {
        match current.unparenthesized() {
            Expr::Ident(head) => {
                return Some(match name {
                    Some(name) => QualifiedName::prefixed(head.name(), name),
                    None => head.name().into(),
                });
            }
            Expr::Field(field) if name.is_none() => {
                name = Some(field.field().name());
                current = field.object();
            }
            Expr::GenericInstantiation(instantiation) if allow_generic_instantiation => {
                current = instantiation.target();
            }
            _ => return None,
        }
    }
}

/// Resolve a constructor call's callee, borrowing its constructor name.
///
/// Constant evaluation permits generic-instantiation wrappers; lexical lint
/// policies can keep rejecting those wrappers by passing `false`.
pub fn resolve_call_constructor<'a>(
    callee: Expr<'a>,
    model: &SemanticModel<'_>,
    allow_generic_instantiation: bool,
) -> Option<(DeclarationIdentity, &'a str)> {
    // A type has at most a prefix and name, followed by an optional constructor.
    let mut segments = [""; 3];
    let mut count = 0;
    let mut current = callee;
    loop {
        match current.unparenthesized() {
            Expr::Ident(identifier) => {
                *segments.get_mut(count)? = identifier.name();
                count += 1;
                break;
            }
            Expr::Field(field) => {
                *segments.get_mut(count)? = field.field().name();
                count += 1;
                current = field.object();
            }
            Expr::GenericInstantiation(instantiation) if allow_generic_instantiation => {
                current = instantiation.target();
            }
            _ => return None,
        }
    }
    segments[..count].reverse();
    if let Some(identity) = model.resolve_dotted(&segments[..count]) {
        return Some((identity, "new"));
    }
    Some((
        model.resolve_dotted(&segments[..count - 1])?,
        segments[count - 1],
    ))
}

/// Resolve an explicit `new`/`const` constructor without assuming a dotted
/// type name denotes an import prefix rather than a named constructor.
pub fn resolve_new_constructor<'a>(
    dart_type: DartType<'a>,
    constructor_name: Option<falcon_syntax::cst::Identifier<'a>>,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str)> {
    let DartType::Named(named) = dart_type else {
        return None;
    };
    if let Some(identity) =
        QualifiedName::from_identifiers(named.segments()).and_then(|name| model.resolve_name(name))
    {
        return Some((identity, constructor_name.map_or("new", |name| name.name())));
    }
    if constructor_name.is_some() {
        return None;
    }
    let mut qualifier = named.segments();
    let constructor = qualifier.next_back()?;
    Some((
        QualifiedName::from_identifiers(qualifier).and_then(|name| model.resolve_name(name))?,
        constructor.name(),
    ))
}

struct SignatureInference<'a, 'context, 'model, 'lookup, F: Fn(&str) -> Option<ResolvedType>> {
    environment: &'a TypeEnvironment,
    signature: &'a ResolvedSignature,
    call_parameters: &'a [TypeParameterId],
    return_type: &'a ResolvedType,
    owner: &'a FxHashMap<TypeParameterId, ResolvedType>,
    context: &'a InferenceContext<'context, 'model, 'lookup, F>,
    substitutions: FxHashMap<TypeParameterId, ResolvedType>,
    inferred: FxHashMap<TypeParameterId, ResolvedType>,
    upper: FxHashMap<TypeParameterId, ResolvedType>,
    partial: FxHashMap<TypeParameterId, ResolvedType>,
    argument_context: FxHashMap<TypeParameterId, ResolvedType>,
}

impl<'a, F: Fn(&str) -> Option<ResolvedType>> SignatureInference<'a, '_, '_, '_, F> {
    fn infer_downward(&mut self, expected: Option<&ResolvedType>) {
        // A dynamic result context supplies no downward type schema. Freezing
        // it here would discard upward constraints from ordinary arguments.
        if let Some(expected) = expected.filter(|ty| {
            !matches!(
                ty,
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic
                }
            )
        }) {
            let mut constraints = TypeConstraintContext {
                parameters: self.call_parameters,
                inferred: &mut self.inferred,
                upper: &mut self.upper,
                signatures: self.context.signatures,
                model: self.context.model,
                poison_unknown: false,
                partial: Some(&mut self.partial),
            };
            infer_type_constraints(
                &self.context.model.substitute(self.return_type, self.owner),
                expected,
                false,
                &mut constraints,
            );
        }
        for id in self.call_parameters {
            if let Some(mut candidate) = inference_candidate(
                self.inferred.get(id),
                self.upper.get(id),
                self.context.signatures,
                self.context.model,
            ) {
                // A result context constrains inference only within the
                // parameter's bound. In particular, `int?` cannot instantiate
                // a parameter bounded by `num`: its downward solution is int.
                // A top-type context meets a declared bound instead.
                let bound = parameter_bound(self.return_type, id)
                    .or_else(|| {
                        self.signature
                            .positional
                            .iter()
                            .find_map(|ty| parameter_bound(ty, id))
                    })
                    .or_else(|| {
                        self.signature
                            .named
                            .values()
                            .find_map(|ty| parameter_bound(ty, id))
                    });
                if let Some(bound) = bound {
                    let bound = self.context.model.substitute(bound, &self.substitutions);
                    if !self.inferred.contains_key(id)
                        && let Some(met) = self.context.signatures.declared_bound_meet(
                            id,
                            &candidate,
                            &bound,
                            self.call_parameters,
                        )
                    {
                        candidate = met;
                    } else if self.context.signatures.relation(
                        &candidate,
                        crate::TypeRelation::SubtypeOf(&bound),
                        self.context.model,
                    ) == Truth::No
                    {
                        let non_null = candidate.non_null();
                        if self.context.signatures.relation(
                            &non_null,
                            crate::TypeRelation::SubtypeOf(&bound),
                            self.context.model,
                        ) == Truth::Yes
                        {
                            candidate = non_null;
                        } else if self.context.signatures.relation(
                            &bound,
                            crate::TypeRelation::SubtypeOf(&candidate),
                            self.context.model,
                        ) == Truth::Yes
                        {
                            candidate = bound;
                        } else {
                            continue;
                        }
                    }
                }
                self.substitutions.insert(id.clone(), candidate);
            }
        }
    }

    fn update_substitutions(&mut self, fixed: &HashSet<TypeParameterId>, include_upper: bool) {
        for id in self.call_parameters {
            if fixed.contains(id) || (!include_upper && !self.inferred.contains_key(id)) {
                continue;
            }
            if let Some(mut candidate) = inference_candidate(
                self.inferred.get(id),
                self.upper.get(id),
                self.context.signatures,
                self.context.model,
            ) {
                if !self.inferred.contains_key(id)
                    && let Some(bound) = parameter_bound(self.return_type, id)
                        .or_else(|| {
                            self.signature
                                .positional
                                .iter()
                                .find_map(|ty| parameter_bound(ty, id))
                        })
                        .or_else(|| {
                            self.signature
                                .named
                                .values()
                                .find_map(|ty| parameter_bound(ty, id))
                        })
                    && let Some(met) = self.context.signatures.declared_bound_meet(
                        id,
                        &candidate,
                        &self.context.model.substitute(bound, &self.substitutions),
                        self.call_parameters,
                    )
                {
                    candidate = met;
                }
                self.argument_context.insert(id.clone(), candidate.clone());
                self.substitutions.insert(id.clone(), candidate);
            }
        }
    }

    fn infer_argument(
        &mut self,
        parameter: &ResolvedType,
        argument: Expr<'_>,
        expected_argument: &ResolvedType,
    ) {
        let formal = self.context.model.substitute(parameter, self.owner);
        // Preserve partial callable contexts without passing unresolved
        // schemas to ordinary expressions.
        let mut actual = self.environment.infer_with_context(
            argument,
            self.context.model,
            self.context.signatures,
            self.context.type_parameters,
            (matches!(
                expected_argument,
                ResolvedType {
                    kind: ResolvedTypeKind::Function { .. }
                }
            ) || !has_inference_holes(expected_argument))
            .then_some(expected_argument),
            self.context.lookup,
        );
        if actual == ResolvedType::UNKNOWN
            && let Some(fallback) = self.context.argument_fallback
            && let Some(ty) = fallback.lookup(argument)
        {
            actual = ty;
        }
        let mut constraints = TypeConstraintContext {
            parameters: self.call_parameters,
            inferred: &mut self.inferred,
            upper: &mut self.upper,
            signatures: self.context.signatures,
            model: self.context.model,
            poison_unknown: true,
            partial: None,
        };
        infer_type_constraints(&formal, &actual, true, &mut constraints);
    }

    fn has_unresolved_inputs(&self, parameter: &ResolvedType, argument: Expr<'_>) -> bool {
        if !self
            .call_parameters
            .iter()
            .any(|id| !self.substitutions.contains_key(id))
        {
            return false;
        }
        let Expr::FuncExpr(function) = argument else {
            return false;
        };
        let ResolvedType {
            kind: ResolvedTypeKind::Function {
                positional, named, ..
            },
        } = parameter
        else {
            return false;
        };
        let depends_on_call = |ty: &ResolvedType| {
            let ty = if self.owner.is_empty() {
                Cow::Borrowed(ty)
            } else {
                Cow::Owned(self.context.model.substitute(ty, self.owner))
            };
            self.call_parameters.iter().any(|id| {
                !self.substitutions.contains_key(id) && mentions_any(&ty, std::slice::from_ref(id))
            })
        };
        let params = function.params();
        params
            .positional()
            .zip(positional.iter())
            .any(|(param, ty)| {
                param.param_type().is_none()
                    && param.function_params().is_none()
                    && depends_on_call(ty)
            })
            || params.named().any(|param| {
                param.param_type().is_none()
                    && param.function_params().is_none()
                    && named
                        .iter()
                        .find(|(name, _)| name == param.name().name())
                        .is_some_and(|(_, ty)| depends_on_call(ty))
            })
    }

    fn infer_stages<'syntax>(
        &mut self,
        args: Option<falcon_syntax::cst::ArgList<'syntax>>,
        deferred: &mut Vec<(&'a ResolvedType, Expr<'syntax>)>,
        fixed: &HashSet<TypeParameterId>,
    ) {
        let signature = self.signature;
        // Each stage uses a frozen context. Independent closure results must
        // join before they contextualize closures that consume those results.
        for closures in [false, true] {
            for (parameter, argument) in signature
                .positional
                .iter()
                .zip(args.into_iter().flat_map(|args| args.positional()))
                .chain(
                    args.into_iter()
                        .flat_map(|args| args.named())
                        .filter_map(|argument| {
                            signature
                                .named
                                .get(argument.name().name())
                                .map(|parameter| (parameter, argument.value()))
                        }),
                )
            {
                if matches!(argument, Expr::FuncExpr(_)) != closures {
                    continue;
                }
                let expected_argument = self
                    .context
                    .model
                    .substitute(parameter, &self.argument_context);
                if closures && self.has_unresolved_inputs(parameter, argument) {
                    if deferred.capacity() == 0 {
                        deferred.reserve(
                            args.map_or(0, |args| args.positional().len() + args.named().len()),
                        );
                    }
                    deferred.push((parameter, argument));
                    continue;
                }
                self.infer_argument(parameter, argument, &expected_argument);
            }
            self.update_substitutions(fixed, false);
        }
    }

    fn infer_deferred(
        &mut self,
        deferred: &mut Vec<(&ResolvedType, Expr<'_>)>,
        mut argument_contexts: Option<&mut HashMap<usize, ResolvedType>>,
        fixed: &HashSet<TypeParameterId>,
    ) {
        let mut complete_unresolved = false;
        while !deferred.is_empty() {
            let remaining = deferred.len();
            deferred.retain(|&(parameter, argument)| {
                let mut expected_argument = self
                    .context
                    .model
                    .substitute(parameter, &self.argument_context);
                let unresolved = self.has_unresolved_inputs(parameter, argument);
                if unresolved && !complete_unresolved {
                    return true;
                }
                if unresolved {
                    let ResolvedType {
                        kind: ResolvedTypeKind::Function { return_type, .. },
                    } = &expected_argument
                    else {
                        unreachable!()
                    };
                    let return_type = Arc::clone(return_type);
                    // Cycles and callbacks without producers complete only
                    // missing input variables to top, not the return schema.
                    for id in self.call_parameters {
                        if !self.substitutions.contains_key(id) {
                            self.argument_context
                                .insert(id.clone(), self.context.model.core_type("Object", true));
                        }
                    }
                    expected_argument = self
                        .context
                        .model
                        .substitute(parameter, &self.argument_context);
                    if let ResolvedType {
                        kind:
                            ResolvedTypeKind::Function {
                                return_type: result,
                                ..
                            },
                    } = &mut expected_argument
                    {
                        *result = return_type;
                    }
                    for id in self.call_parameters {
                        if !self.substitutions.contains_key(id) {
                            self.argument_context
                                .insert(id.clone(), ResolvedType::UNKNOWN);
                        }
                    }
                    if let Some(contexts) = argument_contexts.as_deref_mut() {
                        contexts.insert(argument.span().start, expected_argument.clone());
                    }
                }
                self.infer_argument(parameter, argument, &expected_argument);
                false
            });
            self.update_substitutions(fixed, false);
            complete_unresolved = deferred.len() == remaining;
        }
    }

    fn complete_defaults(&mut self) {
        for id in self.call_parameters {
            if self.substitutions.contains_key(id) {
                continue;
            }
            let default = if self.inferred.contains_key(id) || self.upper.contains_key(id) {
                ResolvedType::UNKNOWN
            } else if let Some(schema) = self.partial.remove(id) {
                schema
            } else if let Some(default) = self.context.signatures.owner_defaults.get(id) {
                self.context.model.substitute(default, &self.substitutions)
            } else if self.context.signatures.bound_declared.get(id) == Some(&false) {
                ResolvedType::DYNAMIC
            } else {
                parameter_bound(self.return_type, id)
                    .or_else(|| {
                        self.signature
                            .positional
                            .iter()
                            .find_map(|ty| parameter_bound(ty, id))
                    })
                    .or_else(|| {
                        self.signature
                            .named
                            .values()
                            .find_map(|ty| parameter_bound(ty, id))
                    })
                    .map(|bound| self.context.model.substitute(bound, &self.substitutions))
                    .unwrap_or(ResolvedType::UNKNOWN)
            };
            self.substitutions.entry(id.clone()).or_insert(default);
        }
    }
}

impl TypeEnvironment {
    /// Solves generic call parameters; owner and explicit substitutions are fixed.
    pub fn infer_signature_substitutions<'syntax, F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        signature: &ResolvedSignature,
        args: impl Into<Option<falcon_syntax::cst::ArgList<'syntax>>>,
        type_args: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        owner: &FxHashMap<TypeParameterId, ResolvedType>,
    ) -> FxHashMap<TypeParameterId, ResolvedType> {
        self.infer_signature_substitutions_with_result(
            (
                signature,
                &signature.call_parameters,
                &signature.return_type,
                owner,
            ),
            args,
            type_args,
            context,
            expected,
            None,
        )
    }

    /// Constructors infer their owner's parameters and instance result while
    /// borrowing the declared parameter types and default-value metadata.
    /// When requested, records generic closures' downward function contexts by
    /// expression offset, before upward inference narrows the call's type arguments.
    pub fn infer_signature_substitutions_with_result<
        'syntax,
        F: Fn(&str) -> Option<ResolvedType>,
    >(
        &self,
        signature: (
            &ResolvedSignature,
            &[TypeParameterId],
            &ResolvedType,
            &FxHashMap<TypeParameterId, ResolvedType>,
        ),
        args: impl Into<Option<falcon_syntax::cst::ArgList<'syntax>>>,
        type_args: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        argument_contexts: Option<&mut HashMap<usize, ResolvedType>>,
    ) -> FxHashMap<TypeParameterId, ResolvedType> {
        let args = args.into();
        let (signature, call_parameters, return_type, owner) = signature;
        let mut substitutions = owner.clone();
        if call_parameters.is_empty() {
            return substitutions;
        }
        if !type_args.is_empty() {
            substitutions.extend(call_parameters.iter().enumerate().map(|(index, id)| {
                (
                    id.clone(),
                    type_args
                        .clone()
                        .nth(index)
                        .map_or(ResolvedType::UNKNOWN, |ty| {
                            context.model.resolve_type_in(ty, context.type_parameters)
                        }),
                )
            }));
            return substitutions;
        }
        let mut inference = SignatureInference {
            environment: self,
            signature,
            call_parameters,
            return_type,
            owner,
            context,
            substitutions,
            inferred: FxHashMap::with_capacity_and_hasher(
                call_parameters.len(),
                Default::default(),
            ),
            upper: FxHashMap::with_capacity_and_hasher(call_parameters.len(), Default::default()),
            partial: FxHashMap::default(),
            argument_context: FxHashMap::default(),
        };
        inference.infer_downward(expected);
        let fixed = inference
            .substitutions
            .iter()
            .filter(|(_, ty)| !has_inference_holes(ty))
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();
        inference.argument_context = inference.substitutions.clone();
        for id in call_parameters {
            inference
                .argument_context
                .entry(id.clone())
                .or_insert(ResolvedType::UNKNOWN);
        }
        let mut deferred = Vec::new();
        inference.infer_stages(args, &mut deferred, &fixed);
        inference.infer_deferred(&mut deferred, argument_contexts, &fixed);
        // Pure contravariant evidence constrains the final solution, but must
        // not supply horizontal input contexts to sibling closures.
        inference.update_substitutions(&fixed, true);
        inference.complete_defaults();
        inference.substitutions
    }

    pub(super) fn infer_constructor<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        type_args: AstChildren<'_, DartType<'_>>,
        args: falcon_syntax::cst::ArgList<'_>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        if let Some(alias) = context.model.identities.type_alias(&identity) {
            if !type_args.is_empty() {
                let arguments: Vec<_> = type_args
                    .clone()
                    .map(|ty| context.model.resolve_type_in(ty, context.type_parameters))
                    .collect();
                return context
                    .model
                    .expand_alias(&identity, alias, &arguments, false, None)
                    .with_nullable(false);
            }
            return self.infer_alias_constructor(alias, name, args, context, expected);
        }
        let extension_type =
            context.signatures.type_kind(&identity) == Some(TypeKind::ExtensionType);
        let Some((parameters, _)) = context
            .signatures
            .type_facts
            .get(&identity)
            .and_then(|facts| facts.supertypes.as_ref())
        else {
            return ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity,
                    arguments: type_args
                        .clone()
                        .map(|ty| context.model.resolve_type_in(ty, context.type_parameters))
                        .collect(),
                    nullable: false,
                    extension_type,
                },
            };
        };
        let empty = ResolvedSignature::returning(ResolvedType::UNKNOWN);
        let signature = context
            .signatures
            .constructor(&identity, name)
            .unwrap_or(&empty);
        let return_type =
            context
                .signatures
                .self_instantiation(identity, parameters, context.model);
        let substitutions = self.infer_signature_substitutions_with_result(
            (signature, parameters, &return_type, &FxHashMap::default()),
            args,
            type_args,
            context,
            expected,
            None,
        );
        context.model.substitute(&return_type, &substitutions)
    }

    fn infer_alias_constructor<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        alias: &TypeAlias,
        name: &str,
        args: falcon_syntax::cst::ArgList<'_>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        // Without explicit arguments the alias's own parameters are the
        // inference variables: constructor parameters are rewritten from
        // the class's parameters into the alias's instantiation of them.
        let Some(template) = context.model.alias_template(alias) else {
            return ResolvedType::UNKNOWN;
        };
        let (alias_parameters, aliased) = template.as_ref();
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity: class,
                    arguments: class_arguments,
                    ..
                },
        } = &aliased
        else {
            return ResolvedType::UNKNOWN;
        };
        let Some((class_parameters, _)) = context
            .signatures
            .type_facts
            .get(class)
            .and_then(|facts| facts.supertypes.as_ref())
        else {
            return aliased.with_nullable(false);
        };
        let class_substitutions =
            context
                .signatures
                .owner_substitutions(class_parameters, class_arguments, None);
        let Some(constructor) = context.signatures.constructor(class, name) else {
            return if name == "new" {
                let defaults = alias_parameters
                    .iter()
                    .map(|(_, id, default)| (id.clone(), default.clone()))
                    .collect();
                context
                    .model
                    .substitute(aliased, &defaults)
                    .with_nullable(false)
            } else {
                ResolvedType::UNKNOWN
            };
        };
        let mut signature = constructor.clone();
        for ty in signature
            .positional
            .iter_mut()
            .chain(signature.named.values_mut())
        {
            *ty = context.model.substitute(ty, &class_substitutions);
        }
        signature.owner_parameters = Vec::new();
        signature.call_parameters = alias_parameters
            .iter()
            .map(|(_, id, _)| id.clone())
            .collect();
        signature.return_type = aliased.with_nullable(false);
        let substitutions = self.infer_signature_substitutions(
            &signature,
            args,
            AstChildren::empty(),
            context,
            expected,
            &FxHashMap::default(),
        );
        context
            .model
            .substitute(&signature.return_type, &substitutions)
    }

    pub fn infer_with_signatures(
        &self,
        expr: Expr<'_>,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        type_parameters: &TypeParameterScope,
    ) -> ResolvedType {
        self.infer_with_signatures_using_lookup(expr, model, signatures, type_parameters, &|_| None)
    }

    /// [`Self::infer_with_signatures`], using `lookup` for names without a local binding.
    pub fn infer_with_signatures_using_lookup(
        &self,
        expr: Expr<'_>,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        type_parameters: &TypeParameterScope,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> ResolvedType {
        self.infer_with_context(expr, model, signatures, type_parameters, None, lookup)
    }

    /// The static type of the receiver `object` of the member access
    /// `object.name`; `super` is the supertype through which `name` resolves.
    /// Shorting does not erase nullability declared by a preceding member.
    pub(super) fn receiver_type(
        &self,
        object: Expr<'_>,
        name: &str,
        context: &InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        if matches!(object, Expr::Super(_)) {
            context
                .signatures
                .super_receiver(&self.receiver(), name, context.model)
                .unwrap_or(ResolvedType::UNKNOWN)
        } else if matches!(
            object,
            Expr::Field(_) | Expr::Index(_) | Expr::NullAssert(_)
        ) {
            let (ty, reached) =
                self.infer_selector_chain(object, context, expected, |_| None, |_, _, _| {});
            reached.unwrap_or(ty)
        } else {
            let inference = queries::context_copy(context);
            if null_shorted(object) {
                self.infer_unshorted(object, inference, expected, None)
            } else {
                self.infer_in_context(object, inference, expected, None)
            }
        }
    }

    /// The method an unqualified call `name(…)` invokes through implicit
    /// `this`: a member the enclosing type declares (instance or static), or
    /// an inherited one no library or imported declaration shadows.
    pub(crate) fn implicit_this_method<'a>(
        &self,
        name: &str,
        model: &SemanticModel<'_>,
        signatures: &'a SignatureIndex,
    ) -> Option<(
        &'a ResolvedSignature,
        FxHashMap<TypeParameterId, ResolvedType>,
    )> {
        let this = self.receiver();
        let mut bound = &this;
        while let ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { bound: next, .. },
        }
        | ResolvedType {
            kind: ResolvedTypeKind::NonNullTypeParameter { bound: next, .. },
        } = bound
        {
            bound = next;
        }
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = bound
        else {
            return None;
        };
        let declared = signatures.declared_member_facts(identity, name);
        if declared.is_empty()
            && (model.resolve_value(name).is_some() || model.resolve_name(name).is_some())
        {
            return None;
        }
        if declared.iter().any(|facts| facts.is_static)
            && let Some(signature) = signatures.member(identity, name)
        {
            return Some((signature, FxHashMap::default()));
        }
        signatures.resolved_member(&this, name, model)
    }

    /// The type of `object.name` when `object` names a library prefix or a
    /// type rather than a value: a prefixed top-level variable or getter, an
    /// enum value, or a static field or getter.
    pub(super) fn static_access_type(
        &self,
        object: Expr<'_>,
        name: &str,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> Option<ResolvedType> {
        let qualifier = expression_qualified_name(object, false)?;
        let segments = [qualifier.prefix.unwrap_or(qualifier.name), qualifier.name];
        let len = if qualifier.prefix.is_some() { 2 } else { 1 };
        self.static_access_segments(&segments[..len], name, model, signatures, lookup)
    }

    pub(super) fn static_access_segments(
        &self,
        segments: &[&str],
        name: &str,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> Option<ResolvedType> {
        let head = segments.first()?;
        if self.is_bound(head) || lookup(head).is_some() {
            return None;
        }
        if let [prefix] = segments
            && let Some(identity) = model.resolve_value(QualifiedName::prefixed(prefix, name))
        {
            return signatures.value_type(&identity).cloned();
        }
        let identity = model
            .resolve_dotted(segments)
            .or_else(|| model.resolve_extension(QualifiedName::from_segments(segments)?))?;
        if signatures.is_enum_value(&identity, name) {
            return Some(ResolvedType::interface_identity(identity));
        }
        signatures
            .declared_member_facts(&identity, name)
            .iter()
            .any(|facts| {
                facts.is_static
                    && matches!(facts.kind, MemberKind::Field { .. } | MemberKind::Getter)
            })
            .then(|| signatures.field(&identity, name))
            .flatten()
            .map(|(_, ty)| ty.clone())
            .filter(|ty| *ty != ResolvedType::UNKNOWN)
    }

    pub(super) fn tearoff_type<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        signature: &ResolvedSignature,
        owner: &FxHashMap<TypeParameterId, ResolvedType>,
        explicit: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        let function = context.model.substitute(&signature.function_type(), owner);
        if signature.call_parameters.is_empty() {
            return function;
        }
        let mut substitutions = owner.clone();
        substitutions.reserve(signature.call_parameters.len());
        if !explicit.is_empty() {
            substitutions.extend(
                signature
                    .call_parameters
                    .iter()
                    .zip(explicit)
                    .map(|(id, ty)| {
                        (
                            id.clone(),
                            context.model.resolve_type_in(ty, context.type_parameters),
                        )
                    }),
            );
        } else if let Some(
            expected @ ResolvedType {
                kind: ResolvedTypeKind::Function { .. },
            },
        ) = expected
        {
            let mut lower = FxHashMap::with_capacity_and_hasher(
                signature.call_parameters.len(),
                Default::default(),
            );
            let mut upper = FxHashMap::with_capacity_and_hasher(
                signature.call_parameters.len(),
                Default::default(),
            );
            let mut partial = FxHashMap::default();
            infer_type_constraints(
                &function,
                expected,
                false,
                &mut TypeConstraintContext {
                    parameters: &signature.call_parameters,
                    inferred: &mut lower,
                    upper: &mut upper,
                    signatures: context.signatures,
                    model: context.model,
                    poison_unknown: false,
                    partial: Some(&mut partial),
                },
            );
            for id in &signature.call_parameters {
                let bound = || {
                    parameter_bound(&function, id)
                        .map(|bound| context.model.substitute(bound, &substitutions))
                };
                let ty = match inference_candidate(
                    lower.get(id),
                    upper.get(id),
                    context.signatures,
                    context.model,
                ) {
                    Some(candidate) if !lower.contains_key(id) => bound()
                        .and_then(|bound| {
                            context.signatures.declared_bound_meet(
                                id,
                                &candidate,
                                &bound,
                                &signature.call_parameters,
                            )
                        })
                        .unwrap_or(candidate),
                    Some(candidate) => candidate,
                    None => partial.remove(id).unwrap_or_else(|| {
                        // Truly unconstrained parameters use their declared
                        // bound, or dynamic when no bound was written.
                        if context.signatures.bound_declared.get(id) == Some(&false) {
                            ResolvedType::DYNAMIC
                        } else {
                            bound().unwrap_or(ResolvedType::UNKNOWN)
                        }
                    }),
                };
                substitutions.insert(id.clone(), ty);
            }
        } else {
            return function;
        }
        context.model.substitute(&function, &substitutions)
    }

    pub(super) fn constructor_tearoff<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        explicit: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
    ) -> Option<ResolvedType> {
        let (parameters, result, class, owner) = if let Some(alias) =
            context.model.identities.type_alias(&identity)
        {
            let template = context.model.alias_template(alias)?;
            let (parameters, result) = template.as_ref();
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity: class,
                        arguments,
                        ..
                    },
            } = &result
            else {
                return None;
            };
            let owner = context.signatures.owner_substitutions(
                context.signatures.type_parameters(class)?,
                arguments,
                None,
            );
            (
                parameters.iter().map(|(_, id, _)| id.clone()).collect(),
                result.clone(),
                class.clone(),
                owner,
            )
        } else {
            let parameters = context.signatures.type_parameters(&identity)?.to_vec();
            let result =
                context
                    .signatures
                    .self_instantiation(identity.clone(), &parameters, context.model);
            (parameters, result, identity, FxHashMap::default())
        };
        let constructor = context.signatures.constructor(&class, name);
        if !context.signatures.has_constructor_tearoff(&class, name) {
            return None;
        }
        let signature = ResolvedSignature {
            shape: constructor.map_or_else(
                || Some(FunctionTypeShape::empty()),
                |signature| signature.substituted_shape(context.model, &owner),
            ),
            call_parameters: parameters,
            optional_positional: constructor.map_or(0, |signature| signature.optional_positional),
            positional: constructor.map_or_else(Vec::new, |signature| {
                signature
                    .positional
                    .iter()
                    .map(|ty| context.model.substitute(ty, &owner))
                    .collect()
            }),
            named: constructor.map_or_else(HashMap::new, |signature| {
                signature
                    .named
                    .iter()
                    .map(|(name, ty)| (name.clone(), context.model.substitute(ty, &owner)))
                    .collect()
            }),
            positional_parameter_names: constructor.map_or_else(Vec::new, |signature| {
                signature.positional_parameter_names.clone()
            }),
            ..ResolvedSignature::returning(result)
        };
        Some(self.tearoff_type(
            &signature,
            &FxHashMap::default(),
            explicit,
            context,
            expected,
        ))
    }

    pub(super) fn callable_tearoff<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        target: Expr<'_>,
        explicit: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        cached_receiver: &mut Option<ResolvedType>,
    ) -> Option<ResolvedType> {
        if let Some((identity, name)) = resolve_call_constructor(target, context.model, true)
            && self.invocation_unshadowed(target, context)
            && matches!(target, Expr::Field(_))
            && expression_segments(target, true)
                .is_some_and(|segments| context.model.resolve_dotted(&segments).is_none())
            && let Some(ty) =
                self.constructor_tearoff(identity, name, explicit.clone(), context, expected)
        {
            return Some(ty);
        }
        let (signature, owner) =
            self.invocation_declaration(target, context, cached_receiver, None, false)?;
        Some(self.tearoff_type(signature, &owner, explicit, context, expected))
    }
}

pub(super) struct TypeConstraintContext<'a, 'model> {
    pub(super) parameters: &'a [TypeParameterId],
    pub(super) inferred: &'a mut FxHashMap<TypeParameterId, ResolvedType>,
    pub(super) upper: &'a mut FxHashMap<TypeParameterId, ResolvedType>,
    pub(super) signatures: &'a SignatureIndex,
    pub(super) model: &'a SemanticModel<'model>,
    /// Unknown argument evidence survives joins; Unknown downward schema slots
    /// instead mean that no constraint has been supplied yet.
    pub(super) poison_unknown: bool,
    /// Partial contextual schemas are fallbacks, not upward constraints.
    pub(super) partial: Option<&'a mut FxHashMap<TypeParameterId, ResolvedType>>,
}

pub(super) fn infer_type_constraints(
    formal: &ResolvedType,
    actual: &ResolvedType,
    covariant: bool,
    context: &mut TypeConstraintContext<'_, '_>,
) {
    if matches!(
        actual,
        ResolvedType {
            kind: ResolvedTypeKind::Never
        }
    ) {
        return;
    }
    if matches!(
        actual,
        ResolvedType {
            kind: ResolvedTypeKind::Unknown
        }
    ) {
        if !context.poison_unknown {
            return;
        }
        let mut parameters = Vec::new();
        collect_parameter_ids(formal, &mut parameters);
        for id in parameters {
            if context.parameters.contains(&id) {
                context.inferred.insert(id.clone(), ResolvedType::UNKNOWN);
                context.upper.insert(id, ResolvedType::UNKNOWN);
            }
        }
        return;
    }
    match formal {
        ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { id, nullable, .. },
        } if context.parameters.contains(id) => {
            infer_parameter_constraints(id, *nullable, actual, covariant, context);
        }
        ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } => {
            infer_interface_constraints(formal, identity, actual, covariant, context);
        }
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    alias,
                    return_type,
                    positional,
                    named,
                    ..
                },
        } => {
            infer_function_constraints(
                alias,
                return_type,
                positional,
                named,
                actual,
                covariant,
                context,
            );
        }
        ResolvedType {
            kind:
                ResolvedTypeKind::Record {
                    occurrence,
                    positional,
                    named,
                    ..
                },
        } => {
            infer_record_constraints(occurrence, positional, named, actual, covariant, context);
        }
        _ => {}
    }
}

fn infer_parameter_constraints(
    id: &TypeParameterId,
    nullable: bool,
    actual: &ResolvedType,
    covariant: bool,
    context: &mut TypeConstraintContext<'_, '_>,
) {
    if matches!(actual, ResolvedType { kind: ResolvedTypeKind::TypeParameter { id: other, .. } } if context.parameters.contains(other))
    {
        return;
    }
    let actual = if nullable
        && !matches!(
            actual,
            ResolvedType {
                kind: ResolvedTypeKind::Null
            }
        ) {
        actual.with_nullable(false)
    } else {
        actual.clone()
    };
    // A partial schema cannot fix a whole variable before argument
    // inference, but remains useful when no argument constrains it.
    if !context.poison_unknown && has_inference_holes(&actual) {
        if let Some(partial) = context.partial.as_deref_mut() {
            partial
                .entry(id.clone())
                .and_modify(|previous| {
                    if *previous != actual {
                        *previous = ResolvedType::UNKNOWN;
                    }
                })
                .or_insert(actual);
        }
        return;
    }
    let candidates = if covariant {
        &mut *context.inferred
    } else {
        &mut *context.upper
    };
    let ty = candidates.get(id).map_or_else(
        || actual.clone(),
        |previous| {
            if matches!(
                previous,
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown
                }
            ) {
                return ResolvedType::UNKNOWN;
            }
            let joined = inference_join(previous, &actual, context.signatures, context.model);
            if covariant {
                joined
            } else if joined == *previous {
                actual.clone()
            } else if joined == actual {
                previous.clone()
            } else {
                ResolvedType::UNKNOWN
            }
        },
    );
    candidates.insert(id.clone(), ty);
}

fn infer_interface_constraints(
    formal: &ResolvedType,
    identity: &DeclarationIdentity,
    actual: &ResolvedType,
    covariant: bool,
    context: &mut TypeConstraintContext<'_, '_>,
) {
    // A raw result context still constrains each type argument to its
    // instantiate-to-bound default. Projecting only the formal type
    // would otherwise zip its arguments against an empty raw schema.
    let actual = if covariant {
        Cow::Borrowed(actual)
    } else {
        context
            .signatures
            .normalized_raw_type(Cow::Borrowed(actual))
    };
    let actual = actual.as_ref();
    if !covariant
        && actual.interface("dart:async", "FutureOr")
        && !identity.is_sdk("dart:async", "FutureOr")
        && let Some(argument) = actual.type_argument(0)
    {
        let argument = argument.with_nullable(argument.nullable() || actual.nullable());
        if let Some(future) =
            context
                .signatures
                .instantiated_supertype(formal, "dart:async", "Future", context.model)
        {
            if let Some(formal_argument) = future.type_argument(0) {
                infer_type_constraints(&formal_argument, &argument, false, context);
            }
        } else {
            infer_type_constraints(formal, &argument, false, context);
        }
        return;
    }
    let aligned = if covariant {
        context
            .signatures
            .instantiated_identity(actual, identity, context.model)
    } else if let ResolvedType {
        kind: ResolvedTypeKind::Interface { identity, .. },
    } = actual
    {
        context
            .signatures
            .instantiated_identity(formal, identity, context.model)
    } else {
        None
    };
    if let Some(aligned) = aligned {
        let (formal_arguments, actual_arguments) = if covariant {
            (formal, &aligned)
        } else {
            (&aligned, actual)
        };
        for (formal, actual) in formal_arguments
            .type_arguments()
            .zip(actual_arguments.type_arguments())
        {
            infer_type_constraints(&formal, &actual, covariant, context);
        }
    } else if identity.is_sdk("dart:async", "FutureOr")
        && let Some(formal_argument) = formal.type_argument(0)
    {
        let actual_argument = if actual.interface("dart:async", "Future") {
            actual.type_argument(0).unwrap_or_else(|| actual.clone())
        } else {
            actual.clone()
        };
        let actual_argument =
            actual_argument.with_nullable(actual_argument.nullable() || actual.nullable());
        infer_type_constraints(&formal_argument, &actual_argument, covariant, context);
    }
}

fn infer_function_constraints(
    formal_alias: &FunctionTypeAlias,
    return_type: &ResolvedType,
    positional: &[ResolvedType],
    named: &[(String, ResolvedType)],
    actual: &ResolvedType,
    covariant: bool,
    context: &mut TypeConstraintContext<'_, '_>,
) {
    if let ResolvedType {
        kind:
            ResolvedTypeKind::Function {
                alias: actual_alias,
                return_type: actual_return,
                positional: actual_positional,
                named: actual_named,
                ..
            },
    } = actual
    {
        infer_type_constraints(
            &return_type.clone().projected_from(formal_alias),
            &actual_return.as_ref().clone().projected_from(actual_alias),
            covariant,
            context,
        );
        for (formal, actual) in positional.iter().zip(actual_positional.iter()) {
            infer_type_constraints(
                &formal.clone().projected_from(formal_alias),
                &actual.clone().projected_from(actual_alias),
                !covariant,
                context,
            );
        }
        for (name, formal) in named.iter() {
            if let Some((_, actual)) = actual_named
                .iter()
                .find(|(actual_name, _)| actual_name == name)
            {
                infer_type_constraints(
                    &formal.clone().projected_from(formal_alias),
                    &actual.clone().projected_from(actual_alias),
                    !covariant,
                    context,
                );
            }
        }
    }
}

fn infer_record_constraints(
    formal_occurrence: &FunctionTypeAlias,
    positional: &[ResolvedType],
    named: &[(String, ResolvedType)],
    actual: &ResolvedType,
    covariant: bool,
    context: &mut TypeConstraintContext<'_, '_>,
) {
    if let ResolvedType {
        kind:
            ResolvedTypeKind::Record {
                occurrence: actual_occurrence,
                positional: actual_positional,
                named: actual_named,
                ..
            },
    } = actual
    {
        for (formal, actual) in positional.iter().zip(actual_positional.iter()) {
            infer_type_constraints(
                &formal.clone().projected_from(formal_occurrence),
                &actual.clone().projected_from(actual_occurrence),
                covariant,
                context,
            );
        }
        for (name, formal) in named.iter() {
            if let Some((_, actual)) = actual_named
                .iter()
                .find(|(actual_name, _)| actual_name == name)
            {
                infer_type_constraints(
                    &formal.clone().projected_from(formal_occurrence),
                    &actual.clone().projected_from(actual_occurrence),
                    covariant,
                    context,
                );
            }
        }
    }
}

pub(super) fn inference_candidate(
    lower: Option<&ResolvedType>,
    upper: Option<&ResolvedType>,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> Option<ResolvedType> {
    match (lower, upper) {
        (Some(lower), Some(upper))
            if matches!(
                upper,
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown
                }
            ) || inference_join(lower, upper, signatures, model) != *upper =>
        {
            Some(ResolvedType::UNKNOWN)
        }
        (Some(lower), _) => Some(lower.clone()),
        (_, Some(upper)) => Some(upper.clone()),
        _ => None,
    }
}
