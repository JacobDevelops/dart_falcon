use super::*;

/// Address identity whose referent must remain live for the cache's lifetime.
pub(super) struct BorrowedIdentity<'a, T: ?Sized>(&'a T);

impl<'a, T: ?Sized> BorrowedIdentity<'a, T> {
    pub(super) fn new(value: &'a T) -> Self {
        Self(value)
    }
}

impl<T: ?Sized> Copy for BorrowedIdentity<'_, T> {}

impl<T: ?Sized> Clone for BorrowedIdentity<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> PartialEq for BorrowedIdentity<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self.0, other.0)
    }
}

impl<T: ?Sized> Eq for BorrowedIdentity<'_, T> {}

impl<T: ?Sized> Hash for BorrowedIdentity<'_, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(self.0, state);
    }
}

/// Persistent address identity owns only the existing shared storage.
pub(super) struct SharedIdentity<T: ?Sized>(Arc<T>);

impl<T: ?Sized> SharedIdentity<T> {
    pub(super) fn new(value: &Arc<T>) -> Self {
        Self(Arc::clone(value))
    }
}

impl<T: ?Sized> Clone for SharedIdentity<T> {
    fn clone(&self) -> Self {
        Self::new(&self.0)
    }
}

impl<T: ?Sized> PartialEq for SharedIdentity<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: ?Sized> Eq for SharedIdentity<T> {}

