use super::*;

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

    pub(super) fn same_origin_memoized(
        left: &Arc<Self>,
        right: &Arc<Self>,
        seen: &mut FxHashSet<(*const Self, *const Self)>,
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
                if !seen.insert((Arc::as_ptr(left), Arc::as_ptr(right))) {
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
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
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
    pub(super) fn eq_memoized(
        &self,
        other: &Self,
        seen: &mut FxHashSet<(*const Self, *const Self)>,
    ) -> bool {
        self.eq_memoized_with_aliases(other, seen, AliasComparison::Ignore)
    }

    pub(super) fn eq_memoized_with_aliases(
        &self,
        other: &Self,
        seen: &mut FxHashSet<(*const Self, *const Self)>,
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
        if !seen.insert((self, other)) {
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

    fn eq_types(
        left: &[Self],
        right: &[Self],
        seen: &mut FxHashSet<(*const Self, *const Self)>,
        compare_aliases: AliasComparison,
    ) -> bool {
        std::ptr::eq(left, right)
            || (left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(a, b)| a.eq_memoized_with_aliases(b, seen, compare_aliases)))
    }

    fn eq_named(
        left: &[(String, Self)],
        right: &[(String, Self)],
        seen: &mut FxHashSet<(*const Self, *const Self)>,
        compare_aliases: AliasComparison,
    ) -> bool {
        std::ptr::eq(left, right)
            || (left.len() == right.len()
                && left.iter().zip(right).all(|((a, at), (b, bt))| {
                    a == b && at.eq_memoized_with_aliases(bt, seen, compare_aliases)
                }))
    }

    pub(super) fn structural_hash(&self, memo: &mut FxHashMap<*const Self, u64>) -> u64 {
        if let Some(hash) = memo.get(&(self as *const Self)) {
            return *hash;
        }
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
                if arguments.is_empty() {
                    return state.finish();
                }
            }
            Self::TypeParameter {
                id,
                bound,
                nullable,
            } => {
                id.hash(&mut state);
                nullable.hash(&mut state);
                bound.structural_hash(memo).hash(&mut state);
            }
            Self::NonNullTypeParameter { id, bound } => {
                id.hash(&mut state);
                bound.structural_hash(memo).hash(&mut state);
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
                    shape.required_named.hash(&mut state);
                    Self::hash_types(&shape.type_parameters, memo, &mut state);
                }
                return_type.structural_hash(memo).hash(&mut state);
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
                if positional.is_empty() && named.is_empty() {
                    return state.finish();
                }
            }
            _ => return state.finish(),
        }
        let hash = state.finish();
        // Cache values, not visitation markers: sharing must not affect the hash.
        memo.insert(self, hash);
        hash
    }

    fn hash_types(types: &[Self], memo: &mut FxHashMap<*const Self, u64>, state: &mut impl Hasher) {
        types.len().hash(state);
        for ty in types {
            ty.structural_hash(memo).hash(state);
        }
    }

    fn hash_named(
        types: &[(String, Self)],
        memo: &mut FxHashMap<*const Self, u64>,
        state: &mut impl Hasher,
    ) {
        types.len().hash(state);
        for (name, ty) in types {
            name.hash(state);
            ty.structural_hash(memo).hash(state);
        }
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
    seen: &mut FxHashSet<*const ResolvedType>,
) -> Option<&'a ResolvedType> {
    find_type_including_bounds(ty, predicate, seen, false)
}

pub(super) fn find_type_including_bounds<'a>(
    ty: &'a ResolvedType,
    predicate: &mut impl FnMut(&ResolvedType) -> bool,
    seen: &mut FxHashSet<*const ResolvedType>,
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
            if !seen.insert(ty) {
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
            if !seen.insert(ty) {
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum AliasPresenceKey {
    Function(
        *const ResolvedType,
        *const ResolvedType,
        usize,
        *const (String, ResolvedType),
        usize,
    ),
    Interface(*const ResolvedType, usize),
    Record(
        *const ResolvedType,
        usize,
        *const (String, ResolvedType),
        usize,
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
                Arc::as_ptr(return_type),
                positional.as_ptr(),
                positional.len(),
                named.as_ptr(),
                named.len(),
            )),
            Self::Interface { arguments, .. } => Some(AliasPresenceKey::Interface(
                arguments.as_ptr(),
                arguments.len(),
            )),
            Self::Record {
                positional, named, ..
            } => Some(AliasPresenceKey::Record(
                positional.as_ptr(),
                positional.len(),
                named.as_ptr(),
                named.len(),
            )),
            _ => None,
        }
    }

    pub(super) fn has_alias_provenance(&self) -> bool {
        self.has_alias_provenance_memoized(&mut FxHashMap::default())
    }

    fn has_alias_provenance_memoized(&self, memo: &mut FxHashMap<AliasPresenceKey, bool>) -> bool {
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
                return_type.has_alias_provenance_memoized(memo)
                    || positional
                        .iter()
                        .any(|ty| ty.has_alias_provenance_memoized(memo))
                    || named
                        .iter()
                        .any(|(_, ty)| ty.has_alias_provenance_memoized(memo))
            }
            Self::Interface { arguments, .. } => arguments
                .iter()
                .any(|ty| ty.has_alias_provenance_memoized(memo)),
            Self::Record {
                positional, named, ..
            } => {
                positional
                    .iter()
                    .any(|ty| ty.has_alias_provenance_memoized(memo))
                    || named
                        .iter()
                        .any(|(_, ty)| ty.has_alias_provenance_memoized(memo))
            }
            _ => false,
        };
        memo.insert(key, present);
        present
    }

    /// Select a descendant lazily without copying its shared type graph.
    pub(super) fn projected_from(self, owner: &FunctionTypeAlias) -> Self {
        self.projected_from_memoized(owner, &mut FxHashMap::default())
    }

    pub(super) fn projected_from_memoized(
        mut self,
        owner: &FunctionTypeAlias,
        memo: &mut FxHashMap<AliasPresenceKey, bool>,
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
        ) && !self.has_alias_provenance_memoized(memo)
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
