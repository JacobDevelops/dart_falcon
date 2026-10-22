use super::*;

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

type ArgumentTypeFallback<'a> = dyn Fn(Expr<'_>) -> Option<ResolvedType> + 'a;

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
    pub argument_fallback: Option<&'lookup ArgumentTypeFallback<'lookup>>,
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

impl TypeEnvironment {
    /// Solves generic call parameters; owner and explicit substitutions are fixed.
    pub fn infer_signature_substitutions<'syntax, F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        signature: &ResolvedSignature,
        args: impl Into<Option<falcon_syntax::cst::ArgList<'syntax>>>,
        type_args: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        owner: &HashMap<TypeParameterId, ResolvedType>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
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
            &HashMap<TypeParameterId, ResolvedType>,
        ),
        args: impl Into<Option<falcon_syntax::cst::ArgList<'syntax>>>,
        type_args: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        mut argument_contexts: Option<&mut HashMap<usize, ResolvedType>>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
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
                        .map_or(ResolvedType::Unknown, |ty| {
                            context.model.resolve_type_in(ty, context.type_parameters)
                        }),
                )
            }));
            return substitutions;
        }
        let mut inferred = HashMap::with_capacity(call_parameters.len());
        let mut upper = HashMap::with_capacity(call_parameters.len());
        let mut partial = HashMap::new();
        // A dynamic result context supplies no downward type schema. Freezing
        // it here would discard upward constraints from ordinary arguments.
        if let Some(expected) = expected.filter(|ty| !matches!(ty, ResolvedType::Dynamic)) {
            let mut constraints = TypeConstraintContext {
                parameters: call_parameters,
                inferred: &mut inferred,
                upper: &mut upper,
                signatures: context.signatures,
                model: context.model,
                poison_unknown: false,
                partial: Some(&mut partial),
            };
            infer_type_constraints(
                &context.model.substitute(return_type, owner),
                expected,
                false,
                &mut constraints,
            );
        }
        for id in call_parameters {
            if let Some(mut candidate) = inference_candidate(
                inferred.get(id),
                upper.get(id),
                context.signatures,
                context.model,
            ) {
                // A result context constrains inference only within the
                // parameter's bound. In particular, `int?` cannot instantiate
                // a parameter bounded by `num`: its downward solution is int.
                // A top-type context meets a declared bound instead.
                let bound = parameter_bound(return_type, id)
                    .or_else(|| {
                        signature
                            .positional
                            .iter()
                            .find_map(|ty| parameter_bound(ty, id))
                    })
                    .or_else(|| {
                        signature
                            .named
                            .values()
                            .find_map(|ty| parameter_bound(ty, id))
                    });
                if let Some(bound) = bound {
                    let bound = context.model.substitute(bound, &substitutions);
                    if !inferred.contains_key(id)
                        && let Some(met) = context.signatures.declared_bound_meet(
                            id,
                            &candidate,
                            &bound,
                            call_parameters,
                        )
                    {
                        candidate = met;
                    } else if context
                        .signatures
                        .subtype(&candidate, &bound, context.model)
                        == Truth::No
                    {
                        let non_null = candidate.non_null();
                        if context.signatures.subtype(&non_null, &bound, context.model)
                            == Truth::Yes
                        {
                            candidate = non_null;
                        } else if context
                            .signatures
                            .subtype(&bound, &candidate, context.model)
                            == Truth::Yes
                        {
                            candidate = bound;
                        } else {
                            continue;
                        }
                    }
                }
                substitutions.insert(id.clone(), candidate);
            }
        }
        let fixed = substitutions
            .iter()
            .filter(|(_, ty)| !has_inference_holes(ty))
            .map(|(id, _)| id.clone())
            .collect::<HashSet<_>>();
        let update_substitutions =
            |substitutions: &mut HashMap<TypeParameterId, ResolvedType>,
             argument_context: &mut HashMap<TypeParameterId, ResolvedType>,
             inferred: &HashMap<TypeParameterId, ResolvedType>,
             upper: &HashMap<TypeParameterId, ResolvedType>,
             include_upper: bool| {
                for id in call_parameters {
                    if fixed.contains(id) || (!include_upper && !inferred.contains_key(id)) {
                        continue;
                    }
                    if let Some(mut candidate) = inference_candidate(
                        inferred.get(id),
                        upper.get(id),
                        context.signatures,
                        context.model,
                    ) {
                        if !inferred.contains_key(id)
                            && let Some(bound) = parameter_bound(return_type, id)
                                .or_else(|| {
                                    signature
                                        .positional
                                        .iter()
                                        .find_map(|ty| parameter_bound(ty, id))
                                })
                                .or_else(|| {
                                    signature
                                        .named
                                        .values()
                                        .find_map(|ty| parameter_bound(ty, id))
                                })
                            && let Some(met) = context.signatures.declared_bound_meet(
                                id,
                                &candidate,
                                &context.model.substitute(bound, substitutions),
                                call_parameters,
                            )
                        {
                            candidate = met;
                        }
                        argument_context.insert(id.clone(), candidate.clone());
                        substitutions.insert(id.clone(), candidate);
                    }
                }
            };
        let infer_argument =
            |parameter: &ResolvedType,
             argument: Expr<'_>,
             expected_argument: &ResolvedType,
             inferred: &mut HashMap<TypeParameterId, ResolvedType>,
             upper: &mut HashMap<TypeParameterId, ResolvedType>| {
                let formal = context.model.substitute(parameter, owner);
                // Preserve partial callable contexts without passing unresolved
                // schemas to ordinary expressions.
                let mut actual = self.infer_with_context(
                    argument,
                    context.model,
                    context.signatures,
                    context.type_parameters,
                    (matches!(expected_argument, ResolvedType::Function { .. })
                        || !has_inference_holes(expected_argument))
                    .then_some(expected_argument),
                    context.lookup,
                );
                if actual == ResolvedType::Unknown
                    && let Some(fallback) = context.argument_fallback
                    && let Some(ty) = fallback(argument)
                {
                    actual = ty;
                }
                let mut constraints = TypeConstraintContext {
                    parameters: call_parameters,
                    inferred,
                    upper,
                    signatures: context.signatures,
                    model: context.model,
                    poison_unknown: true,
                    partial: None,
                };
                infer_type_constraints(&formal, &actual, true, &mut constraints);
            };
        let has_unresolved_inputs =
            |parameter: &ResolvedType,
             argument: Expr<'_>,
             substitutions: &HashMap<TypeParameterId, ResolvedType>| {
                if !call_parameters
                    .iter()
                    .any(|id| !substitutions.contains_key(id))
                {
                    return false;
                }
                let Expr::FuncExpr(function) = argument else {
                    return false;
                };
                let ResolvedType::Function {
                    positional, named, ..
                } = parameter
                else {
                    return false;
                };
                let depends_on_call = |ty: &ResolvedType| {
                    let ty = if owner.is_empty() {
                        Cow::Borrowed(ty)
                    } else {
                        Cow::Owned(context.model.substitute(ty, owner))
                    };
                    call_parameters.iter().any(|id| {
                        !substitutions.contains_key(id)
                            && mentions_any(&ty, std::slice::from_ref(id))
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
            };
        let mut argument_context = substitutions.clone();
        for id in call_parameters {
            argument_context
                .entry(id.clone())
                .or_insert(ResolvedType::Unknown);
        }
        let mut deferred = Vec::new();
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
                let expected_argument = context.model.substitute(parameter, &argument_context);
                if closures && has_unresolved_inputs(parameter, argument, &substitutions) {
                    if deferred.capacity() == 0 {
                        deferred.reserve(
                            args.map_or(0, |args| args.positional().len() + args.named().len()),
                        );
                    }
                    deferred.push((parameter, argument));
                    continue;
                }
                infer_argument(
                    parameter,
                    argument,
                    &expected_argument,
                    &mut inferred,
                    &mut upper,
                );
            }
            update_substitutions(
                &mut substitutions,
                &mut argument_context,
                &inferred,
                &upper,
                false,
            );
        }
        let mut complete_unresolved = false;
        while !deferred.is_empty() {
            let remaining = deferred.len();
            deferred.retain(|&(parameter, argument)| {
                let mut expected_argument = context.model.substitute(parameter, &argument_context);
                let unresolved = has_unresolved_inputs(parameter, argument, &substitutions);
                if unresolved && !complete_unresolved {
                    return true;
                }
                if unresolved {
                    let ResolvedType::Function { return_type, .. } = &expected_argument else {
                        unreachable!()
                    };
                    let return_type = Arc::clone(return_type);
                    // Cycles and callbacks without producers complete only
                    // missing input variables to top, not the return schema.
                    for id in call_parameters {
                        if !substitutions.contains_key(id) {
                            argument_context
                                .insert(id.clone(), context.model.core_type("Object", true));
                        }
                    }
                    expected_argument = context.model.substitute(parameter, &argument_context);
                    if let ResolvedType::Function {
                        return_type: result,
                        ..
                    } = &mut expected_argument
                    {
                        *result = return_type;
                    }
                    for id in call_parameters {
                        if !substitutions.contains_key(id) {
                            argument_context.insert(id.clone(), ResolvedType::Unknown);
                        }
                    }
                    if let Some(contexts) = argument_contexts.as_deref_mut() {
                        contexts.insert(argument.span().start, expected_argument.clone());
                    }
                }
                infer_argument(
                    parameter,
                    argument,
                    &expected_argument,
                    &mut inferred,
                    &mut upper,
                );
                false
            });
            update_substitutions(
                &mut substitutions,
                &mut argument_context,
                &inferred,
                &upper,
                false,
            );
            complete_unresolved = deferred.len() == remaining;
        }
        // Pure contravariant evidence constrains the final solution, but must
        // not supply horizontal input contexts to sibling closures.
        update_substitutions(
            &mut substitutions,
            &mut argument_context,
            &inferred,
            &upper,
            true,
        );
        for id in call_parameters {
            if substitutions.contains_key(id) {
                continue;
            }
            let default = if inferred.contains_key(id) || upper.contains_key(id) {
                ResolvedType::Unknown
            } else if let Some(schema) = partial.remove(id) {
                schema
            } else if let Some(default) = context.signatures.owner_defaults.get(id) {
                context.model.substitute(default, &substitutions)
            } else if context.signatures.bound_declared.get(id) == Some(&false) {
                ResolvedType::Dynamic
            } else {
                parameter_bound(return_type, id)
                    .or_else(|| {
                        signature
                            .positional
                            .iter()
                            .find_map(|ty| parameter_bound(ty, id))
                    })
                    .or_else(|| {
                        signature
                            .named
                            .values()
                            .find_map(|ty| parameter_bound(ty, id))
                    })
                    .map(|bound| context.model.substitute(bound, &substitutions))
                    .unwrap_or(ResolvedType::Unknown)
            };
            substitutions.entry(id.clone()).or_insert(default);
        }
        substitutions
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
            // Without explicit arguments the alias's own parameters are the
            // inference variables: constructor parameters are rewritten from
            // the class's parameters into the alias's instantiation of them.
            let Some(template) = context.model.alias_template(alias) else {
                return ResolvedType::Unknown;
            };
            let (alias_parameters, aliased) = template.as_ref();
            let ResolvedType::Interface {
                identity: class,
                arguments: class_arguments,
                ..
            } = &aliased
            else {
                return ResolvedType::Unknown;
            };
            let Some((class_parameters, _)) = context.signatures.supertypes.get(class) else {
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
                    ResolvedType::Unknown
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
                &HashMap::new(),
            );
            return context
                .model
                .substitute(&signature.return_type, &substitutions);
        }
        let extension_type =
            context.signatures.type_kind(&identity) == Some(TypeKind::ExtensionType);
        let Some((parameters, _)) = context.signatures.supertypes.get(&identity) else {
            return ResolvedType::Interface {
                occurrence: Default::default(),
                identity,
                arguments: type_args
                    .clone()
                    .map(|ty| context.model.resolve_type_in(ty, context.type_parameters))
                    .collect(),
                nullable: false,
                extension_type,
            };
        };
        let empty = ResolvedSignature::returning(ResolvedType::Unknown);
        let signature = context
            .signatures
            .constructor(&identity, name)
            .unwrap_or(&empty);
        let return_type =
            context
                .signatures
                .self_instantiation(identity, parameters, context.model);
        let substitutions = self.infer_signature_substitutions_with_result(
            (signature, parameters, &return_type, &HashMap::new()),
            args,
            type_args,
            context,
            expected,
            None,
        );
        context.model.substitute(&return_type, &substitutions)
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
                .super_receiver(&self.lookup("this"), name, context.model)
                .unwrap_or(ResolvedType::Unknown)
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
        HashMap<TypeParameterId, ResolvedType>,
    )> {
        let this = self.lookup("this");
        let mut bound = &this;
        while let ResolvedType::TypeParameter { bound: next, .. }
        | ResolvedType::NonNullTypeParameter { bound: next, .. } = bound
        {
            bound = next;
        }
        let ResolvedType::Interface { identity, .. } = bound else {
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
            return Some((signature, HashMap::new()));
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
        let segments = expression_segments(object, false)?;
        self.static_access_segments(&segments, name, model, signatures, lookup)
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
            .filter(|ty| *ty != ResolvedType::Unknown)
    }

    pub(super) fn tearoff_type<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        signature: &ResolvedSignature,
        owner: &HashMap<TypeParameterId, ResolvedType>,
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
        } else if let Some(expected @ ResolvedType::Function { .. }) = expected {
            let mut lower = HashMap::with_capacity(signature.call_parameters.len());
            let mut upper = HashMap::with_capacity(signature.call_parameters.len());
            let mut partial = HashMap::new();
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
                            ResolvedType::Dynamic
                        } else {
                            bound().unwrap_or(ResolvedType::Unknown)
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
            let ResolvedType::Interface {
                identity: class,
                arguments,
                ..
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
            (parameters, result, identity, HashMap::new())
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
        Some(self.tearoff_type(&signature, &HashMap::new(), explicit, context, expected))
    }

    pub(super) fn callable_tearoff<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        target: Expr<'_>,
        explicit: AstChildren<'_, DartType<'_>>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        cached_receiver: &mut Option<ResolvedType>,
    ) -> Option<ResolvedType> {
        if self.invocation_unshadowed(target, context)
            && matches!(target, Expr::Field(_))
            && expression_segments(target, true)
                .is_some_and(|segments| context.model.resolve_dotted(&segments).is_none())
            && let Some((identity, name)) = resolve_call_constructor(target, context.model, true)
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
    pub(super) inferred: &'a mut HashMap<TypeParameterId, ResolvedType>,
    pub(super) upper: &'a mut HashMap<TypeParameterId, ResolvedType>,
    pub(super) signatures: &'a SignatureIndex,
    pub(super) model: &'a SemanticModel<'model>,
    /// Unknown argument evidence survives joins; Unknown downward schema slots
    /// instead mean that no constraint has been supplied yet.
    pub(super) poison_unknown: bool,
    /// Partial contextual schemas are fallbacks, not upward constraints.
    pub(super) partial: Option<&'a mut HashMap<TypeParameterId, ResolvedType>>,
}