impl<T: ?Sized> Hash for SharedIdentity<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::ptr::hash(Arc::as_ptr(&self.0), state);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum BorrowedHashKey<'a> {
    Type(BorrowedIdentity<'a, ResolvedType>),
    Positional(BorrowedIdentity<'a, [ResolvedType]>),
    Named(BorrowedIdentity<'a, [(String, ResolvedType)]>),
    Shape(BorrowedIdentity<'a, FunctionTypeShape>),
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum SharedHashKey {
    Type(SharedIdentity<ResolvedType>),
    Positional(SharedIdentity<[ResolvedType]>),
    Named(SharedIdentity<[(String, ResolvedType)]>),
    Shape(SharedIdentity<FunctionTypeShape>),
}

trait StructuralHashKey<'a>: Clone + Eq + Hash {
    fn ty(value: &'a Arc<ResolvedType>) -> Self;
    fn positional(value: &'a Arc<[ResolvedType]>) -> Self;
    fn named(value: &'a Arc<[(String, ResolvedType)]>) -> Self;
    fn shape(value: &'a Arc<FunctionTypeShape>) -> Self;
}

impl<'a> StructuralHashKey<'a> for BorrowedHashKey<'a> {
    fn ty(value: &'a Arc<ResolvedType>) -> Self {
        Self::Type(BorrowedIdentity::new(value.as_ref()))
    }
    fn positional(value: &'a Arc<[ResolvedType]>) -> Self {
        Self::Positional(BorrowedIdentity::new(value.as_ref()))
    }
    fn named(value: &'a Arc<[(String, ResolvedType)]>) -> Self {
        Self::Named(BorrowedIdentity::new(value.as_ref()))
    }
    fn shape(value: &'a Arc<FunctionTypeShape>) -> Self {
        Self::Shape(BorrowedIdentity::new(value.as_ref()))
    }
}

impl<'a> StructuralHashKey<'a> for SharedHashKey {
    fn ty(value: &'a Arc<ResolvedType>) -> Self {
        Self::Type(SharedIdentity::new(value))
    }
    fn positional(value: &'a Arc<[ResolvedType]>) -> Self {
        Self::Positional(SharedIdentity::new(value))
    }
    fn named(value: &'a Arc<[(String, ResolvedType)]>) -> Self {
        Self::Named(SharedIdentity::new(value))
    }
    fn shape(value: &'a Arc<FunctionTypeShape>) -> Self {
        Self::Shape(SharedIdentity::new(value))
    }
}

struct StructuralHashMemo<'a, K> {
    hashes: &'a mut FxHashMap<K, u64>,
    inserted: Option<&'a mut Vec<K>>,
}

fn memoized_hash<K: Clone + Eq + Hash>(
    key: K,
    memo: &mut StructuralHashMemo<'_, K>,
    compute: impl FnOnce(&mut StructuralHashMemo<'_, K>) -> u64,
) -> u64 {
    if let Some(hash) = memo.hashes.get(&key) {
        return *hash;
    }
    let hash = compute(memo);
    if let Some(inserted) = &mut memo.inserted {
        inserted.push(key.clone());
    }
    memo.hashes.insert(key, hash);
    hash
}

type AliasParameter = (usize, TypeParameterId, ResolvedType);

pub(super) type AliasTemplate = (Vec<AliasParameter>, ResolvedType);

pub(super) fn empty_type_arguments() -> Arc<[ResolvedType]> {
    static EMPTY: std::sync::LazyLock<Arc<[ResolvedType]>> =
        std::sync::LazyLock::new(|| Arc::from([]));
    Arc::clone(&EMPTY)
}

pub(super) fn empty_named_types() -> Arc<[(String, ResolvedType)]> {
    static EMPTY: std::sync::LazyLock<Arc<[(String, ResolvedType)]>> =
        std::sync::LazyLock::new(|| Arc::from([]));
    Arc::clone(&EMPTY)
}

/// The declaration that introduces a type-parameter list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParameterOwner {
    /// Keyed by the source offset of the list's first parameter.
    Source {
        file: Option<FileId>,
        offset: usize,
    },
    Sdk(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeParameterId {
    pub owner: ParameterOwner,
    pub ordinal: usize,
    pub name: Arc<str>,
}

/// Typedef provenance is metadata, not part of structural function-type equality.
#[derive(Debug, Clone, Default, Eq)]
pub struct FunctionTypeAlias(pub(super) Option<Arc<AliasProvenance>>);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum AliasProvenance {
    Source {
        identity: DeclarationIdentity,
        // Clones of one type use retain analyzer type identity; separate uses do not.
        origin: Option<(Option<FileId>, usize)>,
    },
    Substituted {
        source: Arc<AliasProvenance>,
    },
    /// A lazily selected descendant of an instantiated function alias. The
    /// owner distinguishes sibling uses without copying their shared type DAG.
    Projected {
        source: Arc<AliasProvenance>,
        owner: Arc<AliasProvenance>,
    },
    /// An unaliased function still carries its enclosing occurrence to descendants.
    Context {
        owner: Arc<AliasProvenance>,
    },
}

impl AliasProvenance {
    fn identity(&self) -> Option<&DeclarationIdentity> {
        match self {
            Self::Source { identity, .. } => Some(identity),
            Self::Substituted { source } | Self::Projected { source, .. } => source.identity(),
            Self::Context { .. } => None,
        }
    }

    fn same_origin(left: &Arc<Self>, right: &Arc<Self>) -> bool {
        Self::same_origin_memoized(left, right, &mut FxHashSet::default())
    }

    pub(super) fn same_origin_memoized<'a>(
        left: &'a Arc<Self>,
        right: &'a Arc<Self>,
        seen: &mut FxHashSet<(BorrowedIdentity<'a, Self>, BorrowedIdentity<'a, Self>)>,
    ) -> bool {
        if Arc::ptr_eq(left, right) {
            return true;
        }
        match (left.as_ref(), right.as_ref()) {
            (Self::Source { .. }, Self::Source { .. }) => left == right,
            (Self::Context { owner: left }, Self::Context { owner: right }) => {
                Self::same_origin_memoized(left, right, seen)
            }
            (
                Self::Projected {
                    source: left_source,
                    owner: left_owner,
                },
                Self::Projected {
                    source: right_source,
                    owner: right_owner,
                },
            ) => {
                if !seen.insert((
                    BorrowedIdentity::new(left.as_ref()),
                    BorrowedIdentity::new(right.as_ref()),
                )) {
                    return true;
                }
                Self::same_origin_memoized(left_source, right_source, seen)
                    && Self::same_origin_memoized(left_owner, right_owner, seen)
            }
            _ => false,
        }
    }
}

impl FunctionTypeAlias {
    pub fn identity(&self) -> Option<&DeclarationIdentity> {
        self.0.as_ref().and_then(|alias| alias.identity())
    }

    pub(super) fn same_origin(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => AliasProvenance::same_origin(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    /// Substitution may intern structurally equal descendants from separate
    /// passes over one cached source use. This is deliberately weaker than
    /// `same_origin`: inference joins must retain distinct instantiated uses.
    pub(super) fn same_substitution_source(&self, other: &Self) -> bool {
        if self.same_origin(other) {
            return true;
        }
        match (self.0.as_deref(), other.0.as_deref()) {
            (
                Some(AliasProvenance::Substituted { source: left }),
                Some(AliasProvenance::Substituted { source: right }),
            ) => Arc::ptr_eq(left, right),
            _ => false,
        }
    }

    pub(super) fn projected_from(&self, owner: &Self) -> Self {
        match (&self.0, &owner.0) {
            (Some(source), Some(owner)) => Self(Some(Arc::new(AliasProvenance::Projected {
                source: Arc::clone(source),
                owner: Arc::clone(owner),
            }))),
            (None, Some(owner)) => Self(Some(Arc::new(AliasProvenance::Context {
                owner: Arc::clone(owner),
            }))),
            _ => self.clone(),
        }
    }

    pub(super) fn substituted(&self) -> Self {
        Self(self.0.as_ref().map(|alias| {
            // Keep the original source directly, never a chain of substitutions.
            let source = match alias.as_ref() {
                AliasProvenance::Source { .. }
                | AliasProvenance::Projected { .. }
                | AliasProvenance::Context { .. } => alias,
                AliasProvenance::Substituted { source } => source,
            };
            Arc::new(AliasProvenance::Substituted {
                source: Arc::clone(source),
            })
        }))
    }
}

impl PartialEq for FunctionTypeAlias {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

/// Exact declaration shape; `Function::shape == None` means unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionTypeShape {
    pub required_named: Arc<[String]>,
    /// Declared binders (identity and bound), in declaration order.
    pub type_parameters: Arc<[ResolvedType]>,
}

impl FunctionTypeShape {
    pub fn empty() -> Arc<Self> {
        static EMPTY: std::sync::LazyLock<Arc<FunctionTypeShape>> =
            std::sync::LazyLock::new(|| {
                Arc::new(FunctionTypeShape {
                    required_named: Arc::from([]),
                    type_parameters: empty_type_arguments(),
                })
            });
        Arc::clone(&EMPTY)
    }

    pub fn new(mut required_named: Vec<String>, type_parameters: Vec<ResolvedType>) -> Arc<Self> {
        if required_named.is_empty() && type_parameters.is_empty() {
            return Self::empty();
        }
        required_named.sort_unstable();
        Arc::new(Self {
            required_named: if required_named.is_empty() {
                Arc::clone(&Self::empty().required_named)
            } else {
                required_named.into()
            },
            type_parameters: if type_parameters.is_empty() {
                empty_type_arguments()
            } else {
                type_parameters.into()
            },
        })
    }
    /// Transforms binder bounds without replacing their declaration identities.
    /// Unchanged shapes retain their shared storage.
    pub fn map_bounds<'a>(
        self: &'a Arc<Self>,
        mut map: impl FnMut(&'a ResolvedType) -> ResolvedType,
    ) -> Arc<Self> {
        let mut mapped = None;
        for (index, parameter) in self.type_parameters.iter().enumerate() {
            let ResolvedType::TypeParameter {
                id,
                bound,
                nullable,
            } = parameter
            else {
                continue;
            };
            let result = map(bound);
            if result != **bound {
                let parameters = mapped.get_or_insert_with(|| self.type_parameters.to_vec());
                parameters[index] = ResolvedType::TypeParameter {
                    id: id.clone(),
                    bound: Arc::new(result),
                    nullable: *nullable,
                };
            }
        }
        mapped.map_or_else(
            || Arc::clone(self),
            |parameters| {
                Arc::new(Self {
                    required_named: Arc::clone(&self.required_named),
                    type_parameters: parameters.into(),
                })
            },
        )
    }

    /// Instantiated binders cease to be generic; retained binders keep their
    /// identities while substitutions apply to their bounds.
    pub(super) fn substitute<'a>(
        self: &'a Arc<Self>,
        substitutions: &FxHashMap<TypeParameterId, ResolvedType>,
        map: impl FnMut(&'a ResolvedType) -> ResolvedType,
    ) -> Arc<Self> {
        let mapped = self.map_bounds(map);
        if !self.type_parameters.iter().any(|parameter| {
            matches!(parameter, ResolvedType::TypeParameter { id, .. } if substitutions.contains_key(id))
        }) {
            return mapped;
        }
        Self::new(
            mapped.required_named.to_vec(),
            mapped.type_parameters.iter().filter(|parameter| {
                !matches!(parameter, ResolvedType::TypeParameter { id, .. } if substitutions.contains_key(id))
            }).cloned().collect(),
        )
    }

    pub(super) fn from_scope<'tree>(
        required_named: Vec<String>,
        parameters: impl Iterator<Item = TypeParam<'tree>>,
        scope: &TypeParameterScope,
    ) -> Option<Arc<Self>> {
        let parameters = parameters
            .map(|parameter| scope.lookup(parameter.name().name()))
            .collect::<Option<Vec<_>>>()?;
        Some(Self::new(required_named, parameters))
    }
}

#[derive(Debug, Clone)]
pub enum ResolvedType {
    Unknown,
    Dynamic,
    Void,
    Never,
    Null,
    Interface {
        occurrence: FunctionTypeAlias,
        identity: DeclarationIdentity,
        arguments: Arc<[ResolvedType]>,
        nullable: bool,
        extension_type: bool,
    },
    TypeParameter {
        id: TypeParameterId,
        bound: Arc<ResolvedType>,
        nullable: bool,
    },
    /// The promoted intersection `T & Object`, distinct from a potentially nullable `T`.
    NonNullTypeParameter {
        id: TypeParameterId,
        bound: Arc<ResolvedType>,
    },
    Function {
        shape: Option<Arc<FunctionTypeShape>>,
        alias: FunctionTypeAlias,
        /// Optional positional arity, retained for function UP shape checks.
        optional_positional: usize,
        return_type: Arc<ResolvedType>,
        positional: Arc<[ResolvedType]>,
        named: Arc<[(String, ResolvedType)]>,
        nullable: bool,
    },
    Record {
        occurrence: FunctionTypeAlias,
        positional: Arc<[ResolvedType]>,
        named: Arc<[(String, ResolvedType)]>,
        nullable: bool,
    },
}

impl PartialEq for ResolvedType {
    fn eq(&self, other: &Self) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        self.eq_memoized(other, &mut FxHashSet::default())
    }
}

impl Eq for ResolvedType {}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum AliasComparison {
    Ignore,
    Substitution,
    Origin,
}

impl ResolvedType {
    pub(super) fn eq_memoized<'a>(
        &'a self,
        other: &'a Self,
        seen: &mut FxHashSet<(BorrowedIdentity<'a, Self>, BorrowedIdentity<'a, Self>)>,
    ) -> bool {
        self.eq_memoized_with_aliases(other, seen, AliasComparison::Ignore)
    }

    pub(super) fn eq_memoized_with_aliases<'a>(
        &'a self,
        other: &'a Self,
        seen: &mut FxHashSet<(BorrowedIdentity<'a, Self>, BorrowedIdentity<'a, Self>)>,
        compare_aliases: AliasComparison,
    ) -> bool {
        if std::ptr::eq(self, other) {
            return true;
        }
        // Reject scalar differences, and accept already-shared children, before
        // allocating a memo table. Flat SDK interface comparisons stay allocation-free.
        match (self, other) {
            (
                Self::Interface {
                    occurrence: a_occurrence,
                    identity: a,
                    arguments: aa,
                    nullable: an,
                    extension_type: ae,
                },
                Self::Interface {
                    occurrence: b_occurrence,
                    identity: b,
                    arguments: ba,
                    nullable: bn,
                    extension_type: be,
                },
            ) => {
                if compare_aliases != AliasComparison::Ignore
                    && !a_occurrence.same_origin(b_occurrence)
                {
                    return false;
                }
                if a != b || an != bn || ae != be || aa.len() != ba.len() {
                    return false;
                }
                if aa.is_empty() || Arc::ptr_eq(aa, ba) {
                    return true;
                }
            }
            (
                Self::TypeParameter {
                    id: a,
                    bound: ab,
                    nullable: an,
                },
                Self::TypeParameter {
                    id: b,
                    bound: bb,
                    nullable: bn,
                },
            ) => {
                if a != b || an != bn {
                    return false;
                }
                if Arc::ptr_eq(ab, bb) {
                    return true;
                }
            }
            (
                Self::NonNullTypeParameter { id: a, bound: ab },
                Self::NonNullTypeParameter { id: b, bound: bb },
            ) => {
                if a != b {
                    return false;
                }
                if Arc::ptr_eq(ab, bb) {
                    return true;
                }
            }
            (
                Self::Function {
                    optional_positional: ao,
                    return_type: ar,
                    positional: ap,
                    named: a,
                    nullable: an,
                    alias: a_alias,
                    shape: a_shape,
                    ..
                },
                Self::Function {
                    optional_positional: bo,
                    return_type: br,
                    positional: bp,
                    named: b,
                    nullable: bn,
                    alias: b_alias,
                    shape: b_shape,
                    ..
                },
            ) => {
                if match compare_aliases {
                    AliasComparison::Ignore => false,
                    AliasComparison::Substitution => !a_alias.same_substitution_source(b_alias),
                    AliasComparison::Origin => !a_alias.same_origin(b_alias),
                } {
                    return false;
                }
                if a_shape != b_shape {
                    return false;
                }
                if ao != bo || an != bn || ap.len() != bp.len() || a.len() != b.len() {
                    return false;
                }
                if Arc::ptr_eq(ar, br)
                    && (ap.is_empty() || Arc::ptr_eq(ap, bp))
                    && (a.is_empty() || Arc::ptr_eq(a, b))
                {
                    return true;
                }
            }
            (
                Self::Record {
                    occurrence: a_occurrence,
                    positional: ap,
                    named: a,
                    nullable: an,
                },
                Self::Record {
                    occurrence: b_occurrence,
                    positional: bp,
                    named: b,
                    nullable: bn,
                },
            ) => {
                if compare_aliases != AliasComparison::Ignore
                    && !a_occurrence.same_origin(b_occurrence)
                {
                    return false;
                }
                if an != bn || ap.len() != bp.len() || a.len() != b.len() {
                    return false;
                }
                if (ap.is_empty() || Arc::ptr_eq(ap, bp)) && (a.is_empty() || Arc::ptr_eq(a, b)) {
                    return true;
                }
            }
            (Self::Unknown, Self::Unknown)
            | (Self::Dynamic, Self::Dynamic)
            | (Self::Void, Self::Void)
            | (Self::Never, Self::Never)
            | (Self::Null, Self::Null) => {
                return true;
            }
            _ => return false,
        }
        if !seen.insert((BorrowedIdentity::new(self), BorrowedIdentity::new(other))) {
            return true;
        }
        match (self, other) {
            (Self::Interface { arguments: a, .. }, Self::Interface { arguments: b, .. }) => {
                Self::eq_types(a, b, seen, compare_aliases)
            }
            (Self::TypeParameter { bound: a, .. }, Self::TypeParameter { bound: b, .. })
            | (
                Self::NonNullTypeParameter { bound: a, .. },
                Self::NonNullTypeParameter { bound: b, .. },
            ) => a.eq_memoized_with_aliases(b, seen, compare_aliases),
            (
                Self::Function {
                    return_type: ar,
                    positional: ap,
                    named: a,
                    ..
                },
                Self::Function {
                    return_type: br,
                    positional: bp,
                    named: b,
                    ..
                },
            ) => {
                ar.eq_memoized_with_aliases(br, seen, compare_aliases)
                    && Self::eq_types(ap, bp, seen, compare_aliases)
                    && Self::eq_named(a, b, seen, compare_aliases)
            }
            (
                Self::Record {
                    positional: ap,
                    named: a,
                    ..
                },
                Self::Record {
                    positional: bp,
                    named: b,
                    ..
                },
            ) => {
                Self::eq_types(ap, bp, seen, compare_aliases)
                    && Self::eq_named(a, b, seen, compare_aliases)
            }
            _ => false,
        }
    }

    fn eq_types<'a>(
        left: &'a [Self],
        right: &'a [Self],
        seen: &mut FxHashSet<(BorrowedIdentity<'a, Self>, BorrowedIdentity<'a, Self>)>,
        compare_aliases: AliasComparison,
    ) -> bool {
        std::ptr::eq(left, right)
            || (left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(a, b)| a.eq_memoized_with_aliases(b, seen, compare_aliases)))
    }

    fn eq_named<'a>(
        left: &'a [(String, Self)],
        right: &'a [(String, Self)],
        seen: &mut FxHashSet<(BorrowedIdentity<'a, Self>, BorrowedIdentity<'a, Self>)>,
        compare_aliases: AliasComparison,
    ) -> bool {
        std::ptr::eq(left, right)
            || (left.len() == right.len()
                && left.iter().zip(right).all(|((a, at), (b, bt))| {
                    a == b && at.eq_memoized_with_aliases(bt, seen, compare_aliases)
                }))
    }

    pub(super) fn structural_hash<'a>(
        &'a self,
        memo: &mut FxHashMap<BorrowedHashKey<'a>, u64>,
    ) -> u64 {
        self.structural_hash_inner(&mut StructuralHashMemo {
            hashes: memo,
            inserted: None,
        })
    }

    #[cfg(test)]
    pub(super) fn structural_hash_shared(&self, memo: &mut FxHashMap<SharedHashKey, u64>) -> u64 {
        self.structural_hash_inner(&mut StructuralHashMemo {
            hashes: memo,
            inserted: None,
        })
    }

    pub(super) fn structural_hash_shared_tracked(
        &self,
        memo: &mut FxHashMap<SharedHashKey, u64>,
        inserted: &mut Vec<SharedHashKey>,
    ) -> u64 {
        self.structural_hash_inner(&mut StructuralHashMemo {
            hashes: memo,
            inserted: Some(inserted),
        })
    }

    fn structural_hash_inner<'a, K: StructuralHashKey<'a>>(
        &'a self,
        memo: &mut StructuralHashMemo<'_, K>,
    ) -> u64 {
        let mut state = rustc_hash::FxHasher::default();
        std::mem::discriminant(self).hash(&mut state);
        match self {
            Self::Interface {
                identity,
                arguments,
                nullable,
                extension_type,
                ..
            } => {
                identity.hash(&mut state);
                nullable.hash(&mut state);
                extension_type.hash(&mut state);
                Self::hash_types(arguments, memo, &mut state);
            }
            Self::TypeParameter {
                id,
                bound,
                nullable,
            } => {
                id.hash(&mut state);
                nullable.hash(&mut state);
                Self::hash_shared_type(bound, memo).hash(&mut state);
            }
            Self::NonNullTypeParameter { id, bound } => {
                id.hash(&mut state);
                Self::hash_shared_type(bound, memo).hash(&mut state);
            }
            Self::Function {
                optional_positional,
                return_type,
                positional,
                named,
                nullable,
                shape,
                ..
            } => {
                optional_positional.hash(&mut state);
                nullable.hash(&mut state);
                shape.is_some().hash(&mut state);
                if let Some(shape) = shape {
                    let hash = if shape.type_parameters.is_empty() {
                        Self::hash_shape(shape, memo)
                    } else {
                        memoized_hash(K::shape(shape), memo, |memo| Self::hash_shape(shape, memo))
                    };
                    hash.hash(&mut state);
                }
                Self::hash_shared_type(return_type, memo).hash(&mut state);
                Self::hash_types(positional, memo, &mut state);
                Self::hash_named(named, memo, &mut state);
            }
            Self::Record {
                positional,
                named,
                nullable,
                ..
            } => {
                nullable.hash(&mut state);
                Self::hash_types(positional, memo, &mut state);
                Self::hash_named(named, memo, &mut state);
            }
            _ => return state.finish(),
        }
        state.finish()
    }

    fn hash_shape<'a, K: StructuralHashKey<'a>>(
        shape: &'a FunctionTypeShape,
        memo: &mut StructuralHashMemo<'_, K>,
    ) -> u64 {
        let mut state = rustc_hash::FxHasher::default();
        shape.required_named.hash(&mut state);
        Self::hash_types(&shape.type_parameters, memo, &mut state);
        state.finish()
    }

    fn hash_shared_type<'a, K: StructuralHashKey<'a>>(
        ty: &'a Arc<Self>,
        memo: &mut StructuralHashMemo<'_, K>,
    ) -> u64 {
        match ty.as_ref() {
            Self::Interface { arguments, .. } if arguments.is_empty() => {
                ty.structural_hash_inner(memo)
            }
            Self::Interface { .. }
            | Self::TypeParameter { .. }
            | Self::NonNullTypeParameter { .. }
            | Self::Function { .. }
            | Self::Record { .. } => {
                memoized_hash(K::ty(ty), memo, |memo| ty.structural_hash_inner(memo))
            }
            _ => ty.structural_hash_inner(memo),
        }
    }

    fn hash_types<'a, K: StructuralHashKey<'a>>(
        types: &'a Arc<[Self]>,
        memo: &mut StructuralHashMemo<'_, K>,
        state: &mut impl Hasher,
    ) {
        let compute = |memo: &mut StructuralHashMemo<'_, K>| {
            let mut state = rustc_hash::FxHasher::default();
            types.len().hash(&mut state);
            for ty in types.iter() {
                ty.structural_hash_inner(memo).hash(&mut state);
            }
            state.finish()
        };
        let hash = if types.is_empty() {
            compute(memo)
        } else {
            memoized_hash(K::positional(types), memo, compute)
        };
        hash.hash(state);
    }

    fn hash_named<'a, K: StructuralHashKey<'a>>(
        types: &'a Arc<[(String, Self)]>,
        memo: &mut StructuralHashMemo<'_, K>,
        state: &mut impl Hasher,
    ) {
        let compute = |memo: &mut StructuralHashMemo<'_, K>| {
            let mut state = rustc_hash::FxHasher::default();
            types.len().hash(&mut state);
            for (name, ty) in types.iter() {
                name.hash(&mut state);
                ty.structural_hash_inner(memo).hash(&mut state);
            }
            state.finish()
        };
        let hash = if types.is_empty() {
            compute(memo)
        } else {
            memoized_hash(K::named(types), memo, compute)
        };
        hash.hash(state);
    }
}

