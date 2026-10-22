use super::*;
use falcon_syntax::cst::BinaryExpr;

impl TypeEnvironment {
    /// Infers an expression with a downward result context and lexical lookup.
    pub fn infer_with_context(
        &self,
        expr: Expr<'_>,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        type_parameters: &TypeParameterScope,
        expected: Option<&ResolvedType>,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> ResolvedType {
        let context = InferenceContext {
            model,
            signatures,
            type_parameters,
            lookup,
            argument_fallback: None,
        };
        self.infer_in_context(expr, context, expected, None)
    }

    /// Cached prefixes must come from the same flow state and lexical context.
    pub(crate) fn infer_binary_chain<'a>(
        &self,
        binary: BinaryExpr<'a>,
        context: &InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        cached: impl Fn(Expr<'a>) -> Option<ResolvedType>,
        mut publish: impl FnMut(Expr<'a>, &ResolvedType),
    ) -> Option<ResolvedType> {
        let ordinary = |binary: BinaryExpr<'_>| {
            !matches!(
                binary.op(),
                BinaryOp::EqEq
                    | BinaryOp::NotEq
                    | BinaryOp::Lt
                    | BinaryOp::Gt
                    | BinaryOp::LtEq
                    | BinaryOp::GtEq
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::NullCoalesce
            )
        };
        if !ordinary(binary) {
            return None;
        }
        let mut pending = Vec::new();
        let mut operand = binary.left();
        let mut left = None;
        loop {
            if let Some(ty) = cached(operand) {
                left = Some(ty);
                break;
            }
            let Expr::Binary(next) = operand else {
                break;
            };
            if !ordinary(next) {
                break;
            }
            pending.push(next);
            operand = next.left();
        }
        let mut left = left.unwrap_or_else(|| {
            self.infer_with_signatures_using_lookup(
                operand,
                context.model,
                context.signatures,
                context.type_parameters,
                context.lookup,
            )
        });
        for binary in pending.into_iter().rev().chain(std::iter::once(binary)) {
            let op = binary.op();
            let right_context =
                operator_parameter_type(&left, op, context.signatures, context.model);
            let right = self.infer_with_context(
                binary.right(),
                context.model,
                context.signatures,
                context.type_parameters,
                right_context.as_ref(),
                context.lookup,
            );
            left = binary_result(op, &left, &right, context.signatures, context.model);
            publish(Expr::Binary(binary), &left);
        }
        Some(left)
    }

    pub fn infer_constant_name(
        &self,
        mut names: falcon_syntax::cst::PatternNames<'_>,
        context: &InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
    ) -> ResolvedType {
        let Some(first) = names.next() else {
            return ResolvedType::Unknown;
        };
        let model = context.model;
        let signatures = context.signatures;
        let unshadowed = self.invocation_unshadowed(Expr::Ident(first), context);
        let mut ty = self.infer_in_context(
            Expr::Ident(first),
            queries::context_copy(context),
            None,
            None,
        );
        if names.is_empty() {
            return ty;
        }
        let mut segments = Vec::with_capacity(names.len() + 1);
        segments.push(first.name());
        for name in names {
            let field = name.name();
            segments.push(field);
            let owner = &segments[..segments.len() - 1];
            let promoted = (owner.len() == 1)
                .then(|| self.promoted_field(Expr::Ident(first), field))
                .flatten();
            let static_type =
                || self.static_access_segments(owner, field, model, signatures, context.lookup);
            ty = if let Some(promoted) = promoted {
                promoted
            } else if let Some(static_type) = static_type() {
                static_type
            } else if let Some(data) = signatures.instantiated_data_read(&ty, field, model) {
                data
            } else if matches!(ty, ResolvedType::Dynamic) {
                ResolvedType::Dynamic
            } else if field == "call" && matches!(ty, ResolvedType::Function { .. }) {
                ty.with_nullable(false)
            } else {
                let constructor = if unshadowed && model.resolve_dotted(&segments).is_none() {
                    model.resolve_dotted(owner).and_then(|identity| {
                        self.constructor_tearoff(
                            identity,
                            field,
                            AstChildren::empty(),
                            context,
                            None,
                        )
                    })
                } else {
                    None
                };
                let prefixed = || {
                    if matches!(ty, ResolvedType::Unknown) || ty.interface("dart:core", "Type") {
                        let [prefix] = owner else {
                            return None;
                        };
                        if !self.is_bound(prefix) && (context.lookup)(prefix).is_none() {
                            return model
                                .resolve_value(QualifiedName::prefixed(prefix, field))
                                .and_then(|identity| signatures.function(&identity))
                                .map(|signature| (signature, HashMap::new()));
                        }
                    }
                    None
                };
                let method = || {
                    if ty.record_field(field).is_some() {
                        return None;
                    }
                    signatures.resolved_member(&ty, field, model).or_else(|| {
                        if !unshadowed {
                            return None;
                        }
                        let identity = model.resolve_dotted(owner).or_else(|| {
                            model.resolve_extension(QualifiedName::from_segments(owner)?)
                        })?;
                        Some((signatures.member(&identity, field)?, HashMap::new()))
                    })
                };
                constructor
                    .or_else(|| {
                        let (signature, owner) = prefixed().or_else(method)?;
                        Some(self.tearoff_type(
                            signature,
                            &owner,
                            AstChildren::empty(),
                            context,
                            None,
                        ))
                    })
                    .unwrap_or_else(|| {
                        if ty == ResolvedType::Unknown
                            && !self.is_bound(first.name())
                            && (context.lookup)(first.name()).is_none()
                            && model.resolve_dotted(&segments).is_some()
                        {
                            model.core_type("Type", false)
                        } else {
                            ResolvedType::Unknown
                        }
                    })
            };
        }
        ty
    }

    /// Reuses an immediate receiver already inferred by a shared syntax visitor.
    /// A null-shorted selector chain makes the result nullable.
    pub(super) fn infer_in_context(
        &self,
        expr: Expr<'_>,
        context: InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        expected: Option<&ResolvedType>,
        receiver: Option<ResolvedType>,
    ) -> ResolvedType {
        let ty = self.infer_unshorted(expr, queries::context_copy(&context), expected, receiver);
        let ty = self.contextual_callable_type(ty, &context, expected);
        if null_shorted(expr) {
            ty.with_nullable(true)
        } else {
            ty
        }
    }

    /// Non-null callable values in a function context denote their `call` tear-off.
    /// Nullable values must first reach a non-null branch, as on the left of `??`.
    fn contextual_callable_type(
        &self,
        ty: ResolvedType,
        context: &InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        if matches!(expected, Some(ResolvedType::Function { .. }))
            && !matches!(ty, ResolvedType::Function { .. })
            && !ty.nullable()
            && let Some((signature, owner)) =
                context
                    .signatures
                    .resolved_member(&ty, "call", context.model)
        {
            self.tearoff_type(signature, &owner, AstChildren::empty(), context, expected)
        } else {
            ty
        }
    }

    /// The reached-branch type, before this chain's shorted null is added.
    /// Nullability declared by a member remains part of that branch's type.
    pub(super) fn infer_unshorted(
        &self,
        expr: Expr<'_>,
        context: InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        expected: Option<&ResolvedType>,
        mut receiver: Option<ResolvedType>,
    ) -> ResolvedType {
        if let Expr::Paren(paren) = expr {
            return self.infer_in_context(paren.expr(), context, expected, receiver);
        }
        let model = context.model;
        let signatures = context.signatures;
        let type_parameters = context.type_parameters;
        let lookup = context.lookup;
        let infer = |expr: Expr<'_>| {
            self.infer_with_signatures_using_lookup(
                expr,
                model,
                signatures,
                type_parameters,
                lookup,
            )
        };
        if let Expr::Field(access) = expr {
            let object = access.object();
            let field = access.field();
            let is_null_safe = access.is_null_safe();
            if let Some(ty) = self.promoted_field(object, field.name()) {
                return ty;
            }
            if let Some(ty) =
                self.static_access_type(object, field.name(), model, signatures, lookup)
            {
                return ty;
            }
            let receiver = receiver.take().unwrap_or_else(|| {
                self.receiver_type(
                    object,
                    field.name(),
                    &context,
                    expected.filter(|_| expression_has_dot_shorthand_base(object)),
                )
            });
            let receiver = if is_null_safe {
                receiver.non_null()
            } else {
                receiver
            };
            if let Some(ty) = signatures.instantiated_data_read(&receiver, field.name(), model) {
                return ty;
            }
            if matches!(receiver, ResolvedType::Dynamic) {
                return ResolvedType::Dynamic;
            }
            if field.name() == "call" && matches!(receiver, ResolvedType::Function { .. }) {
                return receiver.with_nullable(false);
            }
            let explicit = match object {
                Expr::GenericInstantiation(generic) => generic.type_args(),
                _ => AstChildren::empty(),
            };
            let unknown_receiver = receiver == ResolvedType::Unknown;
            if let Some(ty) =
                self.callable_tearoff(expr, explicit, &context, expected, &mut Some(receiver))
            {
                return ty;
            }
            if unknown_receiver
                && let Some(segments) = expression_segments(expr, true)
                && segments
                    .first()
                    .is_some_and(|name| !self.is_bound(name) && lookup(name).is_none())
                && model.resolve_dotted(&segments).is_some()
            {
                return model.core_type("Type", false);
            }
        }
        if let Expr::Call(call) = expr {
            let callee = call.callee();
            let type_args = call.type_args();
            let args = call.args();
            let (target, explicit) = match callee.unparenthesized() {
                Expr::GenericInstantiation(generic) => (generic.target(), generic.type_args()),
                Expr::Field(field) => match field.object() {
                    Expr::GenericInstantiation(generic) => (callee, generic.type_args()),
                    _ => (callee, type_args),
                },
                target => (target, type_args),
            };
            if let Expr::DotShorthand(shorthand) = target
                && let Some(ResolvedType::Interface { identity, .. }) = expected
            {
                let name = shorthand.name();
                if signatures.constructor(identity, name.name()).is_some() || name.name() == "new" {
                    return self.infer_constructor(
                        identity.clone(),
                        name.name(),
                        explicit,
                        args,
                        &context,
                        expected,
                    );
                }
                if signatures
                    .declared_member_facts(identity, name.name())
                    .iter()
                    .any(|facts| facts.is_static)
                    && let Some(signature) = signatures.member(identity, name.name())
                {
                    let substitutions = self.infer_signature_substitutions(
                        signature,
                        args,
                        explicit,
                        &context,
                        expected,
                        &HashMap::new(),
                    );
                    return model.substitute(&signature.return_type, &substitutions);
                }
            }
            let unshadowed = receiver.as_ref().is_none_or(|ty| {
                matches!(ty, ResolvedType::Unknown) || ty.interface("dart:core", "Type")
            }) && self.invocation_unshadowed(target, &context);
            if unshadowed {
                if matches!(target, Expr::Ident(_))
                    && let Some((signature, owner)) =
                        self.invocation_declaration(target, &context, &mut receiver, expected, true)
                {
                    let substitutions = self.infer_signature_substitutions(
                        signature, args, explicit, &context, expected, &owner,
                    );
                    return model.substitute(&signature.return_type, &substitutions);
                }
                if let Some((identity, name)) = resolve_call_constructor(target, model, true)
                    && (signatures.constructor(&identity, name).is_some()
                        || name == "new"
                        || model.identities.type_alias(&identity).is_some())
                {
                    return self
                        .infer_constructor(identity, name, explicit, args, &context, expected);
                }
            }
            // `num.clamp` alone has argument-dependent return typing.
            if let Expr::Field(access) = target
                && !access.is_null_safe()
                && access.field().name() == "clamp"
            {
                let object = access.object();
                let field = access.field();
                let reached = receiver.get_or_insert_with(|| {
                    self.receiver_type(
                        object,
                        field.name(),
                        &context,
                        expected.filter(|_| expression_has_dot_shorthand_base(object)),
                    )
                });
                if numeric(reached)
                    && !reached.nullable()
                    && args.positional().len() == 2
                    && args.named().is_empty()
                {
                    let operands = [
                        reached.clone(),
                        infer(args.positional().next().expect("two clamp arguments")),
                        infer(args.positional().nth(1).expect("two clamp arguments")),
                    ];
                    let all = |name| {
                        operands
                            .iter()
                            .all(|ty| ty.interface("dart:core", name) && !ty.nullable())
                    };
                    let name = if all("int") {
                        "int"
                    } else if all("double") {
                        "double"
                    } else {
                        "num"
                    };
                    return model.core_type(name, false);
                }
            }
            return match self.resolved_invocation_in(
                target,
                explicit,
                &context,
                expected,
                &mut receiver,
                false,
            ) {
                Ok(invocation) => {
                    let substitutions = self.infer_signature_substitutions_with_result(
                        invocation.inference_signature(),
                        args,
                        invocation.type_args.clone(),
                        &context,
                        expected,
                        None,
                    );
                    model.substitute(invocation.inference_signature().2, &substitutions)
                }
                Err(return_type) => return_type,
            };
        }
        self.infer_non_call(expr, context, expected)
    }

