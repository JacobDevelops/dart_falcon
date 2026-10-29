use super::*;
use crate::resolve::semantic::ResolvedTypeKind;

/// Substitution interns equal source subgraphs within one pass. Unlike semantic
/// equality, its keys retain function-use provenance for diagnostics.
#[derive(Clone, Copy)]
pub(super) struct SubstitutionKey<'a>(pub(super) &'a ResolvedType);

impl PartialEq for SubstitutionKey<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_memoized_with_aliases(
            other.0,
            &mut FxHashSet::default(),
            AliasComparison::Substitution,
        )
    }
}

impl Eq for SubstitutionKey<'_> {}

/// Identifies shared source storage without hashing its descendants. Scalar
/// context stays in the key: nullable views and distinct function alias uses
/// may share all their child Arcs but require different substitution results.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum SubstitutionSourceKey<'a> {
    Interface(
        &'a DeclarationIdentity,
        BorrowedIdentity<'a, [ResolvedType]>,
        bool,
        bool,
        Option<BorrowedIdentity<'a, AliasProvenance>>,
    ),
    Function(
        BorrowedIdentity<'a, ResolvedType>,
        BorrowedIdentity<'a, [ResolvedType]>,
        BorrowedIdentity<'a, [(String, ResolvedType)]>,
        usize,
        bool,
        Option<BorrowedIdentity<'a, AliasProvenance>>,
        Option<BorrowedIdentity<'a, FunctionTypeShape>>,
    ),
    Record(
        BorrowedIdentity<'a, [ResolvedType]>,
        BorrowedIdentity<'a, [(String, ResolvedType)]>,
        bool,
        Option<BorrowedIdentity<'a, AliasProvenance>>,
    ),
}

impl<'a> SubstitutionSourceKey<'a> {
    pub(super) fn of(ty: &'a ResolvedType) -> Option<Self> {
        match ty {
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        occurrence,
                        identity,
                        arguments,
                        nullable,
                        extension_type,
                    },
            } if !arguments.is_empty() => Some(Self::Interface(
                identity,
                BorrowedIdentity::new(arguments.as_ref()),
                *nullable,
                *extension_type,
                occurrence.0.as_deref().map(BorrowedIdentity::new),
            )),
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type,
                        positional,
                        named,
                        optional_positional,
                        nullable,
                        alias,
                        shape,
                    },
            } => Some(Self::Function(
                BorrowedIdentity::new(return_type.as_ref()),
                BorrowedIdentity::new(positional.as_ref()),
                BorrowedIdentity::new(named.as_ref()),
                *optional_positional,
                *nullable,
                alias.0.as_deref().map(BorrowedIdentity::new),
                shape.as_deref().map(BorrowedIdentity::new),
            )),
            ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        occurrence,
                        positional,
                        named,
                        nullable,
                    },
            } => Some(Self::Record(
                BorrowedIdentity::new(positional.as_ref()),
                BorrowedIdentity::new(named.as_ref()),
                *nullable,
                occurrence.0.as_deref().map(BorrowedIdentity::new),
            )),
            _ => None,
        }
    }
}

/// All memo tables are scoped to one fixed substitution map. Source references
/// keep their Arc storage alive throughout the pass; replacement types are never
/// traversed, so their parameters cannot leak into this pass's relevance cache.
#[derive(Default)]
pub(super) struct SubstitutionMemo<'a> {
    pub(super) relevant: FxHashMap<SubstitutionSourceKey<'a>, bool>,
    pub(super) sources: FxHashMap<SubstitutionSourceKey<'a>, ResolvedType>,
    pub(super) hashes: FxHashMap<BorrowedHashKey<'a>, u64>,
    pub(super) structural: FxHashMap<u64, Vec<(SubstitutionKey<'a>, ResolvedType)>>,
}

