//! Invocation metadata is resolved before null-chain result lifting.

use super::*;

/// A callable declaration and its receiver substitutions. Indexed signatures
/// remain borrowed; only function-typed values need a synthesized signature.
pub struct ResolvedInvocation<'signature, 'tree> {
    pub signature: Cow<'signature, ResolvedSignature>,
    pub owner: FxHashMap<TypeParameterId, ResolvedType>,
    pub type_args: AstChildren<'tree, DartType<'tree>>,
    pub constructor_result: Option<ResolvedType>,
}

impl ResolvedInvocation<'_, '_> {
    /// Constructors infer owner parameters against their instantiated result;
    /// other invocations infer the callable's own type parameters.
    pub fn inference_signature(
        &self,
    ) -> (
        &ResolvedSignature,
        &[TypeParameterId],
        &ResolvedType,
        &FxHashMap<TypeParameterId, ResolvedType>,
    ) {
        let signature = self.signature.as_ref();
        match &self.constructor_result {
            Some(result) => (signature, &signature.owner_parameters, result, &self.owner),
            None => (
                signature,
                &signature.call_parameters,
                &signature.return_type,
                &self.owner,
            ),
        }
    }
}

pub(super) fn context_copy<'a, 'model, 'lookup, F: Fn(&str) -> Option<ResolvedType>>(
    context: &InferenceContext<'a, 'model, 'lookup, F>,
) -> InferenceContext<'a, 'model, 'lookup, F> {
    InferenceContext {
        model: context.model,
        signatures: context.signatures,
        type_parameters: context.type_parameters,
        lookup: context.lookup,
        argument_fallback: context.argument_fallback,
    }
}

