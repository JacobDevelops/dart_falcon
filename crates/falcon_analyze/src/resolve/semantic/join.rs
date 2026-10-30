use super::{
    AliasComparison, AliasPresenceKey, Arc, BinaryOp, Cow, DeclarationIdentity, FunctionTypeAlias,
    FunctionTypeShape, FxHashMap, FxHashSet, ResolvedType, SemanticModel, SharedHashKey,
    SignatureIndex, Truth, has_inference_holes,
};
use crate::resolve::semantic::ResolvedTypeKind;

/// The position of a top type in Dart's `MORETOP` order, where `void` is above
/// `dynamic`, which is above `Object?`; `None` for any other type.
pub(super) fn top_rank(ty: &ResolvedType) -> Option<u8> {
    match ty {
        ResolvedType {
            kind: ResolvedTypeKind::Void,
        } => Some(2),
        ResolvedType {
            kind: ResolvedTypeKind::Dynamic,
        } => Some(1),
        ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity,
                    nullable: true,
                    ..
                },
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
        ResolvedType {
            kind: ResolvedTypeKind::Function { .. } | ResolvedTypeKind::Record { .. }
        }
    ) || matches!(left, ResolvedType { kind: ResolvedTypeKind::Interface { arguments, .. } } if !arguments.is_empty());
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
    if matches!(
        right,
        ResolvedType {
            kind: ResolvedTypeKind::Never
        }
    ) {
        return left.clone();
    }
    if left == right {
        return match (&left.kind, &right.kind) {
            (ResolvedTypeKind::Record { .. }, ResolvedTypeKind::Record { .. }) => {
                join_equal_records(left, right, signatures, model, memo)
            }
            (ResolvedTypeKind::Function { .. }, ResolvedTypeKind::Function { .. }) => {
                join_equal_functions(left, right, signatures, model, memo)
            }
            // Equal interfaces retain the right operand's argument provenance.
            _ => right.clone(),
        };
    }
    if let Some(joined) = join_special_types(left, right) {
        return joined;
    }
    let nullable = left.nullable() || right.nullable();
    let (left_base, right_base) = (left.with_nullable(false), right.with_nullable(false));
    if left_base == right_base {
        return inference_join_memoized(&left_base, &right_base, signatures, model, memo)
            .with_nullable(nullable);
    }
    if matches!(
        (&left.kind, &right.kind),
        (
            ResolvedTypeKind::Function { .. },
            ResolvedTypeKind::Function { .. }
        )
    ) {
        return join_functions(left, right, signatures, model, memo);
    }
    if matches!(
        (&left.kind, &right.kind),
        (
            ResolvedTypeKind::Interface { .. },
            ResolvedTypeKind::Interface { .. }
        )
    ) {
        return join_interfaces(
            left,
            right,
            signatures,
            model,
            memo,
            &left_base,
            &right_base,
        );
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
    ResolvedType::UNKNOWN
}

fn join_equal_records(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> ResolvedType {
    if let (
        ResolvedType {
            kind:
                ResolvedTypeKind::Record {
                    occurrence: left_occurrence,
                    positional: left_positional,
                    named: left_named,
                    nullable,
                },
        },
        ResolvedType {
            kind:
                ResolvedTypeKind::Record {
                    occurrence: right_occurrence,
                    positional: right_positional,
                    named: right_named,
                    ..
                },
        },
    ) = (left, right)
    {
        if Arc::ptr_eq(left_positional, right_positional)
            && Arc::ptr_eq(left_named, right_named)
            && left_occurrence.same_origin(right_occurrence)
        {
            return right.clone();
        }
        return ResolvedType {
            kind: ResolvedTypeKind::Record {
                occurrence: FunctionTypeAlias::default(),
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
            },
        };
    }
    right.clone()
}

fn join_equal_functions(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> ResolvedType {
    if let (
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    alias,
                    return_type,
                    positional,
                    named,
                    nullable,
                    optional_positional,
                    shape,
                },
        },
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    alias: other,
                    return_type: other_return,
                    positional: other_positional,
                    named: other_named,
                    ..
                },
        },
    ) = (right, left)
        && !alias.same_origin(other)
    {
        // UP synthesizes a structural function type unless both operands
        // retain the same aliased type instance.
        return ResolvedType {
            kind: ResolvedTypeKind::Function {
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
            },
        };
    }
    right.clone()
}