pub(super) fn infer_type_constraints(
    formal: &ResolvedType,
    actual: &ResolvedType,
    covariant: bool,
    context: &mut TypeConstraintContext<'_, '_>,
) {
    if matches!(actual, ResolvedType::Never) {
        return;
    }
    if matches!(actual, ResolvedType::Unknown) {
        if !context.poison_unknown {
            return;
        }
        let mut parameters = Vec::new();
        collect_parameter_ids(formal, &mut parameters);
        for id in parameters {
            if context.parameters.contains(&id) {
                context.inferred.insert(id.clone(), ResolvedType::Unknown);
                context.upper.insert(id, ResolvedType::Unknown);
            }
        }
        return;
    }
    match formal {
        ResolvedType::TypeParameter { id, nullable, .. } if context.parameters.contains(id) => {
            if matches!(actual, ResolvedType::TypeParameter { id: other, .. } if context.parameters.contains(other))
            {
                return;
            }
            let actual = if *nullable && !matches!(actual, ResolvedType::Null) {
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
                                *previous = ResolvedType::Unknown;
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
                    if matches!(previous, ResolvedType::Unknown) {
                        return ResolvedType::Unknown;
                    }
                    let joined =
                        inference_join(previous, &actual, context.signatures, context.model);
                    if covariant {
                        joined
                    } else if joined == *previous {
                        actual.clone()
                    } else if joined == actual {
                        previous.clone()
                    } else {
                        ResolvedType::Unknown
                    }
                },
            );
            candidates.insert(id.clone(), ty);
        }
        ResolvedType::Interface { identity, .. } => {
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
                if let Some(future) = context.signatures.instantiated_supertype(
                    formal,
                    "dart:async",
                    "Future",
                    context.model,
                ) {
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
            } else if let ResolvedType::Interface { identity, .. } = actual {
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
        ResolvedType::Function {
            alias: formal_alias,
            return_type,
            positional,
            named,
            ..
        } => {
            if let ResolvedType::Function {
                alias: actual_alias,
                return_type: actual_return,
                positional: actual_positional,
                named: actual_named,
                ..
            } = actual
            {
                infer_type_constraints(
                    &return_type.as_ref().clone().projected_from(formal_alias),
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
        ResolvedType::Record {
            occurrence: formal_occurrence,
            positional,
            named,
            ..
        } => {
            if let ResolvedType::Record {
                occurrence: actual_occurrence,
                positional: actual_positional,
                named: actual_named,
                ..
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
        _ => {}
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
            if matches!(upper, ResolvedType::Unknown)
                || inference_join(lower, upper, signatures, model) != *upper =>
        {
            Some(ResolvedType::Unknown)
        }
        (Some(lower), _) => Some(lower.clone()),
        (_, Some(upper)) => Some(upper.clone()),
        _ => None,
    }
}