impl<'a> SubstitutionMemo<'a> {
    pub(super) fn is_relevant(
        &mut self,
        ty: &'a ResolvedType,
        substitutions: &FxHashMap<TypeParameterId, ResolvedType>,
        cache_result: bool,
    ) -> bool {
        let key = SubstitutionSourceKey::of(ty);
        if let Some(relevant) = key.and_then(|key| self.relevant.get(&key)) {
            return *relevant;
        }
        // Match substitution's free-parameter boundary, not mentions_any's
        // bound traversal: an unreplaced parameter retains its original bound.
        let relevant = match ty {
            ResolvedType { kind: ResolvedTypeKind::TypeParameter { id, .. } }
            | ResolvedType { kind: ResolvedTypeKind::NonNullTypeParameter { id, .. } } => substitutions.contains_key(id),
            ResolvedType { kind: ResolvedTypeKind::Interface { arguments, .. } } => arguments
                .iter()
                .any(|ty| self.is_relevant(ty, substitutions, true)),
            ResolvedType { kind: ResolvedTypeKind::Function {
                return_type,
                positional,
                named,
                shape,
                ..
            } } => {
                self.is_relevant(return_type, substitutions, true)
                    || positional
                        .iter()
                        .any(|ty| self.is_relevant(ty, substitutions, true))
                    || named
                        .iter()
                        .any(|(_, ty)| self.is_relevant(ty, substitutions, true))
                    || shape.as_ref().is_some_and(|shape| shape.type_parameters.iter().any(|ty| {
                        matches!(ty, ResolvedType { kind: ResolvedTypeKind::TypeParameter { id, bound, .. } }
                            if substitutions.contains_key(id) || self.is_relevant(bound, substitutions, true))
                    }))
            }
            ResolvedType { kind: ResolvedTypeKind::Record {
                positional, named, ..
            } } => {
                positional
                    .iter()
                    .any(|ty| self.is_relevant(ty, substitutions, true))
                    || named
                        .iter()
                        .any(|(_, ty)| self.is_relevant(ty, substitutions, true))
            }
            _ => false,
        };
        if cache_result && let Some(key) = key {
            self.relevant.insert(key, relevant);
        }
        relevant
    }
}

pub(super) fn instantiate_parameter_bound(
    id: &TypeParameterId,
    bounds: &FxHashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    let _ = model;
    instantiate_bound(id, bounds, visiting, true)
}

fn instantiate_bound(
    id: &TypeParameterId,
    bounds: &FxHashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
) -> ResolvedType {
    if !visiting.insert(id.clone()) {
        return if covariant {
            ResolvedType::DYNAMIC
        } else {
            ResolvedType::NEVER
        };
    }
    let result = bounds.get(id).map_or(ResolvedType::DYNAMIC, |bound| {
        instantiate_bound_type(bound, bounds, visiting, covariant)
    });
    visiting.remove(id);
    result
}

/// Recursive occurrences use the top or bottom type according to their
/// variance. Function parameters reverse variance; result and record fields
/// preserve it, as do Dart's covariant interface type arguments.
fn instantiate_bound_type(
    ty: &ResolvedType,
    bounds: &FxHashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
) -> ResolvedType {
    instantiate_bound_type_memoized(ty, bounds, visiting, covariant, &mut FxHashMap::default())
}

fn instantiate_bound_type_memoized<'a>(
    ty: &'a ResolvedType,
    bounds: &FxHashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
    memo: &mut FxHashMap<(BorrowedIdentity<'a, ResolvedType>, bool), ResolvedType>,
) -> ResolvedType {
    let key = (BorrowedIdentity::new(ty), covariant);
    if let Some(result) = memo.get(&key) {
        return result.clone();
    }
    let result = match ty {
        ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { id, nullable, .. },
        } if bounds.contains_key(id) => {
            let result = if bound_reaches_visiting(id, bounds, visiting, &mut HashSet::new()) {
                if covariant {
                    ResolvedType::DYNAMIC
                } else {
                    ResolvedType::NEVER
                }
            } else {
                instantiate_bound(id, bounds, visiting, true)
            };
            if *nullable {
                result.with_nullable(true)
            } else {
                result
            }
        }
        ResolvedType {
            kind: ResolvedTypeKind::NonNullTypeParameter { id, .. },
        } if bounds.contains_key(id) => {
            let result = if bound_reaches_visiting(id, bounds, visiting, &mut HashSet::new()) {
                if covariant {
                    ResolvedType::DYNAMIC
                } else {
                    ResolvedType::NEVER
                }
            } else {
                instantiate_bound(id, bounds, visiting, true)
            };
            result.non_null()
        }
        _ => instantiate_bound_children(ty, bounds, visiting, covariant, memo),
    };
    if matches!(
        ty,
        ResolvedType {
            kind: ResolvedTypeKind::Interface { .. }
        } | ResolvedType {
            kind: ResolvedTypeKind::Function { .. }
        } | ResolvedType {
            kind: ResolvedTypeKind::Record { .. }
        } | ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { .. }
        } | ResolvedType {
            kind: ResolvedTypeKind::NonNullTypeParameter { .. }
        }
    ) {
        memo.insert(key, result.clone());
    }
    result
}