fn join_special_types(left: &ResolvedType, right: &ResolvedType) -> Option<ResolvedType> {
    if matches!(
        left,
        ResolvedType {
            kind: ResolvedTypeKind::Never
        }
    ) {
        return Some(right.clone());
    }
    if matches!(
        left,
        ResolvedType {
            kind: ResolvedTypeKind::Unknown
        }
    ) || matches!(
        right,
        ResolvedType {
            kind: ResolvedTypeKind::Unknown
        }
    ) {
        return Some(ResolvedType::UNKNOWN);
    }
    if let (Some(left_rank), Some(right_rank)) = (top_rank(left), top_rank(right)) {
        return Some(if left_rank > right_rank {
            left.clone()
        } else {
            right.clone()
        });
    }
    if matches!(
        left,
        ResolvedType {
            kind: ResolvedTypeKind::Void | ResolvedTypeKind::Dynamic
        }
    ) {
        return Some(left.clone());
    }
    if matches!(
        right,
        ResolvedType {
            kind: ResolvedTypeKind::Void | ResolvedTypeKind::Dynamic
        }
    ) {
        return Some(right.clone());
    }
    if matches!(
        left,
        ResolvedType {
            kind: ResolvedTypeKind::Null
        }
    ) {
        return Some(right.with_nullable(true));
    }
    if matches!(
        right,
        ResolvedType {
            kind: ResolvedTypeKind::Null
        }
    ) {
        return Some(left.with_nullable(true));
    }
    None
}

fn join_aligned_function_binders(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> Option<ResolvedType> {
    let nullable = left.nullable() || right.nullable();
    if let (
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    shape: Some(left_shape),
                    ..
                },
        },
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    shape: Some(right_shape),
                    ..
                },
        },
    ) = (left, right)
        && (!left_shape.type_parameters.is_empty() || !right_shape.type_parameters.is_empty())
    {
        if left_shape.type_parameters.len() != right_shape.type_parameters.len() {
            return Some(model.core_type("Function", nullable));
        }
        // Method tear-offs have declaration-local binders. Align those
        // binders before joining their covariant results and parameter meets.
        let substitutions: FxHashMap<_, _> = right_shape
            .type_parameters
            .iter()
            .zip(left_shape.type_parameters.iter())
            .filter_map(|(right, left)| match (right, left) {
                (
                    ResolvedType {
                        kind: ResolvedTypeKind::TypeParameter { id: right_id, .. },
                    },
                    ResolvedType {
                        kind: ResolvedTypeKind::TypeParameter { id: left_id, .. },
                    },
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
                    ResolvedType {
                        kind: ResolvedTypeKind::TypeParameter { bound: left, .. },
                    },
                    ResolvedType {
                        kind: ResolvedTypeKind::TypeParameter { bound: right, .. },
                    },
                ) = (left, right)
                else {
                    return Some(ResolvedType::UNKNOWN);
                };
                let right = model.substitute(right, &substitutions);
                match (
                    signatures.relation(left, crate::TypeRelation::SubtypeOf(&right), model),
                    signatures.relation(&right, crate::TypeRelation::SubtypeOf(left), model),
                ) {
                    (Truth::Yes, Truth::Yes) => {}
                    (Truth::No, _) | (_, Truth::No) => {
                        return Some(model.core_type("Function", nullable));
                    }
                    _ => return Some(ResolvedType::UNKNOWN),
                }
            }
            let mut right = model.substitute(right, &substitutions);
            if let ResolvedType {
                kind: ResolvedTypeKind::Function { shape, .. },
            } = &mut right
            {
                *shape = Some(FunctionTypeShape::new(
                    right_shape.required_named.to_vec(),
                    left_shape.type_parameters.to_vec(),
                ));
            }
            return Some(inference_join_memoized(
                left, &right, signatures, model, memo,
            ));
        }
    }
    None
}