impl TypeEnvironment {
    /// Infers an invocation or member read, retaining the nullable result of a
    /// null-shorted chain. Receiver lookup uses the reached branch internally.
    pub fn infer_invocation<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        expression: Expr<'_>,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        self.infer_in_context(expression, context_copy(context), expected, None)
    }

    /// The reached receiver of an access, before shorted null is added. Member
    /// nullability remains intact; parentheses end null shortening. `name`
    /// selects the correct superclass/mixin receiver for `super` dispatch.
    pub fn infer_access_receiver<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        object: Expr<'_>,
        name: &str,
        is_null_safe: bool,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        let receiver = self.receiver_type(object, name, context, expected);
        if is_null_safe {
            receiver.non_null()
        } else {
            receiver
        }
    }
    pub fn named_extension(
        &self,
        expression: Expr<'_>,
        context: &InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
    ) -> Option<DeclarationIdentity> {
        if !self.invocation_unshadowed(expression, context) {
            return None;
        }
        context
            .model
            .resolve_extension(QualifiedName::from_segments(&expression_segments(
                expression, true,
            )?)?)
    }

    pub(super) fn invocation_unshadowed(
        &self,
        expression: Expr<'_>,
        context: &InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
    ) -> bool {
        expression_segments(expression, true).is_some_and(|segments| {
            segments.first().is_some_and(|head| {
                !self.is_bound(head)
                    && (context.lookup)(head).is_none()
                    && !matches!(self.receiver(),
                        ResolvedType::Interface { identity, .. }
                        if context.signatures.declared_member_facts(&identity, head)
                            .iter().any(|facts| facts.kind == MemberKind::Method))
            })
        })
    }

    pub(super) fn invocation_declaration<'a, 'lookup, F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        target: Expr<'_>,
        context: &InferenceContext<'a, '_, 'lookup, F>,
        reached_receiver: &mut Option<ResolvedType>,
        expected: Option<&ResolvedType>,
        include_override: bool,
    ) -> Option<(
        &'a ResolvedSignature,
        FxHashMap<TypeParameterId, ResolvedType>,
    )> {
        let model = context.model;
        let signatures = context.signatures;
        match target {
            Expr::Ident(identifier)
                if !self.is_bound(identifier.name())
                    && (context.lookup)(identifier.name()).is_none() =>
            {
                self.implicit_this_method(identifier.name(), model, signatures)
                    .or_else(|| {
                        model
                            .resolve_value(identifier.name())
                            .and_then(|identity| signatures.function(&identity))
                            .map(|signature| (signature, FxHashMap::default()))
                    })
            }
            Expr::Field(access) => {
                let object = access.object();
                let field = access.field();
                let is_null_safe = access.is_null_safe();
                let extension = if include_override
                    && let Expr::Call(call) = object.unparenthesized()
                    && call.args().named().is_empty()
                    && call.args().positional().len() == 1
                {
                    let receiver = call.args().positional().next().unwrap();
                    let (callee, type_args) = match call.callee().unparenthesized() {
                        Expr::GenericInstantiation(instantiation) => {
                            (instantiation.target(), instantiation.type_args())
                        }
                        callee => (callee, call.type_args()),
                    };
                    self.named_extension(callee, context).and_then(|identity| {
                        let receiver = self.infer_invocation(receiver, context, None);
                        let receiver = if is_null_safe {
                            receiver.non_null()
                        } else {
                            receiver
                        };
                        let arguments: Vec<_> = type_args
                            .map(|ty| model.resolve_type_in(ty, context.type_parameters))
                            .collect();
                        signatures.resolved_extension_override_method(
                            &identity,
                            &receiver,
                            &arguments,
                            field.name(),
                            model,
                        )
                    })
                } else {
                    None
                };
                extension.or_else(|| {
                    if reached_receiver.as_ref().is_none_or(|ty| {
                        matches!(ty, ResolvedType::Unknown) || ty.interface("dart:core", "Type")
                    }) && let Some(segments) = expression_segments(object, true)
                        && let [prefix] = segments.as_slice()
                        && !self.is_bound(prefix)
                        && (context.lookup)(prefix).is_none()
                        && let Some(identity) =
                            model.resolve_value(QualifiedName::prefixed(prefix, field.name()))
                        && let Some(signature) = signatures.function(&identity)
                    {
                        return Some((signature, FxHashMap::default()));
                    }
                    let receiver = reached_receiver.get_or_insert_with(|| {
                        self.receiver_type(
                            object,
                            field.name(),
                            context,
                            expected.filter(|_| expression_has_dot_shorthand_base(object)),
                        )
                    });
                    let non_null;
                    let receiver = if is_null_safe && receiver.nullable() {
                        non_null = receiver.non_null();
                        &non_null
                    } else {
                        &*receiver
                    };
                    if receiver.record_field(field.name()).is_some() {
                        return None;
                    }
                    signatures
                        .resolved_member(receiver, field.name(), model)
                        .or_else(|| {
                            if !self.invocation_unshadowed(object, context) {
                                return None;
                            }
                            let segments = expression_segments(object, true)?;
                            let identity = model.resolve_dotted(&segments).or_else(|| {
                                model.resolve_extension(QualifiedName::from_segments(&segments)?)
                            })?;
                            Some((
                                signatures.member(&identity, field.name())?,
                                FxHashMap::default(),
                            ))
                        })
                })
            }
            _ => None,
        }
    }

    /// Resolves contextual parameter metadata without lifting the signature's
    /// return type for a null-shorted call. Locals shadow own members, which
    /// shadow library declarations, which shadow inherited members.
    pub fn resolved_invocation<'signature, 'tree, 'lookup, F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        callee: Expr<'tree>,
        type_args: AstChildren<'tree, DartType<'tree>>,
        context: &InferenceContext<'signature, '_, 'lookup, F>,
    ) -> Option<ResolvedInvocation<'signature, 'tree>> {
        self.resolved_invocation_in(callee, type_args, context, None, &mut None, true)
            .ok()
    }

    // Err carries the final result when parameter inference is unnecessary.
    pub(super) fn resolved_invocation_in<
        'signature,
        'tree,
        'lookup,
        F: Fn(&str) -> Option<ResolvedType>,
    >(
        &self,
        mut target: Expr<'tree>,
        mut explicit: AstChildren<'tree, DartType<'tree>>,
        context: &InferenceContext<'signature, '_, 'lookup, F>,
        expected: Option<&ResolvedType>,
        reached_receiver: &mut Option<ResolvedType>,
        metadata: bool,
    ) -> Result<ResolvedInvocation<'signature, 'tree>, ResolvedType> {
        let mut callee_type = None;
        let original_target = target;
        loop {
            if let Expr::GenericInstantiation(instantiation) = target {
                target = instantiation.target();
                explicit = instantiation.type_args();
            }
            let Expr::Field(access) = target else {
                break;
            };
            let object = access.object();
            if access.field().name() != "call" {
                break;
            }
            let receiver = callee_type
                .take()
                .or_else(|| reached_receiver.take())
                .unwrap_or_else(|| self.receiver_type(object, "call", context, None));
            if !matches!(receiver, ResolvedType::Function { .. }) {
                *reached_receiver = Some(receiver);
                break;
            }
            callee_type = Some(receiver);
            target = object;
        }
        if let Expr::Field(access) = target
            && let Expr::GenericInstantiation(instantiation) = access.object()
        {
            explicit = instantiation.type_args();
        }
        let model = context.model;
        let signatures = context.signatures;
        let resolved =
            self.invocation_declaration(target, context, reached_receiver, expected, true);
        let constructor = ((metadata || target.syntax() != original_target.syntax())
            && resolved.is_none()
            && self.invocation_unshadowed(target, context))
        .then(|| resolve_call_constructor(target, model, true))
        .flatten()
        .filter(|(identity, name)| signatures.constructor(identity, name).is_some());
        let resolved = resolved.or_else(|| {
            let (identity, name) = constructor.as_ref()?;
            Some((
                signatures.constructor(identity, name)?,
                FxHashMap::default(),
            ))
        });
        let (signature, owner) = if let Some((signature, owner)) = resolved {
            (Cow::Borrowed(signature), owner)
        } else {
            let receiver = callee_type.unwrap_or_else(|| {
                self.infer_unshorted(target, context_copy(context), None, reached_receiver.take())
            });
            if let ResolvedType::Function {
                positional,
                named,
                shape,
                optional_positional,
                ..
            } = &receiver
            {
                let return_type = receiver
                    .function_return_type()
                    .ok_or(ResolvedType::Unknown)?;
                let local_generic = matches!(target, Expr::Ident(identifier) if self.is_local_function(identifier.name()));
                // Non-generic function values need no parameter metadata.
                // Generic values still infer their binders from this call.
                if !metadata
                    && !local_generic
                    && shape
                        .as_ref()
                        .is_none_or(|shape| shape.type_parameters.is_empty())
                {
                    return Err(return_type);
                }
                let mut signature = ResolvedSignature {
                    shape: shape.clone(),
                    optional_positional: *optional_positional,
                    positional: positional.to_vec(),
                    named: named.iter().cloned().collect(),
                    ..ResolvedSignature::returning(return_type)
                };
                if local_generic || shape.is_some() {
                    signature.bind_local_type_parameters(context.type_parameters);
                }
                (Cow::Owned(signature), FxHashMap::default())
            } else {
                let (signature, owner) = signatures
                    .resolved_member(&receiver, "call", model)
                    .ok_or({
                        if matches!(receiver, ResolvedType::Dynamic)
                            || receiver.interface("dart:core", "Function")
                        {
                            ResolvedType::Dynamic
                        } else {
                            ResolvedType::Unknown
                        }
                    })?;
                (Cow::Borrowed(signature), owner)
            }
        };
        let constructor_result = constructor.map(|(identity, _)| {
            signatures.self_instantiation(identity, &signature.owner_parameters, model)
        });
        Ok(ResolvedInvocation {
            signature,
            owner,
            type_args: explicit,
            constructor_result,
        })
    }
}