fn instantiate_bound_children<'a>(
    ty: &'a ResolvedType,
    bounds: &FxHashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
    memo: &mut FxHashMap<(BorrowedIdentity<'a, ResolvedType>, bool), ResolvedType>,
) -> ResolvedType {
    let mut result = ty.clone();
    // Read original nodes rather than copy-on-write destinations: memo
    // keys must remain valid for this entire traversal.
    match (ty, &mut result) {
        (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        arguments: source, ..
                    },
            },
            ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            },
        ) => {
            *arguments = source
                .iter()
                .map(|argument| {
                    instantiate_bound_type_memoized(argument, bounds, visiting, covariant, memo)
                })
                .collect();
        }
        (
            ResolvedType {
                kind: ResolvedTypeKind::Function { .. },
            },
            _,
        ) => {
            instantiate_function_bound_children(ty, &mut result, bounds, visiting, covariant, memo);
        }
        (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        positional: source_positional,
                        named: source_named,
                        ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        positional, named, ..
                    },
            },
        ) => {
            *positional = source_positional
                .iter()
                .map(|field| {
                    instantiate_bound_type_memoized(field, bounds, visiting, covariant, memo)
                })
                .collect();
            *named = source_named
                .iter()
                .map(|(name, field)| {
                    (
                        name.clone(),
                        instantiate_bound_type_memoized(field, bounds, visiting, covariant, memo),
                    )
                })
                .collect();
        }
        _ => {}
    }
    result
}

fn instantiate_function_bound_children<'a>(
    ty: &'a ResolvedType,
    result: &mut ResolvedType,
    bounds: &FxHashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
    memo: &mut FxHashMap<(BorrowedIdentity<'a, ResolvedType>, bool), ResolvedType>,
) {
    if let (
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    return_type: source_return,
                    positional: source_positional,
                    named: source_named,
                    shape: source_shape,
                    ..
                },
        },
        ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    return_type,
                    positional,
                    named,
                    shape,
                    ..
                },
        },
    ) = (ty, result)
    {
        *shape = source_shape.as_ref().map(|shape| {
            shape.map_bounds(|bound| {
                instantiate_bound_type_memoized(bound, bounds, visiting, covariant, memo)
            })
        });
        *return_type = Arc::new(instantiate_bound_type_memoized(
            source_return,
            bounds,
            visiting,
            covariant,
            memo,
        ));
        *positional = source_positional
            .iter()
            .map(|parameter| {
                instantiate_bound_type_memoized(parameter, bounds, visiting, !covariant, memo)
            })
            .collect();
        *named = source_named
            .iter()
            .map(|(name, parameter)| {
                (
                    name.clone(),
                    instantiate_bound_type_memoized(parameter, bounds, visiting, !covariant, memo),
                )
            })
            .collect();
    }
}

fn bound_reaches_visiting(
    id: &TypeParameterId,
    bounds: &FxHashMap<TypeParameterId, ResolvedType>,
    visiting: &HashSet<TypeParameterId>,
    seen: &mut HashSet<TypeParameterId>,
) -> bool {
    if visiting.contains(id) {
        return true;
    }
    if !seen.insert(id.clone()) {
        return false;
    }
    let Some(bound) = bounds.get(id) else {
        return false;
    };
    let mut dependencies = Vec::new();
    collect_parameter_ids(bound, &mut dependencies);
    dependencies
        .iter()
        .any(|dependency| bound_reaches_visiting(dependency, bounds, visiting, seen))
}