impl Hash for ResolvedType {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.structural_hash(&mut FxHashMap::default()).hash(state);
    }
}

/// Walk each shared node once. Parameter bounds are intentionally not children:
/// these queries inspect written occurrences, not dependencies of their bounds.
fn find_type<'a>(
    ty: &'a ResolvedType,
    predicate: &mut impl FnMut(&ResolvedType) -> bool,
    seen: &mut FxHashSet<BorrowedIdentity<'a, ResolvedType>>,
) -> Option<&'a ResolvedType> {
    find_type_including_bounds(ty, predicate, seen, false)
}

pub(super) fn find_type_including_bounds<'a>(
    ty: &'a ResolvedType,
    predicate: &mut impl FnMut(&ResolvedType) -> bool,
    seen: &mut FxHashSet<BorrowedIdentity<'a, ResolvedType>>,
    include_bounds: bool,
) -> Option<&'a ResolvedType> {
    if predicate(ty) {
        return Some(ty);
    }
    match ty {
        ResolvedType::Interface { arguments, .. } if arguments.is_empty() => return None,
        ResolvedType::Record {
            positional, named, ..
        } if positional.is_empty() && named.is_empty() => return None,
        ResolvedType::Interface { .. }
        | ResolvedType::Function { .. }
        | ResolvedType::Record { .. } => {
            if !seen.insert(BorrowedIdentity::new(ty)) {
                return None;
            }
        }
        ResolvedType::TypeParameter { .. } | ResolvedType::NonNullTypeParameter { .. }
            if include_bounds => {}
        _ => return None,
    }
    match ty {
        ResolvedType::TypeParameter { bound, .. }
        | ResolvedType::NonNullTypeParameter { bound, .. }
            if include_bounds =>
        {
            if !seen.insert(BorrowedIdentity::new(ty)) {
                return None;
            }
            find_type_including_bounds(bound, predicate, seen, true)
        }
        ResolvedType::Interface { arguments, .. } => arguments
            .iter()
            .find_map(|ty| find_type_including_bounds(ty, predicate, seen, include_bounds)),
        ResolvedType::Function {
            return_type,
            positional,
            named,
            shape,
            ..
        } => find_type_including_bounds(return_type, predicate, seen, include_bounds)
            .or_else(|| {
                positional
                    .iter()
                    .find_map(|ty| find_type_including_bounds(ty, predicate, seen, include_bounds))
            })
            .or_else(|| {
                named.iter().find_map(|(_, ty)| {
                    find_type_including_bounds(ty, predicate, seen, include_bounds)
                })
            })
            .or_else(|| {
                shape.as_ref().and_then(|shape| {
                    shape.type_parameters.iter().find_map(|parameter| {
                        let ResolvedType::TypeParameter { bound, .. } = parameter else {
                            return None;
                        };
                        find_type_including_bounds(bound, predicate, seen, include_bounds)
                    })
                })
            }),
        ResolvedType::Record {
            positional, named, ..
        } => positional
            .iter()
            .find_map(|ty| find_type_including_bounds(ty, predicate, seen, include_bounds))
            .or_else(|| {
                named.iter().find_map(|(_, ty)| {
                    find_type_including_bounds(ty, predicate, seen, include_bounds)
                })
            }),
        _ => None,
    }
}

