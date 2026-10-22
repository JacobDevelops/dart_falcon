use super::*;

/// The position of a top type in Dart's `MORETOP` order, where `void` is above
/// `dynamic`, which is above `Object?`; `None` for any other type.
pub(super) fn top_rank(ty: &ResolvedType) -> Option<u8> {
    match ty {
        ResolvedType::Void => Some(2),
        ResolvedType::Dynamic => Some(1),
        ResolvedType::Interface {
            identity,
            nullable: true,
            ..
        } if identity.is_sdk("dart:core", "Object") => Some(0),
        _ => None,
    }
}

/// Cached left operand, right operand, and their joined result.
type InferenceJoinEntry = (ResolvedType, ResolvedType, ResolvedType);

#[derive(Default)]
pub(super) struct InferenceJoinMemo {
    pub(super) shapes: FxHashMap<(u64, u64), Vec<InferenceJoinEntry>>,
    pub(super) hashes: FxHashMap<SharedHashKey, u64>,
    pub(super) provenance: FxHashMap<AliasPresenceKey, bool>,
    hash_insertions: Vec<SharedHashKey>,
    provenance_insertions: Vec<AliasPresenceKey>,
    depth: usize,
}

impl InferenceJoinMemo {
    fn hash(&mut self, ty: &ResolvedType) -> u64 {
        // Cache only owned shared storage, never projected stack roots.
        ty.structural_hash_shared_tracked(&mut self.hashes, &mut self.hash_insertions)
    }

    fn project(&mut self, ty: &ResolvedType, owner: &FunctionTypeAlias) -> ResolvedType {
        ty.clone().projected_from_memoized(
            owner,
            &mut self.provenance,
            Some(&mut self.provenance_insertions),
        )
    }

    fn discard_unanchored(&mut self, hash_checkpoint: usize, provenance_checkpoint: usize) {
        for key in self.hash_insertions.drain(hash_checkpoint..) {
            self.hashes.remove(&key);
        }
        for key in self.provenance_insertions.drain(provenance_checkpoint..) {
            self.provenance.remove(&key);
        }
    }
}