    // Keep collection and closure temporaries off recursive invocation frames.
    fn infer_non_call(
        &self,
        expr: Expr<'_>,
        context: InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        let model = context.model;
        let signatures = context.signatures;
        let type_parameters = context.type_parameters;
        let lookup = context.lookup;
        let infer = |expr: Expr<'_>| {
            self.infer_with_signatures_using_lookup(
                expr,
                model,
                signatures,
                type_parameters,
                lookup,
            )
        };
        match expr {
            Expr::FuncExpr(function) => self.infer_function(
                FunctionSyntax {
                    type_params: function.type_params(),
                    params: function.params(),
                    body: function.body(),
                    async_marker: function.async_marker(),
                    return_type: None,
                },
                model,
                signatures,
                type_parameters,
                expected,
                lookup,
            ),
            Expr::Is(_) => model.core_type("bool", false),
            Expr::Unary(unary) if unary.op() == UnaryOp::Bang => model.core_type("bool", false),
            Expr::Unary(unary)
                if unary.op() == UnaryOp::Minus
                    && matches!(unary.operand(), Expr::IntLit(_))
                    && expected.is_some_and(|context| context.interface("dart:core", "double")) =>
            {
                model.core_type("double", false)
            }
            Expr::Unary(unary) => {
                let op = unary.op();
                let ty = infer(unary.operand());
                if numeric(&ty) {
                    ty.with_nullable(false)
                } else {
                    let name = match op {
                        falcon_syntax::cst::UnaryOp::Minus => "unary-",
                        falcon_syntax::cst::UnaryOp::Tilde => "~",
                        falcon_syntax::cst::UnaryOp::PlusPlus => "+",
                        falcon_syntax::cst::UnaryOp::MinusMinus => "-",
                        falcon_syntax::cst::UnaryOp::Bang => unreachable!(),
                    };
                    operator_return_type(&ty, name, signatures, model)
                }
            }
            Expr::PostfixIncDec(postfix) => {
                let ty = infer(postfix.operand());
                if numeric(&ty) {
                    ty.with_nullable(false)
                } else {
                    ty
                }
            }
            Expr::IntLit(_)
                if expected.is_some_and(|context| context.interface("dart:core", "double")) =>
            {
                model.core_type("double", false)
            }
            Expr::IntLit(_) => model.core_type("int", false),
            Expr::DoubleLit(_) => model.core_type("double", false),
            Expr::StringLit(_) => model.core_type("String", false),
            Expr::BoolLit(_) => model.core_type("bool", false),
            Expr::NullLit(_) => ResolvedType::Null,
            Expr::SymbolLit(_) => model.core_type("Symbol", false),
            Expr::GenericInstantiation(generic) => {
                let target = generic.target();
                let type_args = generic.type_args();
                self.callable_tearoff(target, type_args.clone(), &context, expected, &mut None)
                    .or_else(|| {
                        let receiver = infer(target);
                        if receiver.nullable() {
                            return None;
                        }
                        let (signature, owner) =
                            signatures.resolved_member(&receiver, "call", model)?;
                        Some(self.tearoff_type(signature, &owner, type_args, &context, expected))
                    })
                    .unwrap_or_else(|| {
                        if expression_segments(target, true)
                            .is_some_and(|segments| model.resolve_dotted(&segments).is_some())
                        {
                            model.core_type("Type", false)
                        } else {
                            ResolvedType::Unknown
                        }
                    })
            }
            Expr::DotShorthand(shorthand) => {
                let name = shorthand.name();
                let receiver = match expected {
                    Some(ResolvedType::Function {
                        return_type, alias, ..
                    }) => Cow::Owned(return_type.as_ref().clone().projected_from(alias)),
                    Some(ty) => Cow::Borrowed(ty),
                    None => return ResolvedType::Unknown,
                };
                let ResolvedType::Interface {
                    occurrence,
                    identity,
                    arguments,
                    ..
                } = receiver.as_ref()
                else {
                    return ResolvedType::Unknown;
                };
                if signatures.is_enum_value(identity, name.name()) {
                    return receiver.with_nullable(false);
                }
                let owner = signatures.owner_substitutions(
                    signatures.type_parameters(identity).unwrap_or(&[]),
                    arguments,
                    Some(occurrence),
                );
                if let Some((_, ty)) = signatures.field(identity, name.name())
                    && signatures
                        .declared_member_facts(identity, name.name())
                        .iter()
                        .any(|facts| facts.is_static)
                {
                    return model.substitute(ty, &owner);
                }
                self.constructor_tearoff(
                    identity.clone(),
                    name.name(),
                    AstChildren::empty(),
                    &context,
                    expected,
                )
                .or_else(|| {
                    signatures.member(identity, name.name()).map(|signature| {
                        self.tearoff_type(
                            signature,
                            &owner,
                            AstChildren::empty(),
                            &context,
                            expected,
                        )
                    })
                })
                .unwrap_or(ResolvedType::Unknown)
            }
            Expr::This(_) => self.lookup("this"),
            Expr::Ident(identifier) => {
                // Bindings this environment adds (pattern variables of an
                // inferred switch arm) shadow the lexical lookup.
                if self.is_local_function(identifier.name()) {
                    self.callable_tearoff(expr, AstChildren::empty(), &context, expected, &mut None)
                        .unwrap_or_else(|| self.lookup(identifier.name()))
                } else if self.is_bound(identifier.name()) {
                    self.lookup(identifier.name())
                } else if let Some(ty) = self.promoted_this_field(identifier.name()) {
                    ty
                } else if let Some(ty) = lookup(identifier.name()) {
                    ty
                } else if let Some(ty) =
                    self.callable_tearoff(expr, AstChildren::empty(), &context, expected, &mut None)
                {
                    ty
                } else if let Some(identity) = model.resolve_value(identifier.name())
                    && let Some(ty) = signatures.value_type(&identity)
                {
                    ty.clone()
                } else if identifier.name() == "Never"
                    || model.resolve_name(identifier.name()).is_some()
                    || type_parameters.lookup(identifier.name()).is_some()
                {
                    model.core_type("Type", false)
                } else {
                    self.lookup(identifier.name())
                }
            }
            Expr::As(cast) => model.resolve_type_in(cast.dart_type(), type_parameters),
            Expr::NullAssert(assertion) => {
                let operand = assertion.operand();
                let operand_context = expected.map(|ty| ty.with_nullable(true));
                self.infer_with_context(
                    operand,
                    model,
                    signatures,
                    type_parameters,
                    operand_context.as_ref(),
                    lookup,
                )
                .non_null()
            }
            Expr::New(creation) => {
                let dart_type = creation.dart_type();
                let constructor_name = creation.constructor_name();
                let args = creation.args();
                if let DartType::Named(named) = dart_type
                    && let Some((identity, name)) =
                        resolve_new_constructor(dart_type, constructor_name, model)
                {
                    self.infer_constructor(
                        identity,
                        name,
                        named.type_args(),
                        args,
                        &context,
                        expected,
                    )
                } else {
                    model
                        .resolve_type_in(dart_type, type_parameters)
                        .with_nullable(false)
                }
            }
            Expr::Cascade(cascade) => self.infer_with_context(
                cascade.object(),
                model,
                signatures,
                type_parameters,
                expected,
                lookup,
            ),
            Expr::Await(awaited) => {
                let operand = awaited.expr();
                // `await e` gives `e` the context `FutureOr<K>`.
                let operand_context = expected
                    .filter(|context| !has_inference_holes(context))
                    .map(|context| ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:async", "FutureOr"),
                        arguments: vec![context.clone()].into(),
                        nullable: false,
                        extension_type: false,
                    });
                let ty = self.infer_with_context(
                    operand,
                    model,
                    signatures,
                    type_parameters,
                    operand_context.as_ref(),
                    lookup,
                );
                let future = if ty.interface("dart:async", "Future")
                    || ty.interface("dart:async", "FutureOr")
                {
                    Some(ty.clone())
                } else if matches!(ty, ResolvedType::Interface { .. }) {
                    signatures.instantiated_supertype(&ty, "dart:async", "Future", model)
                } else {
                    None
                };
                match future {
                    Some(future) => future
                        .type_argument(0)
                        .map(|argument| {
                            argument.with_nullable(argument.nullable() || ty.nullable())
                        })
                        .unwrap_or(ResolvedType::Dynamic),
                    None => ty,
                }
            }
            Expr::Record(record) => {
                let fields = record.fields();
                let expected = expected.filter(|ty| matches!(ty, ResolvedType::Record { positional, named, .. }
                    if positional.len() == fields.clone().filter(|field| field.name().is_none()).count()
                        && named.len() == fields.clone().filter(|field| field.name().is_some()).count()
                        && fields.clone().filter_map(|field| field.name()).all(|field| named.iter().any(|(name, _)| name == field.name()))));
                let positional_count = fields
                    .clone()
                    .filter(|field| field.name().is_none())
                    .count();
                let mut positional = Vec::with_capacity(positional_count);
                let mut named = Vec::with_capacity(fields.len() - positional_count);
                for field in fields {
                    let context = expected.and_then(|ty| match field.name() {
                        Some(name) => ty.record_field(name.name()),
                        None => ty.record_positional(positional.len()),
                    });
                    let ty = self.infer_with_context(
                        field.value(),
                        model,
                        signatures,
                        type_parameters,
                        context.as_ref(),
                        lookup,
                    );
                    // Record fields implicitly downcast dynamic values to
                    // their schema, but a void context does not impute void.
                    let ty = if ty == ResolvedType::Dynamic {
                        context
                            .filter(|ty| {
                                !matches!(ty, ResolvedType::Void) && !has_inference_holes(ty)
                            })
                            .unwrap_or(ty)
                    } else {
                        ty
                    };
                    match field.name() {
                        Some(name) => named.push((name.name().to_owned(), ty)),
                        None => positional.push(ty),
                    }
                }
                named.sort_by(|left, right| left.0.cmp(&right.0));
                ResolvedType::Record {
                    occurrence: Default::default(),
                    positional: positional.into(),
                    named: named.into(),
                    nullable: false,
                }
            }
            Expr::List(_) | Expr::Set(_) => {
                let (type_arg, elements) = match expr {
                    Expr::List(list) => (list.type_arg(), list.elements()),
                    Expr::Set(set) => (set.type_arg(), set.elements()),
                    _ => unreachable!(),
                };
                // Spread-only brace literals are parsed as sets until their
                // downward context or spread operands establish a map shape.
                let map_context = if matches!(expr, Expr::Set(_)) && type_arg.is_none() {
                    expected.and_then(|ty| {
                        signatures.instantiated_supertype(ty, "dart:core", "Map", model)
                    })
                } else {
                    None
                };
                let context = type_arg
                    .map(|ty| model.resolve_type_in(ty, type_parameters))
                    .or_else(|| map_context.as_ref().and_then(|ty| ty.type_argument(0)))
                    .or_else(|| {
                        expected
                            .and_then(|expected| {
                                signatures.instantiated_supertype(
                                    expected,
                                    "dart:core",
                                    "Iterable",
                                    model,
                                )
                            })
                            .and_then(|ty| ty.type_argument(0))
                            // Unsolved downward arguments must not suppress
                            // inference from the actual collection elements.
                            .filter(|ty| !has_inference_holes(ty))
                    });
                let map_value_context = map_context.as_ref().and_then(|ty| ty.type_argument(1));
                let mut visitor = CollectionTypes {
                    lexical: ClosureReturns {
                        environment: self.clone(),
                        conservative_writes: false,
                        never_expressions: FxHashSet::default(),
                        expression_never: None,
                        flow: super::flow::FlowContext {
                            model,
                            signatures,
                            type_parameters,
                            field_promotions: self.field_promotions,
                        },
                        context: None,
                        lookup,
                        types: Vec::new(),
                        exhaustive_switches: Vec::new(),
                    },
                    contexts: [context.as_ref(), map_value_context.as_ref()],
                    components: [Vec::new(), Vec::new()],
                    infer_map_spreads: matches!(expr, Expr::Set(_))
                        && type_arg.is_none()
                        && context.is_none(),
                };
                for element in elements {
                    visitor.visit_element(element);
                }
                let CollectionTypes { mut components, .. } = visitor;
                if let Some(map) = map_context {
                    return map.non_null();
                }
                if !components[1].is_empty() {
                    return ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "Map"),
                        arguments: components
                            .into_iter()
                            .map(|types| {
                                types
                                    .into_iter()
                                    .reduce(|left, right| {
                                        inference_join(&left, &right, signatures, model)
                                    })
                                    .unwrap_or(ResolvedType::Dynamic)
                            })
                            .collect(),
                        nullable: false,
                        extension_type: false,
                    };
                }
                let element = context.unwrap_or_else(|| {
                    components[0]
                        .drain(..)
                        .reduce(|left, right| inference_join(&left, &right, signatures, model))
                        .unwrap_or(ResolvedType::Dynamic)
                });
                ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk(
                        "dart:core",
                        if matches!(expr, Expr::List(_)) {
                            "List"
                        } else {
                            "Set"
                        },
                    ),
                    arguments: vec![element].into(),
                    nullable: false,
                    extension_type: false,
                }
            }
            Expr::Map(map) => {
                let type_args = map.type_args();
                let elements = map.elements();
                // `{}` is a set literal when its context is an iterable type.
                if type_args.is_empty()
                    && elements.is_empty()
                    && let Some(element) = expected
                        .filter(|expected| !expected.interface("dart:core", "Map"))
                        .and_then(|expected| {
                            signatures.instantiated_supertype(
                                expected,
                                "dart:core",
                                "Iterable",
                                model,
                            )
                        })
                        .and_then(|iterable| iterable.type_argument(0))
                {
                    return ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "Set"),
                        arguments: vec![element].into(),
                        nullable: false,
                        extension_type: false,
                    };
                }
                let contexts = [0, 1].map(|ordinal| {
                    type_args
                        .clone()
                        .nth(ordinal)
                        .map(|ty| model.resolve_type_in(ty, type_parameters))
                        .or_else(|| {
                            expected
                                .and_then(|ty| {
                                    signatures.instantiated_supertype(ty, "dart:core", "Map", model)
                                })
                                .and_then(|ty| ty.type_argument(ordinal))
                                .filter(|ty| !has_inference_holes(ty))
                        })
                });
                let mut visitor = CollectionTypes {
                    lexical: ClosureReturns {
                        environment: self.clone(),
                        conservative_writes: false,
                        never_expressions: FxHashSet::default(),
                        expression_never: None,
                        flow: super::flow::FlowContext {
                            model,
                            signatures,
                            type_parameters,
                            field_promotions: self.field_promotions,
                        },
                        context: None,
                        lookup,
                        types: Vec::new(),
                        exhaustive_switches: Vec::new(),
                    },
                    contexts: [contexts[0].as_ref(), contexts[1].as_ref()],
                    components: [Vec::new(), Vec::new()],
                    infer_map_spreads: false,
                };
                for element in elements {
                    visitor.visit_element(element);
                }
                let CollectionTypes { mut components, .. } = visitor;
                let arguments = contexts
                    .into_iter()
                    .enumerate()
                    .map(|(ordinal, context)| {
                        context.unwrap_or_else(|| {
                            components[ordinal]
                                .drain(..)
                                .reduce(|left, right| {
                                    inference_join(&left, &right, signatures, model)
                                })
                                .unwrap_or(ResolvedType::Dynamic)
                        })
                    })
                    .collect();
                ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:core", "Map"),
                    arguments,
                    nullable: false,
                    extension_type: false,
                }
            }
            Expr::Binary(binary) => {
                let op = binary.op();
                let left = binary.left();
                let right = binary.right();
                match op {
                    BinaryOp::EqEq
                    | BinaryOp::NotEq
                    | BinaryOp::Lt
                    | BinaryOp::Gt
                    | BinaryOp::LtEq
                    | BinaryOp::GtEq
                    | BinaryOp::And
                    | BinaryOp::Or => model.core_type("bool", false),
                    // `NonNull(T1) ⊔ T2`: `e1` takes the context `K?`, `e2` the
                    // context `K`, or `NonNull(T1)` without one.
                    BinaryOp::NullCoalesce => {
                        let left_context = expected.map(|context| context.with_nullable(true));
                        let left = self
                            .infer_with_context(
                                left,
                                model,
                                signatures,
                                type_parameters,
                                left_context.as_ref(),
                                lookup,
                            )
                            .non_null();
                        let left = self.contextual_callable_type(left, &context, expected);
                        let right = self.infer_with_context(
                            right,
                            model,
                            signatures,
                            type_parameters,
                            expected
                                .filter(|ty| !has_inference_holes(ty))
                                .or(Some(&left).filter(|ty| **ty != ResolvedType::Unknown)),
                            lookup,
                        );
                        contextual_join(&[left, right], expected, signatures, model)
                    }
                    _ => self
                        .infer_binary_chain(binary, &context, |_| None, |_, _| {})
                        .expect("ordinary binary operator"),
                }
            }
            Expr::Conditional(conditional) => {
                let condition = conditional.condition();
                let then_expr = conditional.then_expr();
                let else_expr = conditional.else_expr();
                let flow = self.flow(model, signatures, type_parameters);
                let branch = |truth: bool, branch: Expr<'_>| {
                    let mut environment = self.clone();
                    flow.assume_condition(&mut environment, condition, truth);
                    environment.infer_with_context(
                        branch,
                        model,
                        signatures,
                        type_parameters,
                        expected,
                        lookup,
                    )
                };
                let types = [branch(true, then_expr), branch(false, else_expr)];
                contextual_join(&types, expected, signatures, model)
            }
            Expr::Switch(switch) => {
                let subject = switch.subject();
                let arms = switch.arms();
                let flow = self.flow(model, signatures, type_parameters);
                let matched = infer(subject);
                let arm_types: Vec<_> = arms
                    .map(|arm| {
                        let mut environment = self.clone();
                        environment.push_scope();
                        flow.assume_pattern(&mut environment, subject, arm.pattern(), true);
                        flow.declare_pattern(&mut environment, arm.pattern(), matched.clone());
                        if let Some(guard) = arm.guard() {
                            flow.assume_condition(&mut environment, guard, true);
                        }
                        environment.infer_with_context(
                            arm.body(),
                            model,
                            signatures,
                            type_parameters,
                            expected,
                            lookup,
                        )
                    })
                    .collect();
                contextual_join(&arm_types, expected, signatures, model)
            }
            Expr::Index(index) => {
                let object = index.object();
                let is_null_safe = index.is_null_safe();
                let receiver = self.receiver_type(object, "[]", &context, None);
                let receiver = if is_null_safe {
                    receiver.non_null()
                } else {
                    receiver
                };
                operator_return_type(&receiver, "[]", signatures, model)
            }
            Expr::Assign(assignment) => {
                let target = assignment.target();
                let op = assignment.op();
                let value = assignment.value();
                let target_type = infer(target);
                match op.binary() {
                    None => self.infer_with_context(
                        value,
                        model,
                        signatures,
                        type_parameters,
                        Some(&target_type).filter(|ty| **ty != ResolvedType::Unknown),
                        lookup,
                    ),
                    Some(BinaryOp::NullCoalesce) => {
                        let left = target_type.non_null();
                        let right = self.infer_with_context(
                            value,
                            model,
                            signatures,
                            type_parameters,
                            Some(&target_type),
                            lookup,
                        );
                        contextual_join(&[left, right], None, signatures, model)
                    }
                    Some(op) => binary_result(op, &target_type, &infer(value), signatures, model),
                }
            }
            Expr::Throw(_) => ResolvedType::Never,
            _ => ResolvedType::Unknown,
        }
    }
}