pub(crate) fn has_inference_holes(ty: &ResolvedType) -> bool {
    find_type(
        ty,
        &mut |ty| matches!(ty, ResolvedType::Unknown),
        &mut FxHashSet::default(),
    )
    .is_some()
}

/// Publish an unknown once, or finish an existing partial without changing any
/// known slot or occurrence metadata. Partial-to-partial growth is forbidden:
/// recursive declarations must not build an unbounded tower of type arguments.
pub(super) fn refine_inferred_type(current: &mut ResolvedType, candidate: ResolvedType) -> bool {
    if index_fixpoint::inference_deferred() {
        return false;
    }
    if matches!(current, ResolvedType::Unknown) {
        if matches!(candidate, ResolvedType::Unknown) {
            return false;
        }
    } else if !has_inference_holes(current)
        || has_inference_holes(&candidate)
        || !inference_refinement_compatible(current, &candidate, &mut FxHashSet::default())
    {
        return false;
    }
    *current = candidate;
    true
}

fn inference_refinement_compatible<'a>(
    current: &'a ResolvedType,
    candidate: &'a ResolvedType,
    seen: &mut FxHashSet<(
        BorrowedIdentity<'a, ResolvedType>,
        BorrowedIdentity<'a, ResolvedType>,
    )>,
) -> bool {
    if !seen.insert((
        BorrowedIdentity::new(current),
        BorrowedIdentity::new(candidate),
    )) {
        return true;
    }
    let positional =
        |left: &'a [ResolvedType], right: &'a [ResolvedType], seen: &mut FxHashSet<_>| {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| inference_refinement_compatible(left, right, seen))
        };
    let named = |left: &'a [(String, ResolvedType)],
                 right: &'a [(String, ResolvedType)],
                 seen: &mut FxHashSet<_>| {
        left.len() == right.len()
            && left
                .iter()
                .zip(right)
                .all(|((left_name, left), (right_name, right))| {
                    left_name == right_name && inference_refinement_compatible(left, right, seen)
                })
    };
    match (current, candidate) {
        (ResolvedType::Unknown, _) => true,
        (
            ResolvedType::Interface {
                identity: left,
                arguments: left_arguments,
                nullable: left_nullable,
                extension_type: left_extension,
                occurrence: left_occurrence,
            },
            ResolvedType::Interface {
                identity: right,
                arguments: right_arguments,
                nullable: right_nullable,
                extension_type: right_extension,
                occurrence: right_occurrence,
            },
        ) => {
            left == right
                && left_nullable == right_nullable
                && left_extension == right_extension
                && left_occurrence.same_origin(right_occurrence)
                && positional(left_arguments, right_arguments, seen)
        }
        (
            ResolvedType::Function {
                shape: left_shape,
                alias: left_alias,
                optional_positional: left_optional,
                return_type: left_return,
                positional: left_positional,
                named: left_named,
                nullable: left_nullable,
            },
            ResolvedType::Function {
                shape: right_shape,
                alias: right_alias,
                optional_positional: right_optional,
                return_type: right_return,
                positional: right_positional,
                named: right_named,
                nullable: right_nullable,
            },
        ) => {
            left_nullable == right_nullable
                && left_optional == right_optional
                && left_alias.same_origin(right_alias)
                && match (left_shape, right_shape) {
                    (None, None) => true,
                    (Some(left), Some(right)) => {
                        left.required_named == right.required_named
                            && positional(&left.type_parameters, &right.type_parameters, seen)
                    }
                    _ => false,
                }
                && inference_refinement_compatible(left_return, right_return, seen)
                && positional(left_positional, right_positional, seen)
                && named(left_named, right_named, seen)
        }
        (
            ResolvedType::Record {
                occurrence: left_occurrence,
                positional: left_positional,
                named: left_named,
                nullable: left_nullable,
            },
            ResolvedType::Record {
                occurrence: right_occurrence,
                positional: right_positional,
                named: right_named,
                nullable: right_nullable,
            },
        ) => {
            left_nullable == right_nullable
                && left_occurrence.same_origin(right_occurrence)
                && positional(left_positional, right_positional, seen)
                && named(left_named, right_named, seen)
        }
        (
            ResolvedType::TypeParameter {
                id: left,
                bound: left_bound,
                nullable: left_nullable,
            },
            ResolvedType::TypeParameter {
                id: right,
                bound: right_bound,
                nullable: right_nullable,
            },
        ) => {
            left == right
                && left_nullable == right_nullable
                && inference_refinement_compatible(left_bound, right_bound, seen)
        }
        (
            ResolvedType::NonNullTypeParameter {
                id: left,
                bound: left_bound,
            },
            ResolvedType::NonNullTypeParameter {
                id: right,
                bound: right_bound,
            },
        ) => left == right && inference_refinement_compatible(left_bound, right_bound, seen),
        _ => current.eq_memoized_with_aliases(
            candidate,
            &mut FxHashSet::default(),
            AliasComparison::Origin,
        ),
    }
}