pub(super) fn inference_join_memoized(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> ResolvedType {
    let hash_checkpoint = memo.hash_insertions.len();
    let provenance_checkpoint = memo.provenance_insertions.len();
    memo.depth += 1;
    let cacheable = matches!(
        left,
        ResolvedType::Function { .. } | ResolvedType::Record { .. }
    ) || matches!(left, ResolvedType::Interface { arguments, .. } if !arguments.is_empty());
    let key = cacheable.then(|| (memo.hash(left), memo.hash(right)));
    if let Some(entries) = key.and_then(|key| memo.shapes.get(&key))
        && let Some((_, _, result)) = entries.iter().find(|(cached_left, cached_right, _)| {
            left.eq_memoized_with_aliases(
                cached_left,
                &mut FxHashSet::default(),
                AliasComparison::Origin,
            ) && right.eq_memoized_with_aliases(
                cached_right,
                &mut FxHashSet::default(),
                AliasComparison::Origin,
            )
        })
    {
        let result = result.clone();
        memo.discard_unanchored(hash_checkpoint, provenance_checkpoint);
        memo.depth -= 1;
        return result;
    }
    let result = inference_join_impl(left, right, signatures, model, memo);
    // Nested non-cacheable frames leave their suffix for an enclosing
    // frame to anchor or discard; otherwise repeated projections rewalk DAGs.
    if let Some(key) = key {
        memo.shapes
            .entry(key)
            .or_default()
            .push((left.clone(), right.clone(), result.clone()));
        // The stored operands/result now anchor every committed child identity.
        memo.hash_insertions.truncate(hash_checkpoint);
        memo.provenance_insertions.truncate(provenance_checkpoint);
    } else if memo.depth == 1 {
        memo.discard_unanchored(hash_checkpoint, provenance_checkpoint);
    }
    memo.depth -= 1;
    result
}

pub(super) fn inference_join(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    inference_join_memoized(
        left,
        right,
        signatures,
        model,
        &mut InferenceJoinMemo::default(),
    )
}

fn inference_join_impl(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> ResolvedType {
    let left = signatures.normalized_raw_type(Cow::Borrowed(left));
    let right = signatures.normalized_raw_type(Cow::Borrowed(right));
    let (left, right) = (left.as_ref(), right.as_ref());
    if matches!(right, ResolvedType::Never) {
        return left.clone();
    }
    if left == right {
        if let (
            ResolvedType::Record {
                occurrence: left_occurrence,
                positional: left_positional,
                named: left_named,
                nullable,
            },
            ResolvedType::Record {
                occurrence: right_occurrence,
                positional: right_positional,
                named: right_named,
                ..
            },
        ) = (left, right)
        {
            if Arc::ptr_eq(left_positional, right_positional)
                && Arc::ptr_eq(left_named, right_named)
                && left_occurrence.same_origin(right_occurrence)
            {
                return right.clone();
            }
            return ResolvedType::Record {
                occurrence: Default::default(),
                positional: left_positional
                    .iter()
                    .zip(right_positional.iter())
                    .map(|(left, right)| {
                        inference_join_memoized(
                            &memo.project(left, left_occurrence),
                            &memo.project(right, right_occurrence),
                            signatures,
                            model,
                            memo,
                        )
                    })
                    .collect(),
                named: left_named
                    .iter()
                    .zip(right_named.iter())
                    .map(|((name, left), (_, right))| {
                        (
                            name.clone(),
                            inference_join_memoized(
                                &memo.project(left, left_occurrence),
                                &memo.project(right, right_occurrence),
                                signatures,
                                model,
                                memo,
                            ),
                        )
                    })
                    .collect(),
                nullable: *nullable,
            };
        }
        if let (
            ResolvedType::Function {
                alias,
                return_type,
                positional,
                named,
                nullable,
                optional_positional,
                shape,
            },
            ResolvedType::Function {
                alias: other,
                return_type: other_return,
                positional: other_positional,
                named: other_named,
                ..
            },
        ) = (right, left)
            && !alias.same_origin(other)
        {
            // UP synthesizes a structural function type unless both operands
            // retain the same aliased type instance.
            return ResolvedType::Function {
                shape: shape.clone(),
                optional_positional: *optional_positional,
                alias: FunctionTypeAlias::default(),
                return_type: Arc::new(inference_join_memoized(
                    &memo.project(other_return, other),
                    &memo.project(return_type, alias),
                    signatures,
                    model,
                    memo,
                )),
                positional: other_positional
                    .iter()
                    .zip(positional.iter())
                    .map(|(left, right)| {
                        inference_join_memoized(
                            &memo.project(left, other),
                            &memo.project(right, alias),
                            signatures,
                            model,
                            memo,
                        )
                    })
                    .collect(),
                named: other_named
                    .iter()
                    .zip(named.iter())
                    .map(|((name, left), (_, right))| {
                        (
                            name.clone(),
                            inference_join_memoized(
                                &memo.project(left, other),
                                &memo.project(right, alias),
                                signatures,
                                model,
                                memo,
                            ),
                        )
                    })
                    .collect(),
                nullable: *nullable,
            };
        }
        // Equal interfaces retain the right operand's argument provenance.
        return right.clone();
    }
    if matches!(left, ResolvedType::Never) {
        return right.clone();
    }
    if matches!(left, ResolvedType::Unknown) || matches!(right, ResolvedType::Unknown) {
        return ResolvedType::Unknown;
    }
    if let (Some(left_rank), Some(right_rank)) = (top_rank(left), top_rank(right)) {
        return if left_rank > right_rank {
            left.clone()
        } else {
            right.clone()
        };
    }
    if matches!(left, ResolvedType::Void | ResolvedType::Dynamic) {
        return left.clone();
    }
    if matches!(right, ResolvedType::Void | ResolvedType::Dynamic) {
        return right.clone();
    }
    if matches!(left, ResolvedType::Null) {
        return right.with_nullable(true);
    }
    if matches!(right, ResolvedType::Null) {
        return left.with_nullable(true);
    }
    let nullable = left.nullable() || right.nullable();
    let (left_base, right_base) = (left.with_nullable(false), right.with_nullable(false));
    if left_base == right_base {
        return inference_join_memoized(&left_base, &right_base, signatures, model, memo)
            .with_nullable(nullable);
    }
    if let (
        ResolvedType::Function {
            alias: left_alias,
            return_type: left_return,
            positional: left_positional,
            named: left_named,
            optional_positional: left_optional,
            shape: left_shape,
            ..
        },
        ResolvedType::Function {
            alias: right_alias,
            return_type: right_return,
            positional: right_positional,
            named: right_named,
            optional_positional: right_optional,
            shape: right_shape,
            ..
        },
    ) = (left, right)
    {
        if let (Some(left_shape), Some(right_shape)) = (left_shape, right_shape)
            && (!left_shape.type_parameters.is_empty() || !right_shape.type_parameters.is_empty())
        {
            if left_shape.type_parameters.len() != right_shape.type_parameters.len() {
                return model.core_type("Function", nullable);
            }
            // Method tear-offs have declaration-local binders. Align those
            // binders before joining their covariant results and parameter meets.
            let substitutions: HashMap<_, _> = right_shape
                .type_parameters
                .iter()
                .zip(left_shape.type_parameters.iter())
                .filter_map(|(right, left)| match (right, left) {
                    (
                        ResolvedType::TypeParameter { id: right_id, .. },
                        ResolvedType::TypeParameter { id: left_id, .. },
                    ) if right_id != left_id => Some((right_id.clone(), left.clone())),
                    _ => None,
                })
                .collect();
            if !substitutions.is_empty() {
                for (left, right) in left_shape
                    .type_parameters
                    .iter()
                    .zip(right_shape.type_parameters.iter())
                {
                    let (
                        ResolvedType::TypeParameter { bound: left, .. },
                        ResolvedType::TypeParameter { bound: right, .. },
                    ) = (left, right)
                    else {
                        return ResolvedType::Unknown;
                    };
                    let right = model.substitute(right, &substitutions);
                    match (
                        signatures.relation(left, crate::TypeRelation::SubtypeOf(&right), model),
                        signatures.relation(&right, crate::TypeRelation::SubtypeOf(left), model),
                    ) {
                        (Truth::Yes, Truth::Yes) => {}
                        (Truth::No, _) | (_, Truth::No) => {
                            return model.core_type("Function", nullable);
                        }
                        _ => return ResolvedType::Unknown,
                    }
                }
                let mut right = model.substitute(right, &substitutions);
                if let ResolvedType::Function { shape, .. } = &mut right {
                    *shape = Some(FunctionTypeShape::new(
                        right_shape.required_named.to_vec(),
                        left_shape.type_parameters.to_vec(),
                    ));
                }
                return inference_join_memoized(left, &right, signatures, model, memo);
            }
        }
        // UP requires equal required arity, but retains only the common
        // optional positional prefix when the optional tails differ.
        let left_required = left_positional.len().saturating_sub(*left_optional);
        let right_required = right_positional.len().saturating_sub(*right_optional);
        if left_required != right_required {
            return model.core_type("Function", nullable);
        }
        let mut meet = |left: &ResolvedType, right: &ResolvedType| {
            let left = memo.project(left, left_alias);
            let right = memo.project(right, right_alias);
            let (left, right) = (&left, &right);
            if left == right {
                return Some(inference_join_memoized(
                    left, right, signatures, model, memo,
                ));
            }
            let left_to_right =
                signatures.relation(left, crate::TypeRelation::SubtypeOf(right), model);
            if left_to_right == Truth::Yes {
                return Some(left.clone());
            }
            match signatures.relation(right, crate::TypeRelation::SubtypeOf(left), model) {
                Truth::Yes => Some(right.clone()),
                Truth::No if left_to_right == Truth::No => Some(ResolvedType::Never),
                _ => None,
            }
        };
        let Some(positional) = left_positional
            .iter()
            .zip(right_positional.iter())
            .map(|(left, right)| meet(left, right))
            .collect::<Option<Vec<_>>>()
        else {
            return ResolvedType::Unknown;
        };
        let Some(mut named) = left_named
            .iter()
            .filter_map(|(name, left)| {
                let (_, right) = right_named.iter().find(|(other, _)| name == other)?;
                Some(meet(left, right).map(|ty| (name.clone(), ty)))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return ResolvedType::Unknown;
        };
        named.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        return ResolvedType::Function {
            shape: match (left_shape, right_shape) {
                (Some(left), Some(right)) if left == right => Some(Arc::clone(left)),
                (Some(left), Some(right)) if left.type_parameters == right.type_parameters => {
                    Some(FunctionTypeShape::new(
                        named
                            .iter()
                            .filter(|(name, _)| {
                                left.required_named.contains(name)
                                    || right.required_named.contains(name)
                            })
                            .map(|(name, _)| name.clone())
                            .collect(),
                        left.type_parameters.to_vec(),
                    ))
                }
                _ => None,
            },
            optional_positional: (*left_optional).min(*right_optional),
            alias: FunctionTypeAlias::default(),
            return_type: Arc::new(inference_join_memoized(
                &memo.project(left_return, left_alias),
                &memo.project(right_return, right_alias),
                signatures,
                model,
                memo,
            )),
            positional: positional.into(),
            named: named.into(),
            nullable,
        };
    }
    if let (
        ResolvedType::Interface {
            occurrence: left_occurrence,
            identity: left_identity,
            arguments: left_args,
            ..
        },
        ResolvedType::Interface {
            occurrence: right_occurrence,
            identity: right_identity,
            arguments: right_args,
            ..
        },
    ) = (left, right)
    {
        if left_identity == right_identity && left_args.len() == right_args.len() {
            return ResolvedType::Interface {
                occurrence: Default::default(),
                identity: left_identity.clone(),
                arguments: left_args
                    .iter()
                    .zip(right_args.iter())
                    .map(|(left, right)| {
                        inference_join_memoized(
                            &memo.project(left, left_occurrence),
                            &memo.project(right, right_occurrence),
                            signatures,
                            model,
                            memo,
                        )
                    })
                    .collect(),
                nullable,
                extension_type: matches!(
                    left,
                    ResolvedType::Interface {
                        extension_type: true,
                        ..
                    }
                ),
            };
        }
        if numeric(left) && numeric(right) {
            return model.core_type("num", nullable);
        }
        // Dart's UP of distinct classes: the supertype when one is a subtype of
        // the other, else the shared superinterface of greatest unique depth.
        // Lifting one side to the other's class and joining arguments instead
        // recurses without end once supertypes nest their own subclasses
        // (`C extends Box<Box<C>>` joined with `Box<D>`, `D extends Box<Box<D>>`).
        if signatures.relation(
            &right_base,
            crate::TypeRelation::SubtypeOf(&left_base),
            model,
        ) == Truth::Yes
        {
            return left.with_nullable(nullable);
        }
        if signatures.relation(
            &left_base,
            crate::TypeRelation::SubtypeOf(&right_base),
            model,
        ) == Truth::Yes
        {
            return right.with_nullable(nullable);
        }
        let scalar = |identity: &DeclarationIdentity| {
            matches!(identity,
            DeclarationIdentity::Sdk { library, name }
                if library.as_str() == "dart:core" && matches!(name.as_str(), "bool" | "int" | "double" | "num" | "String" | "Object"))
        };
        if scalar(left_identity) && scalar(right_identity) {
            return model.core_type("Object", nullable);
        }
        return signatures
            .common_superinterface(left, right, model)
            .map_or(ResolvedType::Unknown, |bound| bound.with_nullable(nullable));
    }
    if signatures.relation(
        &right_base,
        crate::TypeRelation::SubtypeOf(&left_base),
        model,
    ) == Truth::Yes
    {
        return left.with_nullable(nullable);
    }
    if signatures.relation(
        &left_base,
        crate::TypeRelation::SubtypeOf(&right_base),
        model,
    ) == Truth::Yes
    {
        return right.with_nullable(nullable);
    }
    ResolvedType::Unknown
}

/// The return type of the operator method `name` invoked on `receiver`.
pub(super) fn operator_return_type(
    receiver: &ResolvedType,
    name: &str,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    if matches!(receiver, ResolvedType::Dynamic) {
        return ResolvedType::Dynamic;
    }
    signatures
        .resolved_member(receiver, name, model)
        .map_or(ResolvedType::Unknown, |(signature, substitutions)| {
            model.substitute(&signature.return_type, &substitutions)
        })
}

/// The parameter type of the binary operator `op` on `receiver`, which is the
/// context type of its right operand.
pub(super) fn operator_parameter_type(
    receiver: &ResolvedType,
    op: BinaryOp,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> Option<ResolvedType> {
    let (signature, substitutions) =
        signatures.resolved_member(receiver, op.operator_name()?, model)?;
    signature
        .positional
        .first()
        .map(|ty| model.substitute(ty, &substitutions))
}

/// The static type of the binary operator `op`: the special typing of the
/// built-in numeric and `String +` operators, else the return type of the
/// receiver's operator method.
pub(super) fn binary_result(
    op: BinaryOp,
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    let builtin = operator_result(op, left, right, model);
    if builtin != ResolvedType::Unknown || numeric(left) {
        return builtin;
    }
    op.operator_name().map_or(ResolvedType::Unknown, |name| {
        operator_return_type(left, name, signatures, model)
    })
}

/// The static type of a conditional, `??` or switch expression with branch
/// types `types`: their least upper bound `T`, unless `T` is not a subtype of
/// the context type `K` while every branch is, which makes it `K`.
pub(super) fn contextual_join(
    types: &[ResolvedType],
    expected: Option<&ResolvedType>,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    if types.contains(&ResolvedType::Unknown) {
        return ResolvedType::Unknown;
    }
    let join = types.iter().skip(1).fold(
        types.first().cloned().unwrap_or(ResolvedType::Never),
        |join, ty| inference_join(&join, ty, signatures, model),
    );
    let Some(context) = expected.filter(|context| {
        !has_inference_holes(context) && !matches!(context, ResolvedType::Dynamic)
    }) else {
        return join;
    };
    if join == ResolvedType::Unknown || join == *context {
        return join;
    }
    // A concrete downward function context can retain a shared callable type
    // even when shape-sensitive UP produced the general Function interface.
    if join.interface("dart:core", "Function")
        && matches!(context, ResolvedType::Function { .. })
        && types.iter().all(|ty| {
            signatures.relation(ty, crate::TypeRelation::SubtypeOf(context), model) == Truth::Yes
        })
    {
        return context.clone();
    }
    match signatures.relation(&join, crate::TypeRelation::SubtypeOf(context), model) {
        Truth::Yes => join,
        Truth::No
            if types.iter().all(|ty| {
                signatures.relation(ty, crate::TypeRelation::SubtypeOf(context), model)
                    == Truth::Yes
            }) =>
        {
            context.clone()
        }
        Truth::No => join,
        Truth::Unknown => ResolvedType::Unknown,
    }
}

/// The static type of a built-in arithmetic, bitwise or `String +` operator, else `Unknown`.
pub(crate) fn operator_result(
    op: BinaryOp,
    left: &ResolvedType,
    right: &ResolvedType,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    if op == BinaryOp::Add
        && left.interface("dart:core", "String")
        && !left.nullable()
        && right.interface("dart:core", "String")
        && !right.nullable()
    {
        model.core_type("String", false)
    } else if numeric(left) && (numeric(right) || *right == ResolvedType::Dynamic) {
        let name = if op == BinaryOp::Div {
            "double"
        } else if matches!(
            op,
            BinaryOp::IntDiv
                | BinaryOp::BitAnd
                | BinaryOp::BitOr
                | BinaryOp::BitXor
                | BinaryOp::Shl
                | BinaryOp::Shr
                | BinaryOp::UShr
        ) {
            "int"
        } else if left.interface("dart:core", "double") || right.interface("dart:core", "double") {
            "double"
        } else if left.interface("dart:core", "int") && right.interface("dart:core", "int") {
            "int"
        } else {
            "num"
        };
        model.core_type(name, false)
    } else {
        ResolvedType::Unknown
    }
}

pub(super) fn numeric(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk { library, name },
            ..
        } if library.as_str() == "dart:core" && matches!(name.as_str(), "int" | "double" | "num")
    )
}

pub(super) fn fixnum_and_int(fixnum: &ResolvedType, int: &ResolvedType) -> bool {
    matches!(
        fixnum,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Package { package, name, .. },
            ..
        } if package.as_str() == "fixnum" && matches!(name.as_str(), "Int32" | "Int64")
    ) && int.interface("dart:core", "int")
}