impl SignatureIndex {
    /// Whether an instance field or accessor has an instantiated element rather
    /// than the declaration's canonical variable element.
    pub fn member_access_is_substituted(
        &self,
        receiver: &ResolvedType,
        name: &str,
        write: bool,
        model: &SemanticModel<'_>,
    ) -> bool {
        let resolved = if write {
            self.resolved_writable_field(receiver, name, model)
        } else {
            self.resolved_field(receiver, name, model)
        };
        resolved.is_some_and(|(ty, substitutions)| {
            SubstitutionMemo::default().is_relevant(&ty, &substitutions, false)
        })
    }

    /// Reads an instantiated field or getter, excluding method tear-offs.
    pub fn instantiated_data_read(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        receiver.record_field(name).or_else(|| {
            self.resolved_field(receiver, name, model)
                .or_else(|| self.resolved_extension_getter(receiver, name, model))
                .map(|(ty, substitutions)| model.substitute(&ty, &substitutions))
        })
    }

    /// Reads a member on an already instantiated receiver, without a syntax
    /// expression or contextual generic tear-off instantiation.
    pub fn instantiated_member_read(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.instantiated_data_read(receiver, name, model)
            .or_else(|| {
                self.resolved_member(receiver, name, model)
                    .map(|(signature, substitutions)| {
                        model.substitute(&signature.function_type(), &substitutions)
                    })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::IdentitySource;
    use falcon_dart_parser::parse;

    fn arrow_expression(body: Option<FunctionBody<'_>>) -> Expr<'_> {
        let Some(FunctionBody::Arrow(arrow)) = body else {
            panic!()
        };
        arrow.expr()
    }

    fn with_model(
        source: &str,
        test: impl FnOnce(Program<'_>, &SemanticModel<'_>, &SignatureIndex),
    ) {
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        let parsed = Arc::new(crate::ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        assert!(errors.is_empty(), "{errors:?}");
        let path = Path::new("/project/queries.dart");
        let identities = IdentityIndex::build(
            &[IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[source],
            &[],
            None,
        );
        let model = SemanticModel::new(path, &parsed, &identities, None);
        let signatures = SignatureIndex::from_program(program, &model);
        test(program, &model, &signatures);
    }

    #[test]
    fn null_chain_query_stops_at_parentheses() {
        let cases = [
            ("a?.b.c()", true),
            ("a?[0].c()", true),
            ("a?.b!", true),
            ("a?.b<int>", true),
            ("(a?.b).c()", false),
            ("f(a?.b)", false),
        ];
        for (expression, expected) in cases {
            let source = format!("dynamic probe(dynamic a) => {expression};");
            let falcon_dart_parser::ParseResult { tree, errors } = parse(&source);
            let program = tree.program(&source);
            assert!(errors.is_empty(), "{expression}: {errors:?}");
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!()
            };
            let body = arrow_expression(function.body());
            assert_eq!(null_shorted(body), expected, "{expression}");
        }
    }

    #[test]
    fn contextual_signature_precedes_null_chain_lifting() {
        with_model(
            "class Token {} class Box<T> { T apply(T value) => value; }
             dynamic probe(Box<Token>? box, Token value) => box?.apply(value);",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(2).unwrap()
                else {
                    panic!()
                };
                let body = arrow_expression(function.body());
                let Expr::Call(call) = body else { panic!() };
                let callee = call.callee();
                let type_args = call.type_args();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let context = InferenceContext {
                    model,
                    signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                let invocation = environment
                    .resolved_invocation(callee, type_args, &context)
                    .unwrap();
                assert!(matches!(invocation.signature, Cow::Borrowed(_)));
                let parameter =
                    model.substitute(&invocation.signature.positional[0], &invocation.owner);
                assert_eq!(
                    parameter,
                    ResolvedType::interface_identity(model.resolve_name("Token").unwrap())
                );
                assert!(!parameter.nullable());
                let result = environment.infer_invocation(body, &context, None);
                assert_eq!(result, parameter.with_nullable(true));
            },
        );
    }

    #[test]
    fn reached_receiver_retains_declared_nullability() {
        with_model(
            "class Leaf {} class Box { Leaf? get nullable => null; Leaf get present => Leaf(); }
             dynamic nullable(Box? box) => box?.nullable;
             dynamic present(Box? box) => box?.present;
             dynamic grouped(Box? box) => (box?.present);",
            |program, model, signatures| {
                let scope = TypeParameterScope::default();
                for declaration in program.declarations().skip(2) {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let body = arrow_expression(function.body());
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let context = InferenceContext {
                        model,
                        signatures,
                        type_parameters: &scope,
                        lookup: &|_| None,
                        argument_fallback: None,
                    };
                    let reached =
                        environment.infer_access_receiver(body, "member", false, &context, None);
                    assert_eq!(reached.nullable(), function.name().name() != "present");
                    assert_eq!(
                        reached.non_null(),
                        ResolvedType::interface_identity(model.resolve_name("Leaf").unwrap())
                    );
                }
            },
        );
    }

    #[test]
    fn member_reads_substitute_inherited_owner_arguments() {
        with_model(
            "class Token {}
             class Base<T> {
               T field;
               T get value => throw 0;
               T method(T argument) => argument;
             }
             class Middle<U> extends Base<U> {}
             class Child<V> extends Middle<V> {}
             dynamic probe(Child<Token> child) => child;",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(4).unwrap()
                else {
                    panic!()
                };
                let mut environment = TypeEnvironment::new();
                environment.bind_params(
                    function.params().unwrap(),
                    model,
                    &TypeParameterScope::default(),
                );
                let receiver = environment.lookup("child");
                let token = ResolvedType::interface_identity(model.resolve_name("Token").unwrap());
                for name in ["field", "value"] {
                    assert_eq!(
                        signatures.instantiated_member_read(&receiver, name, model),
                        Some(token.clone()),
                        "{name}"
                    );
                }
                let Some(ResolvedType::Function {
                    return_type,
                    positional,
                    ..
                }) = signatures.instantiated_member_read(&receiver, "method", model)
                else {
                    panic!("expected a method tear-off")
                };
                assert_eq!(return_type.as_ref(), &token);
                assert_eq!(positional.as_ref(), &[token]);
                assert_eq!(
                    signatures.instantiated_member_read(&receiver, "missing", model),
                    None
                );
            },
        );
    }

    #[test]
    fn member_reads_resolve_record_fields() {
        with_model(
            "class Token {}
             dynamic probe((Token, {Token value}) record) => record;",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap()
                else {
                    panic!()
                };
                let mut environment = TypeEnvironment::new();
                environment.bind_params(
                    function.params().unwrap(),
                    model,
                    &TypeParameterScope::default(),
                );
                let receiver = environment.lookup("record");
                let token = ResolvedType::interface_identity(model.resolve_name("Token").unwrap());
                for name in ["$1", "value"] {
                    assert_eq!(
                        signatures.instantiated_member_read(&receiver, name, model),
                        Some(token.clone()),
                        "{name}"
                    );
                }
                assert_eq!(
                    signatures.instantiated_member_read(&receiver, "missing", model),
                    None
                );
            },
        );
    }

    #[test]
    fn override_returns_apply_inherited_owner_arguments() {
        with_model(
            "class Base<T> { T method() => throw 0; T get value => throw 0; }
             class Middle<U> extends Base<U> {} class Child extends Middle<void> {}",
            |_, model, signatures| {
                let child = ResolvedType::interface_identity(model.resolve_name("Child").unwrap());
                assert_eq!(
                    signatures.instantiated_override_return(&child, "method", false, model),
                    Some(ResolvedType::Void)
                );
                assert_eq!(
                    signatures.instantiated_override_return(&child, "value", true, model),
                    Some(ResolvedType::Void)
                );
                assert_eq!(
                    signatures.instantiated_override_return(&child, "missing", false, model),
                    None
                );
            },
        );
    }

    #[test]
    fn constructors_and_explicit_function_calls_expose_parameter_contexts() {
        with_model(
            "class Token {}
             class Box<T> { Box(T value); Box.named(T value); }
             dynamic unnamed(Token value) => Box<Token>(value);
             dynamic named(Token value) => Box<Token>.named(value);
             dynamic local(Token Function(Token) callback, Token value) => callback.call(value);
             dynamic record((Token Function(Token),) row, Token value) => row.$1(value);
             dynamic nullable(Token Function(Token)? onChanged, Token value) => onChanged?.call(value);
             dynamic chained(Token Function(Token) callback, Token value) => callback.call.call(value);
             dynamic direct(Token Function(Token) callback, Token value) => callback(value);",
            |program, model, signatures| {
                let scope = TypeParameterScope::default();
                let token = ResolvedType::interface_identity(model.resolve_name("Token").unwrap());
                for declaration in program.declarations().skip(2) {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let body = arrow_expression(function.body());
                    let Expr::Call(call) = body else {
                        panic!()
                    };
                    let callee = call.callee();
                    let type_args = call.type_args();
                    let args = call.args();
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let context = InferenceContext {
                        model,
                        signatures,
                        type_parameters: &scope,
                        lookup: &|_| None,
                        argument_fallback: None,
                    };
                    let invocation = environment
                        .resolved_invocation(callee, type_args, &context)
                        .unwrap();
                    let substitutions = environment.infer_signature_substitutions_with_result(
                        invocation.inference_signature(),
                        args,
                        invocation.type_args.clone(),
                        &context,
                        None,
                        None,
                    );
                    assert_eq!(
                        model.substitute(&invocation.signature.positional[0], &substitutions),
                        token
                    );
                    assert_eq!(
                        invocation.constructor_result.is_some(),
                        matches!(function.name().name(), "unnamed" | "named")
                    );
                    let result = if invocation.constructor_result.is_some() {
                        ResolvedType::Interface {
                            occurrence: Default::default(),
                            identity: model.resolve_name("Box").unwrap(),
                            arguments: vec![token.clone()].into(),
                            nullable: false,
                            extension_type: false,
                        }
                    } else {
                        token.with_nullable(function.name().name() == "nullable")
                    };
                    assert_eq!(environment.infer_invocation(body, &context, None), result);
                }
            },
        );
    }

    #[test]
    fn an_implicit_method_shadows_a_named_extension_override() {
        with_model(
            "class Token {}
             class Foo { int member() => 0; }
             extension E on Foo { Token member() => Token(); }
             class Box { Foo E(Foo value) => value; }
             dynamic probe(Foo value) => E(value).member();",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(4).unwrap()
                else {
                    panic!()
                };
                let body = arrow_expression(function.body());
                let Expr::Call(call) = body else { panic!() };
                let callee = call.callee();
                let type_args = call.type_args();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                environment.declare_receiver(ResolvedType::interface_identity(
                    model.resolve_name("Box").unwrap(),
                ));
                let context = InferenceContext {
                    model,
                    signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                let invocation = environment
                    .resolved_invocation(callee, type_args, &context)
                    .unwrap();
                assert!(
                    invocation
                        .signature
                        .return_type
                        .interface("dart:core", "int")
                );
                assert!(
                    environment
                        .infer_invocation(body, &context, None)
                        .interface("dart:core", "int")
                );
            },
        );
    }

    #[test]
    fn implicit_methods_follow_generic_extension_receiver_bounds() {
        with_model(
            "class Base { int action() => 0; } \
             dynamic probe<T extends Base>(T receiver) => action();",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap()
                else {
                    panic!()
                };
                let body = arrow_expression(function.body());
                let signature = signatures
                    .function(&model.resolve_value("probe").unwrap())
                    .unwrap();
                let mut environment = TypeEnvironment::new();
                environment.declare_receiver(signature.positional[0].clone());
                assert!(
                    environment
                        .infer_with_signatures(
                            body,
                            model,
                            signatures,
                            &TypeParameterScope::default(),
                        )
                        .interface("dart:core", "int")
                );
            },
        );
    }

    #[test]
    fn lexical_lookup_shadows_an_implicit_this_method() {
        with_model(
            "class Token {}
             class Box { void action(Token value) {} }
             dynamic probe(Token Function(Token) callback, Token value) => action(value);",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(2).unwrap()
                else {
                    panic!()
                };
                let body = arrow_expression(function.body());
                let Expr::Call(call) = body else { panic!() };
                let callee = call.callee();
                let type_args = call.type_args();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                environment.declare_receiver(ResolvedType::interface_identity(
                    model.resolve_name("Box").unwrap(),
                ));
                let callback = environment.lookup("callback");
                let lookup = |name: &str| (name == "action").then(|| callback.clone());
                let context = InferenceContext {
                    model,
                    signatures,
                    type_parameters: &scope,
                    lookup: &lookup,
                    argument_fallback: None,
                };
                let invocation = environment
                    .resolved_invocation(callee, type_args, &context)
                    .unwrap();
                let token = ResolvedType::interface_identity(model.resolve_name("Token").unwrap());
                assert!(matches!(invocation.signature, Cow::Owned(_)));
                assert_eq!(invocation.signature.return_type, token);
                assert_eq!(environment.infer_invocation(body, &context, None), token);
            },
        );
    }

    #[test]
    fn a_declared_function_callable_returns_dynamic() {
        with_model(
            "dynamic probe(Function callback) => callback();",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().next().unwrap()
                else {
                    panic!()
                };
                let body = arrow_expression(function.body());
                let Expr::Call(call) = body else { panic!() };
                let callee = call.callee();
                let type_args = call.type_args();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert!(
                    environment
                        .lookup("callback")
                        .interface("dart:core", "Function")
                );
                let context = InferenceContext {
                    model,
                    signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                assert!(matches!(
                    environment.resolved_invocation_in(
                        callee, type_args, &context, None, &mut None, false,
                    ),
                    Err(ResolvedType::Dynamic)
                ));
                assert_eq!(
                    environment.infer_invocation(body, &context, None),
                    ResolvedType::Dynamic
                );
            },
        );
    }

    #[test]
    fn a_null_shorted_callable_field_keeps_its_parameter_context() {
        with_model(
            "class Token {}
             class Add { Token call(Token value) => value; }
             class Box { Add get add => Add(); }
             dynamic probe(Box? box, Token value) => box?.add(value);",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(3).unwrap()
                else {
                    panic!()
                };
                let body = arrow_expression(function.body());
                let Expr::Call(call) = body else { panic!() };
                let callee = call.callee();
                let type_args = call.type_args();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let context = InferenceContext {
                    model,
                    signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                let invocation = environment
                    .resolved_invocation(callee, type_args, &context)
                    .unwrap();
                let token = ResolvedType::interface_identity(model.resolve_name("Token").unwrap());
                assert_eq!(invocation.signature.positional[0], token);
                assert_eq!(invocation.signature.return_type, token);
                assert_eq!(
                    environment.infer_invocation(body, &context, None),
                    token.with_nullable(true)
                );
            },
        );
    }

    #[test]
    fn an_implicit_method_is_not_tagged_as_a_constructor() {
        with_model(
            "class Token {}
             class Foo<T> { Foo(T value); }
             class Bar<U> { U Foo(U value) => value; }
             dynamic probe(Bar<Token> owner, Token value) => Foo(value);",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = program.declarations().nth(3).unwrap()
                else {
                    panic!()
                };
                let body = arrow_expression(function.body());
                let Expr::Call(call) = body else { panic!() };
                let callee = call.callee();
                let type_args = call.type_args();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                environment.declare_receiver(environment.lookup("owner"));
                let context = InferenceContext {
                    model,
                    signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                let invocation = environment
                    .resolved_invocation(callee, type_args, &context)
                    .unwrap();
                let token = ResolvedType::interface_identity(model.resolve_name("Token").unwrap());
                assert!(invocation.constructor_result.is_none());
                assert_eq!(
                    model.substitute(&invocation.signature.positional[0], &invocation.owner),
                    token
                );
                assert_eq!(environment.infer_invocation(body, &context, None), token);
            },
        );
    }
}