pub(super) fn collect_parameter_ids(ty: &ResolvedType, ids: &mut Vec<TypeParameterId>) {
    find_type(
        ty,
        &mut |ty| {
            if let ResolvedType::TypeParameter { id, .. }
            | ResolvedType::NonNullTypeParameter { id, .. } = ty
                && !ids.contains(id)
            {
                ids.push(id.clone());
            }
            false
        },
        &mut FxHashSet::default(),
    );
}

pub(super) fn parameter_bound<'a>(
    ty: &'a ResolvedType,
    parameter: &TypeParameterId,
) -> Option<&'a ResolvedType> {
    let occurrence = find_type(
        ty,
        &mut |ty| {
            matches!(
                ty,
                ResolvedType::TypeParameter { id, .. }
                    | ResolvedType::NonNullTypeParameter { id, .. } if id == parameter
            )
        },
        &mut FxHashSet::default(),
    )?;
    match occurrence {
        ResolvedType::TypeParameter { bound, .. }
        | ResolvedType::NonNullTypeParameter { bound, .. } => Some(bound),
        _ => None,
    }
}

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum AliasPresenceKey {
    Function(
        SharedIdentity<ResolvedType>,
        SharedIdentity<[ResolvedType]>,
        SharedIdentity<[(String, ResolvedType)]>,
    ),
    Interface(SharedIdentity<[ResolvedType]>),
    Record(
        SharedIdentity<[ResolvedType]>,
        SharedIdentity<[(String, ResolvedType)]>,
    ),
}