fn meet_function_parameters(
    left: &ResolvedType,
    right: &ResolvedType,
    left_alias: &FunctionTypeAlias,
    right_alias: &FunctionTypeAlias,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> Option<ResolvedType> {
    let left = memo.project(left, left_alias);
    let right = memo.project(right, right_alias);
    let (left, right) = (&left, &right);
    if left == right {
        return Some(inference_join_memoized(
            left, right, signatures, model, memo,
        ));
    }
    let left_to_right = signatures.relation(left, crate::TypeRelation::SubtypeOf(right), model);
    if left_to_right == Truth::Yes {
        return Some(left.clone());
    }
    match signatures.relation(right, crate::TypeRelation::SubtypeOf(left), model) {
        Truth::Yes => Some(right.clone()),
        Truth::No if left_to_right == Truth::No => Some(ResolvedType::NEVER),
        _ => None,
    }
}

fn join_functions(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> ResolvedType {
    let nullable = left.nullable() || right.nullable();
    if let (
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    alias: left_alias,
                    return_type: left_return,
                    positional: left_positional,
                    named: left_named,
                    optional_positional: left_optional,
                    shape: left_shape,
                    ..
                },
        },
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    alias: right_alias,
                    return_type: right_return,
                    positional: right_positional,
                    named: right_named,
                    optional_positional: right_optional,
                    shape: right_shape,
                    ..
                },
        },
    ) = (left, right)
    {
        if let Some(joined) = join_aligned_function_binders(left, right, signatures, model, memo) {
            return joined;
        }
        // UP requires equal required arity, but retains only the common
        // optional positional prefix when the optional tails differ.
        let left_required = left_positional.len().saturating_sub(*left_optional);
        let right_required = right_positional.len().saturating_sub(*right_optional);
        if left_required != right_required {
            return model.core_type("Function", nullable);
        }
        let mut meet = |left: &ResolvedType, right: &ResolvedType| {
            meet_function_parameters(
                left,
                right,
                left_alias,
                right_alias,
                signatures,
                model,
                memo,
            )
        };
        let Some(positional) = left_positional
            .iter()
            .zip(right_positional.iter())
            .map(|(left, right)| meet(left, right))
            .collect::<Option<Vec<_>>>()
        else {
            return ResolvedType::UNKNOWN;
        };
        let Some(mut named) = left_named
            .iter()
            .filter_map(|(name, left)| {
                let (_, right) = right_named.iter().find(|(other, _)| name == other)?;
                Some(meet(left, right).map(|ty| (name.clone(), ty)))
            })
            .collect::<Option<Vec<_>>>()
        else {
            return ResolvedType::UNKNOWN;
        };
        named.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        return ResolvedType {
            kind: ResolvedTypeKind::Function {
                shape: join_function_shape(left_shape.as_ref(), right_shape.as_ref(), &named),
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
            },
        };
    }
    unreachable!()
}

fn join_function_shape(
    left: Option<&Arc<FunctionTypeShape>>,
    right: Option<&Arc<FunctionTypeShape>>,
    named: &[(String, ResolvedType)],
) -> Option<Arc<FunctionTypeShape>> {
    match (left, right) {
        (Some(left), Some(right)) if left == right => Some(Arc::clone(left)),
        (Some(left), Some(right)) if left.type_parameters == right.type_parameters => {
            Some(FunctionTypeShape::new(
                named
                    .iter()
                    .filter(|(name, _)| {
                        left.required_named.contains(name) || right.required_named.contains(name)
                    })
                    .map(|(name, _)| name.clone())
                    .collect(),
                left.type_parameters.to_vec(),
            ))
        }
        _ => None,
    }
}