impl ResolvedType {
    fn alias_presence_key(&self) -> Option<AliasPresenceKey> {
        match self {
            Self::Function {
                return_type,
                positional,
                named,
                ..
            } => Some(AliasPresenceKey::Function(
                SharedIdentity::new(return_type),
                SharedIdentity::new(positional),
                SharedIdentity::new(named),
            )),
            Self::Interface { arguments, .. } => {
                Some(AliasPresenceKey::Interface(SharedIdentity::new(arguments)))
            }
            Self::Record {
                positional, named, ..
            } => Some(AliasPresenceKey::Record(
                SharedIdentity::new(positional),
                SharedIdentity::new(named),
            )),
            _ => None,
        }
    }

    pub(super) fn has_alias_provenance(&self) -> bool {
        self.has_alias_provenance_memoized(&mut FxHashMap::default(), &mut None)
    }

    fn has_alias_provenance_memoized(
        &self,
        memo: &mut FxHashMap<AliasPresenceKey, bool>,
        inserted: &mut Option<&mut Vec<AliasPresenceKey>>,
    ) -> bool {
        match self {
            Self::Function { alias, .. } if alias.0.is_some() => return true,
            Self::Interface { occurrence, .. } | Self::Record { occurrence, .. }
                if occurrence.0.is_some() =>
            {
                return true;
            }
            _ => {}
        }
        let Some(key) = self.alias_presence_key() else {
            return false;
        };
        if let Some(present) = memo.get(&key) {
            return *present;
        }
        let present = match self {
            Self::Function {
                return_type,
                positional,
                named,
                ..
            } => {
                return_type.has_alias_provenance_memoized(memo, inserted)
                    || positional
                        .iter()
                        .any(|ty| ty.has_alias_provenance_memoized(memo, inserted))
                    || named
                        .iter()
                        .any(|(_, ty)| ty.has_alias_provenance_memoized(memo, inserted))
            }
            Self::Interface { arguments, .. } => arguments
                .iter()
                .any(|ty| ty.has_alias_provenance_memoized(memo, inserted)),
            Self::Record {
                positional, named, ..
            } => {
                positional
                    .iter()
                    .any(|ty| ty.has_alias_provenance_memoized(memo, inserted))
                    || named
                        .iter()
                        .any(|(_, ty)| ty.has_alias_provenance_memoized(memo, inserted))
            }
            _ => false,
        };
        if let Some(inserted) = inserted {
            inserted.push(key.clone());
        }
        memo.insert(key, present);
        present
    }