fn join_interfaces(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
    left_base: &ResolvedType,
    right_base: &ResolvedType,
) -> ResolvedType {
    let nullable = left.nullable() || right.nullable();
    if let (
        ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    occurrence: left_occurrence,
                    identity: left_identity,
                    arguments: left_args,
                    ..
                },
        },
        ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    occurrence: right_occurrence,
                    identity: right_identity,
                    arguments: right_args,
                    ..
                },
        },
    ) = (left, right)
    {
        if left_identity == right_identity && left_args.len() == right_args.len() {
            return ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: FunctionTypeAlias::default(),
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
                        ResolvedType {
                            kind: ResolvedTypeKind::Interface {
                                extension_type: true,
                                ..
                            }
                        }
                    ),
                },
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
        if signatures.relation(right_base, crate::TypeRelation::SubtypeOf(left_base), model)
            == Truth::Yes
        {
            return left.with_nullable(nullable);
        }
        if signatures.relation(left_base, crate::TypeRelation::SubtypeOf(right_base), model)
            == Truth::Yes
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
            .map_or(ResolvedType::UNKNOWN, |bound| bound.with_nullable(nullable));
    }
    unreachable!()
}

/// The return type of the operator method `name` invoked on `receiver`.
pub(super) fn operator_return_type(
    receiver: &ResolvedType,
    name: &str,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    if matches!(
        receiver,
        ResolvedType {
            kind: ResolvedTypeKind::Dynamic
        }
    ) {
        return ResolvedType::DYNAMIC;
    }
    signatures
        .resolved_member(receiver, name, model)
        .map_or(ResolvedType::UNKNOWN, |(signature, substitutions)| {
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
pub(crate) fn binary_result(
    op: BinaryOp,
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    let builtin = operator_result(op, left, right, model);
    if builtin != ResolvedType::UNKNOWN || numeric(left) {
        return builtin;
    }
    op.operator_name().map_or(ResolvedType::UNKNOWN, |name| {
        operator_return_type(left, name, signatures, model)
    })
}

pub(crate) fn null_coalesce_result(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    contextual_join(&[left.non_null(), right.clone()], None, signatures, model)
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
    if types.contains(&ResolvedType::UNKNOWN) {
        return ResolvedType::UNKNOWN;
    }
    let join = types.iter().skip(1).fold(
        types.first().cloned().unwrap_or(ResolvedType::NEVER),
        |join, ty| inference_join(&join, ty, signatures, model),
    );
    let Some(context) = expected.filter(|context| {
        !has_inference_holes(context)
            && !matches!(
                context,
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic
                }
            )
    }) else {
        return join;
    };
    if join == ResolvedType::UNKNOWN || join == *context {
        return join;
    }
    // A concrete downward function context can retain a shared callable type
    // even when shape-sensitive UP produced the general Function interface.
    if join.interface("dart:core", "Function")
        && matches!(
            context,
            ResolvedType {
                kind: ResolvedTypeKind::Function { .. }
            }
        )
        && types.iter().all(|ty| {
            signatures.relation(ty, crate::TypeRelation::SubtypeOf(context), model) == Truth::Yes
        })
    {
        return context.clone();
    }
    match signatures.relation(&join, crate::TypeRelation::SubtypeOf(context), model) {
        Truth::Yes => join,
        Truth::No => {
            if types.iter().all(|ty| {
                signatures.relation(ty, crate::TypeRelation::SubtypeOf(context), model)
                    == Truth::Yes
            }) {
                context.clone()
            } else {
                join
            }
        }
        Truth::Unknown => ResolvedType::UNKNOWN,
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
    } else if numeric(left) && (numeric(right) || *right == ResolvedType::DYNAMIC) {
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
        ResolvedType::UNKNOWN
    }
}

pub(super) fn numeric(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType { kind: ResolvedTypeKind::Interface {
            identity: DeclarationIdentity::Sdk { library, name },
            ..
        } } if library.as_str() == "dart:core" && matches!(name.as_str(), "int" | "double" | "num")
    )
}

pub(super) fn fixnum_and_int(fixnum: &ResolvedType, int: &ResolvedType) -> bool {
    matches!(
        fixnum,
        ResolvedType { kind: ResolvedTypeKind::Interface {
            identity: DeclarationIdentity::Package { package, name, .. },
            ..
        } } if package.as_str() == "fixnum" && matches!(name.as_str(), "Int32" | "Int64")
    ) && int.interface("dart:core", "int")
}