    /// Select a descendant lazily without copying its shared type graph.
    pub(super) fn projected_from(self, owner: &FunctionTypeAlias) -> Self {
        self.projected_from_memoized(owner, &mut FxHashMap::default(), None)
    }

    pub(super) fn projected_from_memoized(
        mut self,
        owner: &FunctionTypeAlias,
        memo: &mut FxHashMap<AliasPresenceKey, bool>,
        mut inserted: Option<&mut Vec<AliasPresenceKey>>,
    ) -> Self {
        if owner.0.is_none() {
            return self;
        }
        if matches!(
            &self,
            Self::Function {
                alias: FunctionTypeAlias(None),
                ..
            } | Self::Interface {
                occurrence: FunctionTypeAlias(None),
                ..
            } | Self::Record {
                occurrence: FunctionTypeAlias(None),
                ..
            }
        ) && !self.has_alias_provenance_memoized(memo, &mut inserted)
        {
            return self;
        }
        match &mut self {
            Self::Function { alias, .. } => *alias = alias.projected_from(owner),
            Self::Interface {
                occurrence,
                arguments,
                ..
            } if !arguments.is_empty() => {
                *occurrence = if occurrence.0.is_none() {
                    owner.clone()
                } else {
                    occurrence.projected_from(owner)
                };
            }
            Self::Record {
                occurrence,
                positional,
                named,
                ..
            } if !positional.is_empty() || !named.is_empty() => {
                *occurrence = if occurrence.0.is_none() {
                    owner.clone()
                } else {
                    occurrence.projected_from(owner)
                };
            }
            _ => {}
        }
        self
    }

    pub fn function_return_type(&self) -> Option<Self> {
        let Self::Function {
            return_type, alias, ..
        } = self
        else {
            return None;
        };
        Some(return_type.as_ref().clone().projected_from(alias))
    }

    pub fn type_argument(&self, index: usize) -> Option<Self> {
        let Self::Interface {
            arguments,
            occurrence,
            ..
        } = self
        else {
            return None;
        };
        arguments
            .get(index)
            .map(|argument| argument.clone().projected_from(occurrence))
    }

    pub fn type_arguments(&self) -> impl Iterator<Item = Self> + '_ {
        (0..self.arguments().len()).filter_map(|index| self.type_argument(index))
    }

    pub fn record_positional(&self, index: usize) -> Option<Self> {
        let Self::Record {
            positional,
            occurrence,
            ..
        } = self
        else {
            return None;
        };
        positional
            .get(index)
            .map(|ty| ty.clone().projected_from(occurrence))
    }

    pub fn record_field(&self, name: &str) -> Option<Self> {
        let Self::Record {
            positional,
            named,
            occurrence,
            ..
        } = self
        else {
            return None;
        };
        name.strip_prefix('$')
            .and_then(|ordinal| ordinal.parse::<usize>().ok())
            .and_then(|ordinal| ordinal.checked_sub(1))
            .and_then(|ordinal| positional.get(ordinal))
            .or_else(|| {
                named
                    .iter()
                    .find(|(field, _)| field == name)
                    .map(|(_, ty)| ty)
            })
            .map(|ty| ty.clone().projected_from(occurrence))
    }

    pub fn interface_identity(identity: DeclarationIdentity) -> Self {
        Self::Interface {
            occurrence: Default::default(),
            identity,
            arguments: empty_type_arguments(),
            nullable: false,
            extension_type: false,
        }
    }

    pub fn nullable(&self) -> bool {
        match self {
            Self::Null => true,
            Self::Dynamic | Self::Unknown => true,
            Self::Interface { nullable, .. }
            | Self::TypeParameter { nullable, .. }
            | Self::Function { nullable, .. }
            | Self::Record { nullable, .. } => *nullable,
            Self::Void | Self::Never | Self::NonNullTypeParameter { .. } => false,
        }
    }

    pub fn with_nullable(&self, nullable: bool) -> Self {
        if self.nullable() == nullable {
            return self.clone();
        }
        match self {
            Self::Interface {
                occurrence,
                identity,
                arguments,
                extension_type,
                ..
            } => Self::Interface {
                occurrence: occurrence.clone(),
                identity: identity.clone(),
                arguments: arguments.clone(),
                nullable,
                extension_type: *extension_type,
            },
            Self::TypeParameter { id, bound, .. } => Self::TypeParameter {
                id: id.clone(),
                bound: bound.clone(),
                nullable,
            },
            Self::NonNullTypeParameter { id, bound } if nullable => Self::TypeParameter {
                id: id.clone(),
                bound: bound.clone(),
                nullable: true,
            },
            Self::Function {
                return_type,
                positional,
                named,
                alias,
                optional_positional,
                shape,
                ..
            } => Self::Function {
                shape: shape.clone(),
                optional_positional: *optional_positional,
                return_type: return_type.clone(),
                positional: positional.clone(),
                named: named.clone(),
                alias: alias.clone(),
                nullable,
            },
            Self::Record {
                positional,
                named,
                occurrence,
                ..
            } => Self::Record {
                occurrence: occurrence.clone(),
                positional: positional.clone(),
                named: named.clone(),
                nullable,
            },
            Self::Never if nullable => Self::Null,
            other => other.clone(),
        }
    }

    /// Whether this type provably excludes null without declaration context.
    /// Extension types require `SignatureIndex::is_non_nullable`.
    pub fn is_non_nullable(&self) -> bool {
        unrelated::non_nullable(self) == Truth::Yes
    }

    // Extension types expose their own members even when their values may be null.
    pub(super) fn has_instance_members(&self) -> bool {
        match self {
            Self::TypeParameter {
                bound, nullable, ..
            } => !nullable && bound.has_instance_members(),
            Self::Interface {
                extension_type: true,
                nullable,
                ..
            } => !nullable,
            ty => ty.is_non_nullable(),
        }
    }

    /// Dart's `NonNull`, rather than merely removing a written `?` suffix.
    pub fn non_null(&self) -> Self {
        match self {
            Self::TypeParameter { id, bound, .. } if !bound.is_non_nullable() => {
                Self::NonNullTypeParameter {
                    id: id.clone(),
                    bound: bound.clone(),
                }
            }
            Self::Null => Self::Never,
            other => other.with_nullable(false),
        }
    }

    pub fn interface(&self, library: &str, name: &str) -> bool {
        matches!(self, Self::Interface { identity, .. } if identity.is_sdk(library, name))
    }

    pub fn arguments(&self) -> &[ResolvedType] {
        match self {
            Self::Interface { arguments, .. } => arguments,
            _ => &[],
        }
    }
}

#[cfg(test)]
mod refinement_tests {
    use super::*;

    #[test]
    fn unknown_fills_once_including_explicit_dynamic() {
        let mut current = ResolvedType::Unknown;
        assert!(!refine_inferred_type(&mut current, ResolvedType::Unknown));
        assert!(refine_inferred_type(&mut current, ResolvedType::Dynamic));
        assert!(!refine_inferred_type(&mut current, ResolvedType::Void));
        assert_eq!(current, ResolvedType::Dynamic);
    }

    #[test]
    fn partials_only_accept_hole_free_structurally_compatible_candidates() {
        let record = |positional: Vec<ResolvedType>| ResolvedType::Record {
            occurrence: FunctionTypeAlias::default(),
            positional: positional.into(),
            named: empty_named_types(),
            nullable: false,
        };
        let partial = record(vec![ResolvedType::Dynamic, ResolvedType::Unknown]);
        let mut current = partial.clone();
        assert!(!refine_inferred_type(
            &mut current,
            record(vec![ResolvedType::Dynamic, partial.clone()]),
        ));
        assert!(!refine_inferred_type(
            &mut current,
            record(vec![ResolvedType::Void, ResolvedType::Never]),
        ));
        assert!(!refine_inferred_type(
            &mut current,
            record(vec![ResolvedType::Dynamic]),
        ));
        assert_eq!(current, partial);
        let complete = record(vec![ResolvedType::Dynamic, ResolvedType::Never]);
        assert!(refine_inferred_type(&mut current, complete.clone()));
        assert_eq!(current, complete);
        assert!(!refine_inferred_type(&mut current, ResolvedType::Dynamic));
    }

    #[test]
    fn function_refinement_freezes_known_slots_shape_and_provenance() {
        let function = |return_type, positional: Vec<ResolvedType>| ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            alias: FunctionTypeAlias::default(),
            optional_positional: 0,
            return_type: Arc::new(return_type),
            positional: positional.into(),
            named: empty_named_types(),
            nullable: false,
        };
        let partial = function(
            ResolvedType::Unknown,
            vec![ResolvedType::Dynamic, ResolvedType::Void],
        );
        let complete = function(
            ResolvedType::Never,
            vec![ResolvedType::Dynamic, ResolvedType::Void],
        );
        let mut current = partial.clone();
        assert!(!refine_inferred_type(
            &mut current,
            function(
                ResolvedType::Never,
                vec![ResolvedType::Never, ResolvedType::Void]
            ),
        ));
        let mut changed_shape = complete.clone();
        if let ResolvedType::Function {
            optional_positional,
            ..
        } = &mut changed_shape
        {
            *optional_positional = 1;
        }
        assert!(!refine_inferred_type(&mut current, changed_shape));
        let mut changed_alias = complete.clone();
        if let ResolvedType::Function { alias, .. } = &mut changed_alias {
            *alias = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
                identity: DeclarationIdentity::OBJECT,
                origin: None,
            })));
        }
        assert!(!refine_inferred_type(&mut current, changed_alias));
        assert_eq!(current, partial);
        assert!(refine_inferred_type(&mut current, complete.clone()));
        assert_eq!(current, complete);
    }

    #[test]
    fn function_parameters_refine_nested_holes_without_changing_known_returns() {
        let parameter = |ty| ResolvedType::Record {
            occurrence: FunctionTypeAlias::default(),
            positional: Arc::from([ty]),
            named: empty_named_types(),
            nullable: false,
        };
        let mut current = ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            alias: FunctionTypeAlias::default(),
            optional_positional: 0,
            return_type: Arc::new(ResolvedType::Void),
            positional: Arc::from([parameter(ResolvedType::Unknown)]),
            named: Arc::from([("named".to_owned(), ResolvedType::Unknown)]),
            nullable: false,
        };
        let mut candidate = current.clone();
        if let ResolvedType::Function {
            positional, named, ..
        } = &mut candidate
        {
            *positional = Arc::from([parameter(ResolvedType::Dynamic)]);
            *named = Arc::from([("named".to_owned(), ResolvedType::Never)]);
        }
        assert!(refine_inferred_type(&mut current, candidate.clone()));
        assert_eq!(current, candidate);
        assert!(!refine_inferred_type(&mut current, ResolvedType::Dynamic));
    }
}
