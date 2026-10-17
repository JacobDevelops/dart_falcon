//! Canonical semantic facts shared by resolver-dependent rules.
//!
//! This is deliberately conservative: every unresolved or ambiguous identity
//! becomes `Unknown`, and consumers diagnose only from proven facts.

use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::{Borrow, Cow};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::{Deref, DerefMut, Range};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use falcon_syntax::ast::{
    BinaryOp, ClassMember, ClassRestriction, DartType, EffectiveMembers, Expr, FormalParam,
    FormalParamList, FunctionBody, FunctionKind, InitializingFormal, ParamKind, Program, Span,
    TopLevelDecl, TypeParam, UnaryOp, VarKeyword,
};
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

use super::identity::TypeAlias;
use super::{
    DeclarationIdentity, FileId, IdentityIndex, LibraryId, MemberKind, NameIdentity, QualifiedName,
    Truth, TypeIndex, TypeKind,
};
use crate::environment::ExternalLibraries;

mod default_syntax;
pub use default_syntax::DefaultSyntaxKey;

mod inherited;
mod queries;
pub use queries::ResolvedInvocation;
mod unrelated;

#[cfg(test)]
mod alias_tests;

type AliasParameter = (usize, TypeParameterId, ResolvedType);
type AliasTemplate = (Vec<AliasParameter>, ResolvedType);

fn empty_type_arguments() -> Arc<[ResolvedType]> {
    static EMPTY: std::sync::LazyLock<Arc<[ResolvedType]>> =
        std::sync::LazyLock::new(|| Arc::from([]));
    Arc::clone(&EMPTY)
}

fn empty_named_types() -> Arc<[(String, ResolvedType)]> {
    static EMPTY: std::sync::LazyLock<Arc<[(String, ResolvedType)]>> =
        std::sync::LazyLock::new(|| Arc::from([]));
    Arc::clone(&EMPTY)
}

/// In-progress entries detect cycles by alias identity, without a depth cap.
/// Clones of an identity index do not retain its derived templates.
#[derive(Debug, Default)]
pub(crate) struct AliasTemplates(
    std::sync::Mutex<FxHashMap<(std::thread::ThreadId, bool), AliasTemplateState>>,
);

impl Clone for AliasTemplates {
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[derive(Debug)]
enum AliasTemplateState {
    Resolving,
    Invalid,
    Ready(Arc<AliasTemplate>),
}

thread_local! {
    /// A cycle invalidates every alias currently expanding on this thread.
    /// Ordinary unresolved source types do not change this generation.
    static ALIAS_CYCLE_GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

fn mark_alias_cycle() {
    ALIAS_CYCLE_GENERATION.with(|generation| generation.set(generation.get().wrapping_add(1)));
}
type IndexedProgram<'a> = (Option<FileId>, &'a Program, &'a [(usize, usize)]);

/// Whether a selector/call chain starts with an implicit dot-shorthand receiver.
/// Argument expressions do not establish the receiver's inference context.
pub fn expression_has_dot_shorthand_base(mut expression: &Expr) -> bool {
    loop {
        expression = match expression.unparenthesized() {
            Expr::DotShorthand { .. } => return true,
            Expr::Field { object, .. } => object,
            Expr::Call { callee, .. } => callee,
            Expr::GenericInstantiation { target, .. } => target,
            _ => return false,
        };
    }
}

/// Whether a null-aware selector (`?.`, `?[`) in `expression`'s own
/// selector chain null-shorts it, which makes its static type nullable
/// (`a?.b.c`, `a?.b!`, `a?[0].c()`). Explicit parentheses end a chain.
pub fn null_shorted(expression: &Expr) -> bool {
    let mut current = expression;
    loop {
        current = match current {
            Expr::Field {
                object,
                is_null_safe,
                ..
            }
            | Expr::Index {
                object,
                is_null_safe,
                ..
            } => {
                if *is_null_safe {
                    return true;
                }
                object
            }
            Expr::NullAssert { operand, .. } => operand,
            Expr::Call { callee, .. } => callee,
            Expr::GenericInstantiation { target, .. } => target,
            _ => return false,
        };
    }
}

type ArgumentTypeFallback<'a> = dyn Fn(&Expr) -> Option<ResolvedType> + 'a;

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
pub struct FunctionTypeAlias(Option<Arc<AliasProvenance>>);

#[derive(Debug, PartialEq, Eq)]
enum AliasProvenance {
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

    fn same_origin_memoized(
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

    fn same_origin(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (Some(left), Some(right)) => AliasProvenance::same_origin(left, right),
            (None, None) => true,
            _ => false,
        }
    }

    /// Substitution may intern structurally equal descendants from separate
    /// passes over one cached source use. This is deliberately weaker than
    /// `same_origin`: inference joins must retain distinct instantiated uses.
    fn same_substitution_source(&self, other: &Self) -> bool {
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

    fn projected_from(&self, owner: &Self) -> Self {
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

    fn substituted(&self) -> Self {
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
    fn substitute<'a>(
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

    fn from_scope(
        required_named: Vec<String>,
        parameters: &[TypeParam],
        scope: &TypeParameterScope,
    ) -> Option<Arc<Self>> {
        let parameters = parameters
            .iter()
            .map(|parameter| scope.lookup(&parameter.name.name))
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
enum AliasComparison {
    Ignore,
    Substitution,
    Origin,
}

/// Substitution interns equal source subgraphs within one pass. Unlike semantic
/// equality, its keys retain function-use provenance for diagnostics.
#[derive(Clone, Copy)]
struct SubstitutionKey<'a>(&'a ResolvedType);

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
enum SubstitutionSourceKey<'a> {
    Interface(
        &'a DeclarationIdentity,
        *const ResolvedType,
        usize,
        bool,
        bool,
        Option<*const AliasProvenance>,
    ),
    Function(
        *const ResolvedType,
        *const ResolvedType,
        usize,
        *const (String, ResolvedType),
        usize,
        usize,
        bool,
        Option<*const AliasProvenance>,
        Option<*const FunctionTypeShape>,
    ),
    Record(
        *const ResolvedType,
        usize,
        *const (String, ResolvedType),
        usize,
        bool,
        Option<*const AliasProvenance>,
    ),
}

impl<'a> SubstitutionSourceKey<'a> {
    fn of(ty: &'a ResolvedType) -> Option<Self> {
        match ty {
            ResolvedType::Interface {
                occurrence,
                identity,
                arguments,
                nullable,
                extension_type,
            } if !arguments.is_empty() => Some(Self::Interface(
                identity,
                arguments.as_ptr(),
                arguments.len(),
                *nullable,
                *extension_type,
                occurrence.0.as_ref().map(Arc::as_ptr),
            )),
            ResolvedType::Function {
                return_type,
                positional,
                named,
                optional_positional,
                nullable,
                alias,
                shape,
            } => Some(Self::Function(
                Arc::as_ptr(return_type),
                positional.as_ptr(),
                positional.len(),
                named.as_ptr(),
                named.len(),
                *optional_positional,
                *nullable,
                alias.0.as_ref().map(Arc::as_ptr),
                shape.as_ref().map(Arc::as_ptr),
            )),
            ResolvedType::Record {
                occurrence,
                positional,
                named,
                nullable,
            } => Some(Self::Record(
                positional.as_ptr(),
                positional.len(),
                named.as_ptr(),
                named.len(),
                *nullable,
                occurrence.0.as_ref().map(Arc::as_ptr),
            )),
            _ => None,
        }
    }
}

/// All memo tables are scoped to one fixed substitution map. Source references
/// keep their Arc storage alive throughout the pass; replacement types are never
/// traversed, so their parameters cannot leak into this pass's relevance cache.
#[derive(Default)]
struct SubstitutionMemo<'a> {
    relevant: FxHashMap<SubstitutionSourceKey<'a>, bool>,
    sources: FxHashMap<SubstitutionSourceKey<'a>, ResolvedType>,
    hashes: FxHashMap<*const ResolvedType, u64>,
    structural: FxHashMap<u64, Vec<(SubstitutionKey<'a>, ResolvedType)>>,
}

impl<'a> SubstitutionMemo<'a> {
    fn is_relevant(
        &mut self,
        ty: &'a ResolvedType,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
        cache_result: bool,
    ) -> bool {
        let key = SubstitutionSourceKey::of(ty);
        if let Some(relevant) = key.and_then(|key| self.relevant.get(&key)) {
            return *relevant;
        }
        // Match substitution's free-parameter boundary, not mentions_any's
        // bound traversal: an unreplaced parameter retains its original bound.
        let relevant = match ty {
            ResolvedType::TypeParameter { id, .. }
            | ResolvedType::NonNullTypeParameter { id, .. } => substitutions.contains_key(id),
            ResolvedType::Interface { arguments, .. } => arguments
                .iter()
                .any(|ty| self.is_relevant(ty, substitutions, true)),
            ResolvedType::Function {
                return_type,
                positional,
                named,
                shape,
                ..
            } => {
                self.is_relevant(return_type, substitutions, true)
                    || positional
                        .iter()
                        .any(|ty| self.is_relevant(ty, substitutions, true))
                    || named
                        .iter()
                        .any(|(_, ty)| self.is_relevant(ty, substitutions, true))
                    || shape.as_ref().is_some_and(|shape| shape.type_parameters.iter().any(|ty| {
                        matches!(ty, ResolvedType::TypeParameter { id, bound, .. }
                            if substitutions.contains_key(id) || self.is_relevant(bound, substitutions, true))
                    }))
            }
            ResolvedType::Record {
                positional, named, ..
            } => {
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

impl ResolvedType {
    fn eq_memoized(&self, other: &Self, seen: &mut FxHashSet<(*const Self, *const Self)>) -> bool {
        self.eq_memoized_with_aliases(other, seen, AliasComparison::Ignore)
    }

    fn eq_memoized_with_aliases(
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

    fn structural_hash(&self, memo: &mut FxHashMap<*const Self, u64>) -> u64 {
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

#[cfg(test)]
mod type_graph_tests {
    use super::*;

    fn fanout(depth: usize, leaf: ResolvedType) -> ResolvedType {
        let mut ty = leaf;
        for _ in 0..depth {
            ty = ResolvedType::Record {
                occurrence: Default::default(),
                positional: vec![ty.clone(), ty.clone(), ty].into(),
                named: Arc::from([]),
                nullable: false,
            };
        }
        ty
    }

    fn unshared(depth: usize) -> ResolvedType {
        if depth == 0 {
            return ResolvedType::Dynamic;
        }
        ResolvedType::Record {
            occurrence: Default::default(),
            positional: (0..3).map(|_| unshared(depth - 1)).collect(),
            named: Arc::from([]),
            nullable: false,
        }
    }

    fn hash(ty: &ResolvedType) -> u64 {
        let mut state = std::collections::hash_map::DefaultHasher::new();
        ty.hash(&mut state);
        state.finish()
    }

    #[test]
    fn flat_interfaces_and_shared_children_do_not_allocate_memos() {
        let left = ResolvedType::interface_identity(DeclarationIdentity::OBJECT);
        let right = ResolvedType::interface_identity(DeclarationIdentity::OBJECT);
        let mut pairs = FxHashSet::default();
        assert!(left.eq_memoized(&right, &mut pairs));
        assert_eq!(pairs.capacity(), 0);
        assert!(!left.eq_memoized(&right.with_nullable(true), &mut pairs));
        assert_eq!(pairs.capacity(), 0);
        let mut hashes = FxHashMap::default();
        left.structural_hash(&mut hashes);
        assert_eq!(hashes.capacity(), 0);
        let shared = fanout(17, ResolvedType::Dynamic);
        assert!(shared.eq_memoized(&shared.clone(), &mut pairs));
        assert_eq!(pairs.capacity(), 0);
    }

    #[test]
    fn independent_deep_graphs_compare_and_hash_once_per_node() {
        let left = fanout(17, ResolvedType::Dynamic);
        let right = fanout(17, ResolvedType::Dynamic);
        let mut pairs = FxHashSet::default();
        assert!(left.eq_memoized(&right, &mut pairs));
        assert!(pairs.len() <= 3 * 17);
        let mut hashes = FxHashMap::default();
        left.structural_hash(&mut hashes);
        assert!(hashes.len() <= 3 * 17);
        assert!(left == right);
        assert_eq!(hash(&left), hash(&right));
    }

    #[test]
    fn structural_equality_and_hash_ignore_sharing_topology() {
        let shared = fanout(4, ResolvedType::Dynamic);
        let expanded = unshared(4);
        assert!(shared == expanded);
        assert_eq!(hash(&shared), hash(&expanded));
        let mut pairs = FxHashSet::default();
        assert!(shared.eq_memoized(&shared, &mut pairs));
        assert!(pairs.is_empty());
    }

    #[test]
    fn sdk_free_unrelatedness_preserves_deep_shared_types() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let signatures = SignatureIndex::default();
        for depth in [17, 80] {
            let left = fanout(depth, ResolvedType::Dynamic);
            let right = fanout(depth, ResolvedType::Dynamic);
            assert!(signatures.extension_type_erasure(&left, &model).is_some());
            assert_eq!(
                signatures.types_are_unrelated(&left, &right, &model),
                Truth::No
            );
            let related = fanout(depth, ResolvedType::Void);
            assert_eq!(
                signatures.types_are_unrelated(&left, &related, &model),
                Truth::No
            );
            assert_eq!(signatures.subtype(&left, &related, &model), Truth::Yes);
        }
        let first = model.core_type("int", false);
        let second = model.core_type("int", false);
        let (
            ResolvedType::Interface {
                arguments: first, ..
            },
            ResolvedType::Interface {
                arguments: second, ..
            },
        ) = (&first, &second)
        else {
            panic!("core types must be interfaces")
        };
        assert!(Arc::ptr_eq(first, second));
    }

    #[test]
    fn owner_parameter_mentions_search_shared_nodes_and_bounds() {
        let id = TypeParameterId {
            owner: ParameterOwner::Source {
                file: None,
                offset: 0,
            },
            ordinal: 0,
            name: "T".into(),
        };
        let absent = TypeParameterId {
            ordinal: 1,
            ..id.clone()
        };
        let parameter = ResolvedType::TypeParameter {
            id: id.clone(),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let graph = fanout(17, parameter);
        assert!(mentions_any(&graph, std::slice::from_ref(&id)));
        assert!(!mentions_any(&graph, std::slice::from_ref(&absent)));
        let bounded = ResolvedType::TypeParameter {
            id: absent,
            bound: Arc::new(graph),
            nullable: false,
        };
        assert!(mentions_any(&bounded, &[id]));
    }

    #[test]
    fn substitution_interns_independently_allocated_equal_siblings() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let id = TypeParameterId {
            owner: ParameterOwner::Sdk("structural-substitution"),
            ordinal: 0,
            name: "T".into(),
        };
        let parameter = ResolvedType::TypeParameter {
            id: id.clone(),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let source = ResolvedType::Record {
            occurrence: Default::default(),
            positional: (0..3).map(|_| fanout(17, parameter.clone())).collect(),
            named: empty_named_types(),
            nullable: false,
        };
        let substitutions = HashMap::from([(id, ResolvedType::Void)]);
        let result = model.substitute(&source, &substitutions);
        let ResolvedType::Record { positional, .. } = result else {
            unreachable!()
        };
        let children: Vec<_> = positional
            .iter()
            .map(|child| {
                let ResolvedType::Record { positional, .. } = child else {
                    unreachable!()
                };
                positional
            })
            .collect();
        assert!(Arc::ptr_eq(children[0], children[1]));
        assert!(Arc::ptr_eq(children[1], children[2]));
        assert!(positional[0] == fanout(17, ResolvedType::Void));
    }

    #[test]
    fn substitution_memos_shared_sources_and_skips_irrelevant_subgraphs() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let id = TypeParameterId {
            owner: ParameterOwner::Sdk("substitution-source-memo"),
            ordinal: 0,
            name: "T".into(),
        };
        let substitutions = HashMap::from([(id.clone(), ResolvedType::Void)]);
        let parameter = ResolvedType::TypeParameter {
            id,
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let source = fanout(24, parameter);
        let mut memo = SubstitutionMemo::default();
        let result = model.substitute_memoized(&source, &substitutions, &mut memo);
        assert_eq!(memo.sources.len(), 24);
        assert_eq!(memo.relevant.len(), 23);
        assert!(memo.hashes.len() <= 3 * 24 + 1);
        assert!(result == fanout(24, ResolvedType::Void));

        let source = fanout(24, ResolvedType::Dynamic);
        let mut memo = SubstitutionMemo::default();
        let result = model.substitute_memoized(&source, &substitutions, &mut memo);
        assert_eq!(memo.relevant.len(), 23);
        assert!(memo.sources.is_empty());
        assert!(memo.hashes.is_empty());
        assert!(memo.structural.is_empty());
        let (
            ResolvedType::Record {
                positional: original,
                ..
            },
            ResolvedType::Record {
                positional: unchanged,
                ..
            },
        ) = (&source, &result)
        else {
            unreachable!()
        };
        assert!(Arc::ptr_eq(original, unchanged));
    }

    #[test]
    fn flat_irrelevant_substitution_roots_do_not_allocate_memos() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let substitutions = HashMap::from([(
            TypeParameterId {
                owner: ParameterOwner::Sdk("irrelevant-root"),
                ordinal: 0,
                name: "T".into(),
            },
            ResolvedType::Void,
        )]);
        let integer = model.core_type("int", false);
        let sources = [
            ResolvedType::Function {
                shape: Some(FunctionTypeShape::empty()),
                alias: FunctionTypeAlias::default(),
                return_type: Arc::new(integer.clone()),
                positional: vec![integer.clone()].into(),
                named: empty_named_types(),
                optional_positional: 0,
                nullable: false,
            },
            ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::sdk("dart:core", "List"),
                arguments: vec![integer].into(),
                nullable: false,
                extension_type: false,
            },
        ];
        for source in &sources {
            let mut memo = SubstitutionMemo::default();
            let result = model.substitute_memoized(source, &substitutions, &mut memo);
            assert!(SubstitutionSourceKey::of(source) == SubstitutionSourceKey::of(&result));
            assert_eq!(memo.relevant.capacity(), 0);
            assert_eq!(memo.sources.capacity(), 0);
            assert_eq!(memo.hashes.capacity(), 0);
            assert_eq!(memo.structural.capacity(), 0);
        }
    }

    #[test]
    fn substitution_memoizes_shared_irrelevant_children_after_relevant_siblings() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let id = TypeParameterId {
            owner: ParameterOwner::Sdk("mixed-substitution-fanout"),
            ordinal: 0,
            name: "T".into(),
        };
        let substitutions = HashMap::from([(id.clone(), ResolvedType::Void)]);
        let record = |positional: Vec<ResolvedType>| ResolvedType::Record {
            occurrence: Default::default(),
            positional: positional.into(),
            named: empty_named_types(),
            nullable: false,
        };
        let irrelevant = record(vec![model.core_type("int", false); 256]);
        let mut source = ResolvedType::TypeParameter {
            id,
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        for _ in 0..24 {
            source = record(vec![source, irrelevant.clone()]);
        }
        let mut memo = SubstitutionMemo::default();
        let result = model.substitute_memoized(&source, &substitutions, &mut memo);
        let irrelevant_key = SubstitutionSourceKey::of(&irrelevant).unwrap();
        assert_eq!(memo.relevant.get(&irrelevant_key), Some(&false));
        assert_eq!(memo.relevant.len(), 24);
        assert_eq!(memo.sources.len(), 24);
        let mut result = &result;
        for _ in 0..24 {
            let ResolvedType::Record { positional, .. } = result else {
                panic!("lost mixed fanout record")
            };
            assert!(SubstitutionSourceKey::of(&positional[1]) == Some(irrelevant_key));
            result = &positional[0];
        }
        assert_eq!(result, &ResolvedType::Void);
    }

    #[test]
    fn substitution_keys_preserve_distinct_function_origins() {
        let function = |offset| ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            optional_positional: 0,
            alias: FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
                identity: DeclarationIdentity::OBJECT,
                origin: Some((None, offset)),
            }))),
            return_type: Arc::new(ResolvedType::Dynamic),
            positional: empty_type_arguments(),
            named: empty_named_types(),
            nullable: false,
        };
        let left = fanout(17, function(1));
        let right = fanout(17, function(2));
        assert!(left == right);
        assert!(SubstitutionKey(&left) != SubstitutionKey(&right));
        assert!(SubstitutionKey(&left) == SubstitutionKey(&left.clone()));
    }

    #[test]
    fn substitution_source_keys_preserve_scalar_context_and_alias_origins() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let id = TypeParameterId {
            owner: ParameterOwner::Sdk("substitution-source-context"),
            ordinal: 0,
            name: "T".into(),
        };
        let parameter = ResolvedType::TypeParameter {
            id: id.clone(),
            bound: Arc::new(fanout(24, ResolvedType::Dynamic)),
            nullable: false,
        };
        let positional: Arc<[ResolvedType]> = vec![parameter.clone()].into();
        let named = empty_named_types();
        let interface = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: DeclarationIdentity::sdk("dart:core", "List"),
            arguments: Arc::clone(&positional),
            nullable: false,
            extension_type: false,
        };
        let mut other_identity = interface.clone();
        let ResolvedType::Interface { identity, .. } = &mut other_identity else {
            unreachable!()
        };
        *identity = DeclarationIdentity::sdk("dart:core", "Set");
        let mut extension = interface.clone();
        let ResolvedType::Interface { extension_type, .. } = &mut extension else {
            unreachable!()
        };
        *extension_type = true;
        let record = ResolvedType::Record {
            occurrence: Default::default(),
            positional: Arc::clone(&positional),
            named: Arc::clone(&named),
            nullable: false,
        };
        let provenance = |offset| {
            FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
                identity: DeclarationIdentity::OBJECT,
                origin: Some((None, offset)),
            })))
        };
        let function = ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            alias: provenance(1),
            optional_positional: 0,
            return_type: Arc::new(parameter.clone()),
            positional,
            named,
            nullable: false,
        };
        let mut optional = function.clone();
        let ResolvedType::Function {
            optional_positional,
            ..
        } = &mut optional
        else {
            unreachable!()
        };
        *optional_positional = 1;
        let mut unaliased = function.clone();
        let ResolvedType::Function { alias, .. } = &mut unaliased else {
            unreachable!()
        };
        *alias = FunctionTypeAlias::default();
        let mut other_origin = function.clone();
        let ResolvedType::Function { alias, .. } = &mut other_origin else {
            unreachable!()
        };
        *alias = provenance(2);
        let mut contextual_interface = interface.clone();
        let ResolvedType::Interface { occurrence, .. } = &mut contextual_interface else {
            unreachable!()
        };
        *occurrence = provenance(3);
        let mut contextual_record = record.clone();
        let ResolvedType::Record { occurrence, .. } = &mut contextual_record else {
            unreachable!()
        };
        *occurrence = provenance(3);
        let mut contextual_function = unaliased.clone();
        let ResolvedType::Function { alias, .. } = &mut contextual_function else {
            unreachable!()
        };
        *alias = alias.projected_from(&provenance(3));
        let cases = vec![
            interface.clone(),
            interface.with_nullable(true),
            other_identity,
            extension,
            contextual_interface,
            record.clone(),
            record.with_nullable(true),
            contextual_record,
            function.clone(),
            function.with_nullable(true),
            optional,
            unaliased,
            other_origin,
            contextual_function,
        ];
        let keys: FxHashSet<_> = cases.iter().filter_map(SubstitutionSourceKey::of).collect();
        assert_eq!(keys.len(), cases.len());
        let substitutions = HashMap::from([(id, ResolvedType::Void)]);
        let expected: Vec<_> = cases
            .iter()
            .map(|ty| model.substitute(ty, &substitutions))
            .collect();
        let source = ResolvedType::Record {
            occurrence: Default::default(),
            positional: cases.into(),
            named: empty_named_types(),
            nullable: false,
        };
        let ResolvedType::Record { positional, .. } = model.substitute(&source, &substitutions)
        else {
            unreachable!()
        };
        for (actual, expected) in positional.iter().zip(&expected) {
            assert!(SubstitutionKey(actual) == SubstitutionKey(expected));
        }
        let mut memo = SubstitutionMemo::default();
        assert!(matches!(
            model.substitute_memoized(&parameter, &substitutions, &mut memo),
            ResolvedType::Void
        ));
        assert!(memo.hashes.is_empty());
        assert!(memo.structural.is_empty());
    }

    #[test]
    fn substituted_alias_origins_only_share_the_same_source_use() {
        let source = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, 1)),
        })));
        let other = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, 2)),
        })));
        let first = source.substituted();
        let second = source.substituted();
        assert!(!first.same_origin(&second));
        assert!(first.same_origin(&first.clone()));
        assert!(first.same_substitution_source(&second));
        assert!(!first.same_substitution_source(&source));
        assert!(!first.same_substitution_source(&other.substituted()));
        assert!(!source.same_substitution_source(&other));
        assert!(!first.same_origin(&source));
        assert!(!first.same_origin(&other.substituted()));
        assert!(!source.same_origin(&other));
    }

    #[test]
    fn projected_alias_origins_compare_shared_provenance_dags_linearly() {
        let source = || {
            Arc::new(AliasProvenance::Source {
                identity: DeclarationIdentity::OBJECT,
                origin: Some((None, 1)),
            })
        };
        let mut left = source();
        let mut right = source();
        for _ in 0..24 {
            left = Arc::new(AliasProvenance::Projected {
                source: Arc::clone(&left),
                owner: left,
            });
            right = Arc::new(AliasProvenance::Projected {
                source: Arc::clone(&right),
                owner: right,
            });
        }
        let mut seen = FxHashSet::default();
        assert!(AliasProvenance::same_origin_memoized(
            &left, &right, &mut seen
        ));
        assert_eq!(seen.len(), 24);
    }

    #[test]
    fn inference_join_memoizes_contextual_record_fanout() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let signatures = SignatureIndex::default();
        let provenance = |offset| {
            FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
                identity: DeclarationIdentity::OBJECT,
                origin: Some((None, offset)),
            })))
        };
        let mut graph = ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            alias: provenance(1),
            return_type: Arc::new(ResolvedType::Void),
            positional: empty_type_arguments(),
            named: empty_named_types(),
            optional_positional: 0,
            nullable: false,
        };
        for _ in 0..24 {
            graph = ResolvedType::Record {
                occurrence: Default::default(),
                positional: vec![graph.clone(), graph.clone(), graph].into(),
                named: empty_named_types(),
                nullable: false,
            };
        }
        let left = graph.clone().projected_from(&provenance(2));
        let right = graph.projected_from(&provenance(3));
        let mut memo = InferenceJoinMemo::default();
        let mut joined = inference_join_memoized(&left, &right, &signatures, &model, &mut memo);
        assert_eq!(memo.shapes.values().map(Vec::len).sum::<usize>(), 25);
        // The root was projected before this pass; only its 23 nested records
        // need provenance-presence queries during recursive joins.
        assert_eq!(memo.provenance.len(), 23);
        assert!(memo.hashes.len() <= 24 * 3 + 1);
        for _ in 0..24 {
            let ResolvedType::Record { positional, .. } = &joined else {
                panic!("lost record shape")
            };
            let keys: Vec<_> = positional.iter().map(SubstitutionSourceKey::of).collect();
            assert!(keys[0] == keys[1]);
            assert!(keys[0] == keys[2]);
            joined = joined.record_positional(0).unwrap();
        }
        let ResolvedType::Function { alias, .. } = joined else {
            panic!("lost function descendant")
        };
        assert_eq!(alias.identity(), None);
    }

    #[test]
    fn inference_join_memoizes_interface_fanout() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let signatures = SignatureIndex::default();
        let fanout = |mut ty: ResolvedType| {
            for _ in 0..24 {
                ty = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::OBJECT,
                    arguments: vec![ty.clone(), ty.clone(), ty].into(),
                    nullable: false,
                    extension_type: false,
                };
            }
            ty
        };
        let left = fanout(ResolvedType::Void);
        let right = fanout(ResolvedType::Dynamic);
        let mut memo = InferenceJoinMemo::default();
        let mut joined = inference_join_memoized(&left, &right, &signatures, &model, &mut memo);
        assert_eq!(memo.shapes.values().map(Vec::len).sum::<usize>(), 24);
        assert!(memo.provenance.is_empty());
        for _ in 0..24 {
            let ResolvedType::Interface { arguments, .. } = &joined else {
                panic!("lost interface shape")
            };
            let keys: Vec<_> = arguments.iter().map(SubstitutionSourceKey::of).collect();
            assert!(keys[0] == keys[1]);
            assert!(keys[0] == keys[2]);
            joined = joined.type_argument(0).unwrap();
        }
        assert_eq!(joined, ResolvedType::Void);
    }

    #[test]
    fn ordinary_composite_projections_do_not_add_occurrence_context() {
        let owner = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, 1)),
        })));
        let function = ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            alias: Default::default(),
            return_type: Arc::new(ResolvedType::Void),
            positional: empty_type_arguments(),
            named: empty_named_types(),
            optional_positional: 0,
            nullable: false,
        };
        for ty in [
            function.clone(),
            ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::OBJECT,
                arguments: vec![function.clone()].into(),
                nullable: false,
                extension_type: false,
            },
            ResolvedType::Record {
                occurrence: Default::default(),
                positional: vec![function].into(),
                named: empty_named_types(),
                nullable: false,
            },
        ] {
            let projected = ty.clone().projected_from(&owner);
            assert!(SubstitutionSourceKey::of(&ty) == SubstitutionSourceKey::of(&projected));
        }
    }

    #[test]
    fn separate_member_substitution_joins_drop_function_alias_annotations() {
        let identities = IdentityIndex::default();
        let model = SemanticModel::new(Path::new("graph.dart"), &identities, None);
        let signatures = SignatureIndex::default();
        let id = TypeParameterId {
            owner: ParameterOwner::Sdk("member-substitution-join"),
            ordinal: 0,
            name: "T".into(),
        };
        let source = ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            alias: FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
                identity: DeclarationIdentity::OBJECT,
                origin: Some((None, 1)),
            }))),
            optional_positional: 0,
            return_type: Arc::new(ResolvedType::TypeParameter {
                id: id.clone(),
                bound: Arc::new(ResolvedType::Dynamic),
                nullable: false,
            }),
            positional: empty_type_arguments(),
            named: empty_named_types(),
            nullable: false,
        };
        let substitutions = HashMap::from([(id, ResolvedType::Void)]);
        // Independent member reads substitute the same cached signature, but
        // their instantiated alias annotations are not one shared type use.
        let first = model.substitute(&source, &substitutions);
        let second = model.substitute(&source, &substitutions);
        assert!(SubstitutionKey(&first) == SubstitutionKey(&second));
        for nullable in [false, true] {
            let joined =
                inference_join(&first, &second.with_nullable(nullable), &signatures, &model);
            let ResolvedType::Function {
                alias,
                nullable: actual_nullable,
                ..
            } = joined
            else {
                panic!("lost joined function shape")
            };
            assert!(alias.identity().is_none());
            assert_eq!(actual_nullable, nullable);
        }
        let shared = inference_join(&first, &first.clone(), &signatures, &model);
        let ResolvedType::Function { alias, .. } = shared else {
            panic!("lost shared function alias")
        };
        assert!(alias.identity().is_some());
    }

    #[test]
    fn graph_equality_preserves_leaf_nullability_and_named_fields() {
        let ty = fanout(17, ResolvedType::Dynamic);
        assert!(ty != fanout(17, ResolvedType::Void));
        assert!(ty != ty.with_nullable(true));
        let named = |name: &str| ResolvedType::Record {
            occurrence: Default::default(),
            positional: Arc::from([]),
            named: vec![(name.to_owned(), ty.clone())].into(),
            nullable: false,
        };
        assert!(named("left") != named("right"));
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

fn find_type_including_bounds<'a>(
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

pub(super) fn has_inference_holes(ty: &ResolvedType) -> bool {
    find_type(
        ty,
        &mut |ty| matches!(ty, ResolvedType::Unknown),
        &mut FxHashSet::default(),
    )
    .is_some()
}

fn collect_parameter_ids(ty: &ResolvedType, ids: &mut Vec<TypeParameterId>) {
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

fn parameter_bound<'a>(
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

fn instantiate_parameter_bound(
    id: &TypeParameterId,
    bounds: &HashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    let _ = model;
    instantiate_bound(id, bounds, visiting, true)
}

fn instantiate_bound(
    id: &TypeParameterId,
    bounds: &HashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
) -> ResolvedType {
    if !visiting.insert(id.clone()) {
        return if covariant {
            ResolvedType::Dynamic
        } else {
            ResolvedType::Never
        };
    }
    let result = bounds.get(id).map_or(ResolvedType::Dynamic, |bound| {
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
    bounds: &HashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
) -> ResolvedType {
    instantiate_bound_type_memoized(ty, bounds, visiting, covariant, &mut FxHashMap::default())
}

fn instantiate_bound_type_memoized(
    ty: &ResolvedType,
    bounds: &HashMap<TypeParameterId, ResolvedType>,
    visiting: &mut HashSet<TypeParameterId>,
    covariant: bool,
    memo: &mut FxHashMap<(*const ResolvedType, bool), ResolvedType>,
) -> ResolvedType {
    let key = (ty as *const ResolvedType, covariant);
    if let Some(result) = memo.get(&key) {
        return result.clone();
    }
    let result = match ty {
        ResolvedType::TypeParameter { id, nullable, .. } if bounds.contains_key(id) => {
            let result = if bound_reaches_visiting(id, bounds, visiting, &mut HashSet::new()) {
                if covariant {
                    ResolvedType::Dynamic
                } else {
                    ResolvedType::Never
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
        ResolvedType::NonNullTypeParameter { id, .. } if bounds.contains_key(id) => {
            let result = if bound_reaches_visiting(id, bounds, visiting, &mut HashSet::new()) {
                if covariant {
                    ResolvedType::Dynamic
                } else {
                    ResolvedType::Never
                }
            } else {
                instantiate_bound(id, bounds, visiting, true)
            };
            result.non_null()
        }
        _ => {
            let mut result = ty.clone();
            // Read original nodes rather than copy-on-write destinations: memo
            // keys must remain valid for this entire traversal.
            match (ty, &mut result) {
                (
                    ResolvedType::Interface {
                        arguments: source, ..
                    },
                    ResolvedType::Interface { arguments, .. },
                ) => {
                    *arguments = source
                        .iter()
                        .map(|argument| {
                            instantiate_bound_type_memoized(
                                argument, bounds, visiting, covariant, memo,
                            )
                        })
                        .collect();
                }
                (
                    ResolvedType::Function {
                        return_type: source_return,
                        positional: source_positional,
                        named: source_named,
                        shape: source_shape,
                        ..
                    },
                    ResolvedType::Function {
                        return_type,
                        positional,
                        named,
                        shape,
                        ..
                    },
                ) => {
                    *shape = source_shape.as_ref().map(|shape| {
                        shape.map_bounds(|bound| {
                            instantiate_bound_type_memoized(
                                bound, bounds, visiting, covariant, memo,
                            )
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
                            instantiate_bound_type_memoized(
                                parameter, bounds, visiting, !covariant, memo,
                            )
                        })
                        .collect();
                    *named = source_named
                        .iter()
                        .map(|(name, parameter)| {
                            (
                                name.clone(),
                                instantiate_bound_type_memoized(
                                    parameter, bounds, visiting, !covariant, memo,
                                ),
                            )
                        })
                        .collect();
                }
                (
                    ResolvedType::Record {
                        positional: source_positional,
                        named: source_named,
                        ..
                    },
                    ResolvedType::Record {
                        positional, named, ..
                    },
                ) => {
                    *positional = source_positional
                        .iter()
                        .map(|field| {
                            instantiate_bound_type_memoized(
                                field, bounds, visiting, covariant, memo,
                            )
                        })
                        .collect();
                    *named = source_named
                        .iter()
                        .map(|(name, field)| {
                            (
                                name.clone(),
                                instantiate_bound_type_memoized(
                                    field, bounds, visiting, covariant, memo,
                                ),
                            )
                        })
                        .collect();
                }
                _ => {}
            }
            result
        }
    };
    if matches!(
        ty,
        ResolvedType::Interface { .. }
            | ResolvedType::Function { .. }
            | ResolvedType::Record { .. }
            | ResolvedType::TypeParameter { .. }
            | ResolvedType::NonNullTypeParameter { .. }
    ) {
        memo.insert(key, result.clone());
    }
    result
}

fn bound_reaches_visiting(
    id: &TypeParameterId,
    bounds: &HashMap<TypeParameterId, ResolvedType>,
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

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum AliasPresenceKey {
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

    fn has_alias_provenance(&self) -> bool {
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
    fn projected_from(self, owner: &FunctionTypeAlias) -> Self {
        self.projected_from_memoized(owner, &mut FxHashMap::default())
    }

    fn projected_from_memoized(
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

    /// Whether this type excludes null, including its type parameter bound.
    pub fn is_non_nullable(&self) -> bool {
        match self {
            Self::Dynamic | Self::Unknown | Self::Void | Self::Null => false,
            Self::TypeParameter {
                bound, nullable, ..
            } => !nullable && bound.is_non_nullable(),
            other => !other.nullable(),
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

#[derive(Debug, Clone, Default)]
pub struct TypeParameterScope {
    scopes: Vec<HashMap<Arc<str>, ResolvedType>>,
}

impl TypeParameterScope {
    pub fn push(&mut self, params: &[TypeParam], model: &SemanticModel<'_>) {
        let owner = ParameterOwner::Source {
            file: model.file,
            offset: params.first().map_or(0, |param| param.span.start),
        };
        self.scopes.push(HashMap::new());
        for (ordinal, param) in params.iter().enumerate() {
            let name: Arc<str> = param.name.name.as_str().into();
            self.scopes
                .last_mut()
                .expect("type-parameter scope")
                .insert(
                    name.clone(),
                    ResolvedType::TypeParameter {
                        id: TypeParameterId {
                            owner,
                            ordinal,
                            name,
                        },
                        bound: Arc::new(model.core_type("Object", true)),
                        nullable: false,
                    },
                );
        }
        for param in params {
            let Some(ResolvedType::TypeParameter { id, .. }) = self.lookup(&param.name.name) else {
                unreachable!("registered type parameter");
            };
            let bound = param
                .bound
                .as_ref()
                .map(|bound| model.resolve_type_in(bound, self))
                .unwrap_or_else(|| model.core_type("Object", true));
            self.scopes
                .last_mut()
                .expect("type-parameter scope")
                .insert(
                    id.name.clone(),
                    ResolvedType::TypeParameter {
                        id,
                        bound: Arc::new(bound),
                        nullable: false,
                    },
                );
        }
    }

    /// Adds already-resolved lexical type bindings, such as a constructor's
    /// instantiated owner parameters while resolving its factory redirect.
    pub fn push_bindings<K>(&mut self, bindings: impl IntoIterator<Item = (K, ResolvedType)>)
    where
        K: Into<Arc<str>>,
    {
        self.scopes.push(
            bindings
                .into_iter()
                .map(|(name, ty)| (name.into(), ty))
                .collect(),
        );
    }

    /// Whether `name` names an in-scope type parameter.
    pub fn contains(&self, name: &str) -> bool {
        self.scopes.iter().any(|scope| scope.contains_key(name))
    }

    pub fn pop(&mut self) {
        self.scopes.pop();
    }

    pub fn lookup(&self, name: &str) -> Option<ResolvedType> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }
}

#[derive(Clone, Copy)]
pub struct SemanticModel<'a> {
    file: Option<FileId>,
    library: Option<LibraryId>,
    identities: &'a IdentityIndex,
    types: Option<&'a TypeIndex>,
}

impl<'a> SemanticModel<'a> {
    pub fn new(file: &Path, identities: &'a IdentityIndex, types: Option<&'a TypeIndex>) -> Self {
        Self::from_file(identities.file(file), identities, types)
    }

    /// Resolve in an indexed file's instance rather than its primary path instance.
    pub(crate) fn from_file(
        file: Option<FileId>,
        identities: &'a IdentityIndex,
        types: Option<&'a TypeIndex>,
    ) -> Self {
        Self {
            file,
            library: file.map(|file| identities.library(file)),
            identities,
            types,
        }
    }

    fn owns_declaration(&self, name: &str) -> bool {
        self.library
            .is_none_or(|library| self.identities.is_authoritative_declaration(library, name))
    }

    fn owns_top_level_declaration(&self, declaration: &TopLevelDecl) -> bool {
        let name = match declaration {
            TopLevelDecl::Class(node) => &node.name,
            TopLevelDecl::ClassTypeAlias(node) => &node.name,
            TopLevelDecl::Mixin(node) => &node.name,
            TopLevelDecl::MixinClass(node) => &node.name,
            TopLevelDecl::Enum(node) => &node.name,
            TopLevelDecl::Extension(node) => {
                return node
                    .name
                    .as_ref()
                    .is_none_or(|name| self.owns_declaration(&name.name));
            }
            TopLevelDecl::ExtensionType(node) => &node.name,
            TopLevelDecl::Function(node) => &node.name,
            TopLevelDecl::TypeAlias(node) => &node.name,
            TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => return true,
        };
        self.owns_declaration(&name.name)
    }

    /// The `package:` URI of the library declaring a project `identity`.
    pub fn declaration_uri(&self, identity: &DeclarationIdentity) -> Option<&'a str> {
        let DeclarationIdentity::Project { library, .. } = identity else {
            return None;
        };
        self.identities.library_uri(*library)
    }

    pub(crate) fn for_declaration(
        &self,
        identity: &DeclarationIdentity,
    ) -> Option<SemanticModel<'a>> {
        let library = self.identities.declaration_library(identity)?;
        Some(Self {
            file: self.identities.declaration_file(library, identity.name()),
            library: Some(library),
            identities: self.identities,
            types: self.types,
        })
    }

    /// Views names from a declaring library without inventing a declaration.
    pub fn for_library(&self, library: LibraryId) -> Self {
        Self::in_library(self.identities, self.types, Some(library))
    }

    /// A model viewing members from `library`, for library-wide passes with
    /// no single source file (privacy is library-scoped).
    pub(crate) fn in_library(
        identities: &'a IdentityIndex,
        types: Option<&'a TypeIndex>,
        library: Option<LibraryId>,
    ) -> Self {
        Self {
            file: None,
            library,
            identities,
            types,
        }
    }

    pub fn resolve_type(&self, ty: &DartType) -> ResolvedType {
        self.resolve_type_in(ty, &TypeParameterScope::default())
    }

    /// Resolves ordinary and old-style function formals in their lexical scope.
    pub fn resolve_formal_parameter_in(
        &self,
        param: &FormalParam,
        parameters: &TypeParameterScope,
    ) -> ResolvedType {
        let mut local;
        let parameters = if param.type_params.is_empty() {
            parameters
        } else {
            local = parameters.clone();
            local.push(&param.type_params, self);
            &local
        };
        let return_type = param
            .param_type
            .as_ref()
            .map(|ty| self.resolve_type_in(ty, parameters))
            .unwrap_or(ResolvedType::Dynamic);
        let Some(function_params) = &param.function_params else {
            return return_type;
        };
        let mut named: Vec<_> = function_params
            .named()
            .map(|param| {
                (
                    param.name.name.clone(),
                    self.resolve_formal_parameter_in(param, parameters),
                )
            })
            .collect();
        named.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        ResolvedType::Function {
            shape: FunctionTypeShape::from_scope(
                function_params
                    .named()
                    .filter(|param| matches!(param.kind, ParamKind::Named { required: true }))
                    .map(|param| param.name.name.clone())
                    .collect(),
                &param.type_params,
                parameters,
            ),
            optional_positional: function_params
                .positional()
                .filter(|param| param.kind == ParamKind::OptionalPositional)
                .count(),
            alias: FunctionTypeAlias::default(),
            return_type: Arc::new(return_type),
            positional: function_params
                .positional()
                .map(|param| self.resolve_formal_parameter_in(param, parameters))
                .collect::<Vec<_>>()
                .into(),
            named: named.into(),
            nullable: false,
        }
    }

    fn extension_visible(&self, declaring: Option<LibraryId>, name: Option<&str>) -> bool {
        self.library
            .zip(declaring)
            .is_some_and(|(from, declaring)| {
                self.identities.extension_visible(from, declaring, name)
            })
    }

    /// Whether a member is visible from this model's library.
    pub fn member_visible(&self, declaring_type: &DeclarationIdentity, name: &str) -> bool {
        !name.starts_with('_')
            || self.library.is_some_and(|library| {
                self.identities.declaration_library(declaring_type) == Some(library)
            })
    }

    pub fn resolve_name<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_declaration(self.library?, name.into())
    }

    pub fn name_identity<'n>(&self, name: impl Into<QualifiedName<'n>>) -> NameIdentity {
        self.library.map_or(NameIdentity::Unknown, |library| {
            self.identities.resolve(library, name.into())
        })
    }

    /// [`Self::resolve_name`] for a dotted name; `None` beyond two segments.
    pub fn resolve_dotted<S: AsRef<str>>(&self, segments: &[S]) -> Option<DeclarationIdentity> {
        self.resolve_name(QualifiedName::from_segments(segments)?)
    }

    /// The class a type alias names when `identity` is an alias of an
    /// interface type (`typedef SB = SizedBox;`), else `identity` itself.
    pub fn unaliased(&self, identity: DeclarationIdentity) -> DeclarationIdentity {
        if self.identities.type_alias(&identity).is_none() {
            return identity;
        }
        match self.declared_type(&identity) {
            ResolvedType::Interface { identity, .. } => identity,
            _ => identity,
        }
    }

    /// The type a dotted type name without type arguments denotes (as the
    /// target of `T.named(...)`), with type aliases expanded; `None` when the
    /// name resolves to no declaration.
    pub fn resolve_type_name<S: AsRef<str>>(&self, segments: &[S]) -> Option<ResolvedType> {
        let identity = self.resolve_dotted(segments)?;
        Some(self.declared_type(&identity))
    }

    pub fn resolve_value<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_value_declaration(self.library?, name.into())
    }

    pub fn resolve_sdk_member<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_sdk_member(self.library?, name.into())
    }

    pub fn resolve_imported_member<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_imported_member(self.library?, name.into())
    }

    /// Resolve a visible named extension, which is not a type declaration.
    pub fn resolve_extension<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_extension_declaration(self.library?, name.into())
    }

    /// `name` in the executable library scope, excluding documentation imports.
    pub fn resolve_library_name(&self, name: &str) -> super::DocName {
        self.library.map_or(super::DocName::Unknown, |library| {
            self.identities.library_name(library, name)
        })
    }

    /// `name` in this library's documentation-comment scope (see
    /// [`IdentityIndex::doc_name`]).
    pub fn resolve_doc_name(&self, name: &str) -> super::DocName {
        self.library.map_or(super::DocName::Unknown, |library| {
            self.identities.doc_name(library, name)
        })
    }

    /// `prefix.name` through this library's imports with that prefix.
    pub fn resolve_doc_prefixed(&self, prefix: &str, name: &str) -> super::DocName {
        self.library.map_or(super::DocName::Unknown, |library| {
            self.identities.doc_prefixed_name(library, prefix, name)
        })
    }

    /// The number of parameters declared by a type alias, not its expanded type.
    pub fn type_alias_parameter_count(&self, identity: &DeclarationIdentity) -> Option<usize> {
        self.identities
            .type_alias(identity)
            .map(|alias| alias.type_params.len())
    }

    /// The type the top-level type declaration `identity` denotes when
    /// written without type arguments, typedefs expanded.
    pub fn declared_type(&self, identity: &DeclarationIdentity) -> ResolvedType {
        if let Some(alias) = self.identities.type_alias(identity) {
            return self.expand_alias(identity, alias, &[], false, None);
        }
        let extension_type = self.types.is_some_and(|types| {
            types.type_kind_identity(identity) == Some(super::TypeKind::ExtensionType)
        });
        ResolvedType::Interface {
            occurrence: Default::default(),
            identity: identity.clone(),
            arguments: empty_type_arguments(),
            nullable: false,
            extension_type,
        }
    }

    pub fn resolve_type_in(&self, ty: &DartType, parameters: &TypeParameterScope) -> ResolvedType {
        match ty {
            DartType::Void { .. } => ResolvedType::Void,
            DartType::Dynamic { .. } => ResolvedType::Dynamic,
            DartType::Named(named) => {
                if let [segment] = named.segments.as_slice()
                    && let Some(parameter) = parameters.lookup(&segment.name)
                {
                    return parameter.with_nullable(parameter.nullable() || named.is_nullable);
                }
                let Some(identity) = QualifiedName::from_identifiers(&named.segments)
                    .and_then(|name| self.resolve_name(name))
                else {
                    return ResolvedType::Unknown;
                };
                if identity.is_sdk("dart:core", "Null") {
                    return ResolvedType::Null;
                }
                if identity.is_sdk("dart:core", "Never") {
                    return if named.is_nullable {
                        ResolvedType::Null
                    } else {
                        ResolvedType::Never
                    };
                }
                if let Some(alias) = self.identities.type_alias(&identity) {
                    let arguments: Vec<_> = named
                        .type_args
                        .iter()
                        .map(|argument| self.resolve_type_in(argument, parameters))
                        .collect();
                    return self.expand_alias(
                        &identity,
                        alias,
                        &arguments,
                        named.is_nullable,
                        Some((self.file, named.span.start)),
                    );
                }
                let extension_type = self.types.is_some_and(|types| {
                    types.type_kind_identity(&identity) == Some(super::TypeKind::ExtensionType)
                });
                ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity,
                    arguments: if named.type_args.is_empty() {
                        empty_type_arguments()
                    } else {
                        named
                            .type_args
                            .iter()
                            .map(|argument| self.resolve_type_in(argument, parameters))
                            .collect()
                    },
                    nullable: named.is_nullable,
                    extension_type,
                }
            }
            DartType::Function(function) => {
                let mut function_parameters = parameters.clone();
                function_parameters.push(&function.type_params, self);
                let mut named: Vec<_> = function
                    .params
                    .iter()
                    .filter(|param| param.is_named)
                    .filter_map(|param| {
                        param.name.as_ref().map(|name| {
                            (
                                name.name.clone(),
                                self.resolve_type_in(&param.param_type, &function_parameters),
                            )
                        })
                    })
                    .collect();
                named.sort_by(|left, right| left.0.cmp(&right.0));
                ResolvedType::Function {
                    shape: FunctionTypeShape::from_scope(
                        function
                            .params
                            .iter()
                            .filter(|param| param.is_named && param.is_required)
                            .filter_map(|param| param.name.as_ref().map(|name| name.name.clone()))
                            .collect(),
                        &function.type_params,
                        &function_parameters,
                    ),
                    optional_positional: function
                        .params
                        .iter()
                        .filter(|param| !param.is_named && !param.is_required)
                        .count(),
                    alias: FunctionTypeAlias::default(),
                    return_type: Arc::new(
                        function
                            .return_type
                            .as_deref()
                            .map(|ty| self.resolve_type_in(ty, &function_parameters))
                            .unwrap_or(ResolvedType::Dynamic),
                    ),
                    positional: if function.params.iter().all(|param| param.is_named) {
                        empty_type_arguments()
                    } else {
                        function
                            .params
                            .iter()
                            .filter(|param| !param.is_named)
                            .map(|param| {
                                self.resolve_type_in(&param.param_type, &function_parameters)
                            })
                            .collect()
                    },
                    named: if named.is_empty() {
                        empty_named_types()
                    } else {
                        named.into()
                    },
                    nullable: function.is_nullable,
                }
            }
            DartType::Record(record) => {
                let mut named: Vec<_> = record
                    .named
                    .iter()
                    .map(|field| {
                        (
                            field.name.name.clone(),
                            self.resolve_type_in(&field.field_type, parameters),
                        )
                    })
                    .collect();
                named.sort_by(|left, right| left.0.cmp(&right.0));
                ResolvedType::Record {
                    occurrence: Default::default(),
                    positional: if record.positional.is_empty() {
                        empty_type_arguments()
                    } else {
                        record
                            .positional
                            .iter()
                            .map(|field| self.resolve_type_in(field, parameters))
                            .collect()
                    },
                    named: if named.is_empty() {
                        empty_named_types()
                    } else {
                        named.into()
                    },
                    nullable: record.is_nullable,
                }
            }
        }
    }

    /// The type a use of `alias` with `arguments` denotes: the aliased type,
    /// resolved in the alias's own library, with its type parameters replaced
    /// by the arguments (or, when omitted, by their bounds, as instantiation
    /// to bounds does), made nullable by a `?` on the use. Alias cycles
    /// (invalid Dart) resolve to `Unknown`.
    fn expand_alias(
        &self,
        identity: &DeclarationIdentity,
        alias: &TypeAlias,
        arguments: &[ResolvedType],
        nullable: bool,
        origin: Option<(Option<FileId>, usize)>,
    ) -> ResolvedType {
        let Some(template) = self.alias_template(alias) else {
            return ResolvedType::Unknown;
        };
        let (parameters, aliased) = template.as_ref();
        let mut expanded = if parameters.is_empty() {
            aliased.clone()
        } else {
            let substitutions: HashMap<_, _> = parameters
                .iter()
                .map(|(ordinal, id, default)| {
                    (
                        id.clone(),
                        arguments
                            .get(*ordinal)
                            .cloned()
                            .unwrap_or_else(|| default.clone()),
                    )
                })
                .collect();
            self.substitute(aliased, &substitutions)
        };
        let has_descendant_provenance = expanded.has_alias_provenance();
        match &mut expanded {
            ResolvedType::Function { alias, .. } => {
                *alias = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
                    identity: identity.clone(),
                    origin,
                })));
            }
            _ if has_descendant_provenance => {
                let occurrence = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
                    identity: identity.clone(),
                    origin,
                })));
                expanded = expanded.projected_from(&occurrence);
            }
            _ => {}
        }
        if nullable {
            expanded.with_nullable(true)
        } else {
            expanded
        }
    }

    /// `alias`'s aliased type over its own type parameters, resolved in the
    /// alias's library, with each parameter's ordinal, id, and the argument an
    /// omitted use instantiates it to. `None` for alias cycles (invalid Dart).
    fn alias_template(&self, alias: &TypeAlias) -> Option<Arc<AliasTemplate>> {
        let key = (std::thread::current().id(), self.types.is_some());
        {
            let mut templates = alias.templates.0.lock().expect("alias templates");
            if let Some(template) = templates.get(&key) {
                return match template {
                    AliasTemplateState::Ready(template) => Some(Arc::clone(template)),
                    AliasTemplateState::Resolving | AliasTemplateState::Invalid => {
                        mark_alias_cycle();
                        None
                    }
                };
            }
            templates.insert(key, AliasTemplateState::Resolving);
        }
        let generation = ALIAS_CYCLE_GENERATION.with(std::cell::Cell::get);
        let model = SemanticModel {
            file: Some(alias.file),
            library: Some(alias.library),
            identities: self.identities,
            types: self.types,
        };
        let mut scope = TypeParameterScope::default();
        scope.push(&alias.type_params, &model);
        let aliased = model.resolve_type_in(&alias.aliased, &scope);
        let mut parameters: Vec<_> = alias
            .type_params
            .iter()
            .enumerate()
            .filter_map(|(ordinal, parameter)| {
                let ResolvedType::TypeParameter { id, bound, .. } =
                    scope.lookup(&parameter.name.name)?
                else {
                    return None;
                };
                let default = if parameter.bound.is_some() {
                    bound.as_ref().clone()
                } else {
                    ResolvedType::Dynamic
                };
                Some((ordinal, id, default))
            })
            .collect();
        let bounds = parameters
            .iter()
            .map(|(_, id, bound)| (id.clone(), bound.clone()))
            .collect();
        for (_, id, default) in &mut parameters {
            *default = instantiate_parameter_bound(id, &bounds, &mut HashSet::new(), &model);
        }
        let invalid = ALIAS_CYCLE_GENERATION.with(std::cell::Cell::get) != generation;
        let template = (!invalid).then(|| Arc::new((parameters, aliased)));
        alias.templates.0.lock().expect("alias templates").insert(
            key,
            template
                .as_ref()
                .map_or(AliasTemplateState::Invalid, |template| {
                    AliasTemplateState::Ready(Arc::clone(template))
                }),
        );
        template
    }

    pub fn core_type(&self, name: &'static str, nullable: bool) -> ResolvedType {
        ResolvedType::Interface {
            occurrence: Default::default(),
            identity: DeclarationIdentity::sdk("dart:core", name),
            arguments: empty_type_arguments(),
            nullable,
            extension_type: false,
        }
    }

    pub fn is_future_like(&self, ty: &ResolvedType) -> Truth {
        match ty {
            ResolvedType::Dynamic | ResolvedType::Null | ResolvedType::Never => Truth::Yes,
            ResolvedType::Unknown => Truth::Unknown,
            ResolvedType::Interface {
                extension_type: true,
                ..
            } => Truth::Yes,
            ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk { library, name },
                ..
            } if library.as_str() == "dart:async"
                && matches!(name.as_str(), "Future" | "FutureOr") =>
            {
                Truth::Yes
            }
            ResolvedType::TypeParameter { bound, .. }
            | ResolvedType::NonNullTypeParameter { bound, .. } => self.is_future_like(bound),
            ResolvedType::Interface { identity, .. } => {
                self.types.map_or(Truth::Unknown, |types| {
                    types.is_subtype_identity(
                        identity,
                        &DeclarationIdentity::sdk("dart:async", "Future"),
                    )
                })
            }
            _ => Truth::No,
        }
    }

    pub fn substitute(
        &self,
        ty: &ResolvedType,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
    ) -> ResolvedType {
        if substitutions.is_empty() {
            return ty.clone();
        }
        self.substitute_memoized(ty, substitutions, &mut SubstitutionMemo::default())
    }

    fn substitute_memoized<'t>(
        &self,
        ty: &'t ResolvedType,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
        records: &mut SubstitutionMemo<'t>,
    ) -> ResolvedType {
        self.substitute_memoized_inner(ty, substitutions, records, false)
    }

    fn substitute_memoized_inner<'t>(
        &self,
        ty: &'t ResolvedType,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
        records: &mut SubstitutionMemo<'t>,
        cache_relevance: bool,
    ) -> ResolvedType {
        // Parameter replacements are leaves in this pass. In particular, do
        // not hash their bounds: neither bounds nor replacements are traversed.
        match ty {
            ResolvedType::TypeParameter { id, nullable, .. } => {
                return substitutions
                    .get(id)
                    .map(|ty| ty.with_nullable(*nullable || ty.nullable()))
                    .unwrap_or_else(|| ty.clone());
            }
            ResolvedType::NonNullTypeParameter { id, .. } => {
                return substitutions
                    .get(id)
                    .map(ResolvedType::non_null)
                    .unwrap_or_else(|| ty.clone());
            }
            _ => {}
        }
        let source_key = SubstitutionSourceKey::of(ty);
        if let Some(result) = source_key.and_then(|key| records.sources.get(&key)) {
            return result.clone();
        }
        // Only the outermost root can omit its relevance entry. Recursive
        // substitution must cache irrelevant children that an earlier
        // short-circuiting relevance scan may not have reached.
        if !records.is_relevant(ty, substitutions, cache_relevance) {
            return ty.clone();
        }
        // Hash each source node at most once per pass, even when separately
        // allocated equal subgraphs need structural interning.
        let hash = ty.structural_hash(&mut records.hashes);
        let key = SubstitutionKey(ty);
        if let Some(result) = records.structural.get(&hash).and_then(|bucket| {
            bucket
                .iter()
                .find_map(|(source, result)| (*source == key).then_some(result))
        }) {
            let result = result.clone();
            if let Some(source_key) = source_key {
                records.sources.insert(source_key, result.clone());
            }
            return result;
        }
        let mut function_changed = None;
        let result = match ty {
            ResolvedType::Interface {
                occurrence,
                identity,
                arguments,
                nullable,
                extension_type,
            } => ResolvedType::Interface {
                occurrence: occurrence.clone(),
                identity: identity.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| {
                        self.substitute_memoized_inner(argument, substitutions, records, true)
                    })
                    .collect(),
                nullable: *nullable,
                extension_type: *extension_type,
            },
            ResolvedType::Function {
                return_type,
                positional,
                named,
                nullable,
                alias,
                optional_positional,
                shape,
            } => {
                let mut substituted = ResolvedType::Function {
                    shape: shape.as_ref().map(|shape| {
                        shape.substitute(substitutions, |bound| {
                            self.substitute_memoized_inner(bound, substitutions, records, true)
                        })
                    }),
                    optional_positional: *optional_positional,
                    alias: FunctionTypeAlias::default(),
                    return_type: Arc::new(self.substitute_memoized_inner(
                        return_type,
                        substitutions,
                        records,
                        true,
                    )),
                    positional: positional
                        .iter()
                        .map(|parameter| {
                            self.substitute_memoized_inner(parameter, substitutions, records, true)
                        })
                        .collect(),
                    named: named
                        .iter()
                        .map(|(name, parameter)| {
                            (
                                name.clone(),
                                self.substitute_memoized_inner(
                                    parameter,
                                    substitutions,
                                    records,
                                    true,
                                ),
                            )
                        })
                        .collect(),
                    nullable: *nullable,
                };
                let changed = substituted != *ty;
                function_changed = Some(changed);
                if let ResolvedType::Function {
                    alias: result_alias,
                    ..
                } = &mut substituted
                {
                    *result_alias = if changed {
                        alias.substituted()
                    } else {
                        alias.clone()
                    };
                }
                substituted
            }
            ResolvedType::Record {
                occurrence,
                positional,
                named,
                nullable,
            } => ResolvedType::Record {
                occurrence: occurrence.clone(),
                positional: positional
                    .iter()
                    .map(|field| {
                        self.substitute_memoized_inner(field, substitutions, records, true)
                    })
                    .collect(),
                named: named
                    .iter()
                    .map(|(name, field)| {
                        (
                            name.clone(),
                            self.substitute_memoized_inner(field, substitutions, records, true),
                        )
                    })
                    .collect(),
                nullable: *nullable,
            },
            other => other.clone(),
        };
        let result = if function_changed.unwrap_or_else(|| result != *ty) {
            result
        } else {
            ty.clone()
        };
        if let Some(source_key) = source_key {
            records.sources.insert(source_key, result.clone());
            records
                .structural
                .entry(hash)
                .or_default()
                .push((key, result.clone()));
        }
        result
    }

    pub fn void_context(&self, ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::Void)
            || matches!(ty, ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library, name }, arguments, .. }
                if library.as_str() == "dart:async" && matches!(name.as_str(), "Future" | "FutureOr") && arguments.first().is_some_and(|argument| self.void_context(argument)))
    }

    pub fn acceptable_for_void_context(
        &self,
        expected: &ResolvedType,
        actual: &ResolvedType,
    ) -> Truth {
        if matches!(actual, ResolvedType::Unknown) {
            return Truth::Unknown;
        }
        if matches!(expected, ResolvedType::Void) {
            return if matches!(
                actual,
                ResolvedType::Void | ResolvedType::Null | ResolvedType::Never
            ) {
                Truth::Yes
            } else {
                Truth::No
            };
        }
        let ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk { library, name },
            arguments,
            ..
        } = expected
        else {
            return Truth::Unknown;
        };
        if *library != "dart:async" || !matches!(name.as_str(), "Future" | "FutureOr") {
            return Truth::Unknown;
        }
        if *name == "FutureOr" && matches!(actual, ResolvedType::Dynamic) {
            return Truth::Yes;
        }
        if matches!(
            actual,
            ResolvedType::Null | ResolvedType::Never | ResolvedType::Void
        ) && *name == "FutureOr"
        {
            return Truth::Yes;
        }
        let Some(expected_argument) = arguments.first() else {
            return Truth::Unknown;
        };
        let ResolvedType::Interface {
            identity:
                DeclarationIdentity::Sdk {
                    library: actual_library,
                    name: actual_name,
                },
            arguments: actual_arguments,
            ..
        } = actual
        else {
            return Truth::No;
        };
        if *actual_library != "dart:async" {
            return Truth::No;
        }
        if *actual_name == "Future" {
            return actual_arguments
                .first()
                .map(|argument| self.acceptable_for_void_context(expected_argument, argument))
                .unwrap_or(Truth::Unknown);
        }
        if *name == "FutureOr" && *actual_name == "FutureOr" {
            return actual_arguments
                .first()
                .map(|argument| self.acceptable_for_void_context(expected_argument, argument))
                .unwrap_or(Truth::Unknown);
        }
        Truth::No
    }

    pub fn unrelated(&self, left: &ResolvedType, right: &ResolvedType) -> Truth {
        if matches!(
            left,
            ResolvedType::Unknown
                | ResolvedType::Dynamic
                | ResolvedType::Never
                | ResolvedType::Null
        ) || matches!(
            right,
            ResolvedType::Unknown
                | ResolvedType::Dynamic
                | ResolvedType::Never
                | ResolvedType::Null
        ) {
            return Truth::Unknown;
        }
        if left == right {
            return Truth::No;
        }
        if numeric(left) && numeric(right) {
            return Truth::No;
        }
        if fixnum_and_int(left, right) || fixnum_and_int(right, left) {
            return Truth::No;
        }
        match (left, right) {
            (
                ResolvedType::Interface {
                    identity: left_id,
                    arguments: left_arguments,
                    ..
                },
                ResolvedType::Interface {
                    identity: right_id,
                    arguments: right_arguments,
                    ..
                },
            ) => {
                if left_id == right_id {
                    if left_arguments.len() != right_arguments.len() {
                        return Truth::Unknown;
                    }
                    return if left_arguments
                        .iter()
                        .zip(right_arguments.iter())
                        .any(|(left, right)| self.unrelated(left, right) == Truth::Yes)
                    {
                        Truth::Yes
                    } else {
                        Truth::No
                    };
                }
                let Some(types) = self.types else {
                    return Truth::Unknown;
                };
                match (
                    types.is_subtype_identity(left_id, right_id),
                    types.is_subtype_identity(right_id, left_id),
                ) {
                    (Truth::Yes, _) | (_, Truth::Yes) => Truth::No,
                    (Truth::No, Truth::No) => Truth::Yes,
                    _ => Truth::Unknown,
                }
            }
            (ResolvedType::NonNullTypeParameter { bound, .. }, other)
            | (other, ResolvedType::NonNullTypeParameter { bound, .. }) => {
                self.unrelated(&bound.non_null(), other)
            }
            (ResolvedType::TypeParameter { bound, .. }, other)
            | (other, ResolvedType::TypeParameter { bound, .. }) => self.unrelated(bound, other),
            (ResolvedType::Function { .. }, ResolvedType::Function { .. })
            | (ResolvedType::Record { .. }, ResolvedType::Record { .. }) => {
                fn fields(ty: &ResolvedType) -> (&[ResolvedType], &[(String, ResolvedType)]) {
                    match ty {
                        ResolvedType::Function {
                            positional, named, ..
                        } => (positional.as_ref(), named.as_ref()),
                        ResolvedType::Record {
                            positional, named, ..
                        } => (positional.as_ref(), named.as_ref()),
                        _ => unreachable!(),
                    }
                }
                let (left_positional, left_named) = fields(left);
                let (right_positional, right_named) = fields(right);
                if left_positional.len() != right_positional.len()
                    || !left_named
                        .iter()
                        .map(|(name, _)| name)
                        .eq(right_named.iter().map(|(name, _)| name))
                {
                    return Truth::Yes;
                }
                if let (
                    ResolvedType::Function {
                        return_type: left_return,
                        ..
                    },
                    ResolvedType::Function {
                        return_type: right_return,
                        ..
                    },
                ) = (left, right)
                    && self.unrelated(left_return, right_return) == Truth::Yes
                {
                    return Truth::Yes;
                }
                if left_positional
                    .iter()
                    .zip(right_positional)
                    .chain(
                        left_named
                            .iter()
                            .zip(right_named)
                            .map(|((_, left), (_, right))| (left, right)),
                    )
                    .any(|(left, right)| self.unrelated(left, right) == Truth::Yes)
                {
                    Truth::Yes
                } else {
                    Truth::No
                }
            }
            _ => Truth::Yes,
        }
    }
}

/// Declaration spelling facts that are not part of a parameter's resolved type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterContract {
    pub name: String,
    pub kind: ParamKind,
    pub is_covariant: bool,
    /// Span- and trivia-independent syntax of an explicit default; absent for implicit null.
    pub default_value_code: Option<DefaultSyntaxKey>,
}

fn empty_parameter_contracts() -> Arc<[ParameterContract]> {
    static EMPTY: std::sync::LazyLock<Arc<[ParameterContract]>> =
        std::sync::LazyLock::new(|| Arc::from([]));
    Arc::clone(&EMPTY)
}

fn setter_parameter_contracts(name: &str, is_covariant: bool) -> Arc<[ParameterContract]> {
    Arc::from([ParameterContract {
        name: name.to_string(),
        kind: ParamKind::Positional,
        is_covariant,
        default_value_code: None,
    }])
}

fn parameter_contracts(params: &FormalParamList) -> Arc<[ParameterContract]> {
    if params.params.is_empty() {
        return empty_parameter_contracts();
    }
    params
        .params
        .iter()
        .map(|param| ParameterContract {
            name: param.name.name.clone(),
            kind: param.kind,
            is_covariant: param.is_covariant,
            default_value_code: param
                .default_value
                .as_ref()
                .map(default_syntax::default_syntax_key),
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct ResolvedSignature {
    pub owner_parameters: Vec<TypeParameterId>,
    pub call_parameters: Vec<TypeParameterId>,
    /// Exact required names and declared binders, or `None` when unavailable.
    pub shape: Option<Arc<FunctionTypeShape>>,
    pub positional: Vec<ResolvedType>,
    pub optional_positional: usize,
    /// The default value of each parameter that has one, keyed by parameter
    /// name (positional and named).
    pub defaults: HashMap<String, ParameterDefault>,
    /// Default-bearing positional names in binding order, with empty slots for
    /// parameters without defaults. Empty when no positional defaults exist.
    pub positional_parameter_names: Vec<String>,
    /// Default-bearing parameters that are required or annotated `@required`.
    pub non_optional_defaults: HashSet<String>,
    pub named: HashMap<String, ResolvedType>,
    pub return_type: ResolvedType,
}

impl ResolvedSignature {
    fn returning(return_type: ResolvedType) -> Self {
        Self {
            owner_parameters: Vec::new(),
            call_parameters: Vec::new(),
            shape: None,
            optional_positional: 0,
            defaults: HashMap::new(),
            positional_parameter_names: Vec::new(),
            non_optional_defaults: HashSet::new(),
            positional: Vec::new(),
            named: HashMap::new(),
            return_type,
        }
    }

    /// The callable type, with named parameters in deterministic name order.
    pub fn function_type(&self) -> ResolvedType {
        let mut named: Vec<_> = self
            .named
            .iter()
            .map(|(name, ty)| (name.clone(), ty.clone()))
            .collect();
        named.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        ResolvedType::Function {
            optional_positional: self.optional_positional,
            alias: FunctionTypeAlias::default(),
            return_type: Arc::new(self.return_type.clone()),
            positional: self.positional.clone().into(),
            named: named.into(),
            shape: self.shape.clone(),
            nullable: false,
        }
    }

    /// Recover a local callable's own parameters, excluding enclosing lexical parameters.
    pub fn bind_local_type_parameters(&mut self, scope: &TypeParameterScope) {
        if let Some(shape) = &self.shape {
            self.call_parameters = shape
                .type_parameters
                .iter()
                .filter_map(|ty| match ty {
                    ResolvedType::TypeParameter { id, .. } => Some(id.clone()),
                    _ => None,
                })
                .collect();
            return;
        }
        collect_parameter_ids(&self.return_type, &mut self.call_parameters);
        for ty in &self.positional {
            collect_parameter_ids(ty, &mut self.call_parameters);
        }
        for ty in self.named.values() {
            collect_parameter_ids(ty, &mut self.call_parameters);
        }
        self.call_parameters.retain(|id| {
            !matches!(scope.lookup(&id.name),
            Some(ResolvedType::TypeParameter { id: scoped, .. }) if scoped == *id)
        });
        self.call_parameters.sort_by_key(|id| id.ordinal);
    }

    /// Preserve declared shape while instantiating supplied call or owner parameters.
    pub fn substituted_shape(
        &self,
        model: &SemanticModel<'_>,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
    ) -> Option<Arc<FunctionTypeShape>> {
        self.shape.as_ref().map(|shape| {
            shape.substitute(substitutions, |bound| {
                model.substitute(bound, substitutions)
            })
        })
    }

    /// The signature of a local function or function literal declared with
    /// `type_params` and `params` in `scope` (which binds `type_params`).
    pub fn local_callable(
        scope: &TypeParameterScope,
        type_params: &[TypeParam],
        params: &FormalParamList,
        model: &SemanticModel<'_>,
    ) -> Self {
        signature(scope, &[], type_params, Some(params), None, None, model)
    }
    /// Whether the declared function type uses the owner's type parameters,
    /// directly or through a bound of its own type parameters. The analyzer
    /// substitutes such a member's function type for every invocation,
    /// rebuilding its parameters without their default values.
    pub fn mentions_owner_parameters(&self) -> bool {
        !self.owner_parameters.is_empty()
            && (mentions_any(&self.return_type, &self.owner_parameters)
                || self
                    .positional
                    .iter()
                    .chain(self.named.values())
                    .chain(
                        self.shape
                            .iter()
                            .flat_map(|shape| shape.type_parameters.iter()),
                    )
                    .any(|ty| mentions_any(ty, &self.owner_parameters)))
    }
}

/// A parameter's default value, as the analyzer's `computeConstantValue`
/// sees it.
#[derive(Debug, Clone)]
pub enum ParameterDefault {
    /// An explicit `= value`, evaluated in the declaring library.
    Declared(Arc<Expr>),
    /// An optional parameter without a declared default defaults to `null`.
    Null,
    /// A super formal without a declared default takes the default of the
    /// super-constructor parameter it forwards to, declared by `owner`.
    Inherited {
        owner: DeclarationIdentity,
        default: Box<ParameterDefault>,
    },
}

/// How a super formal parameter (`super.x`) selects its super-constructor
/// parameter.
#[derive(Debug, Clone, Copy)]
enum SuperFormal<'a> {
    /// The n-th positional super formal.
    Positional(usize),
    Named(&'a str),
}

#[derive(Debug, Clone)]
pub struct ConstructorFacts {
    /// The resolved external annotations, such as package:meta's `literal`.
    pub annotations: Vec<DeclarationIdentity>,
    pub name: String,
    pub is_const: bool,
    pub is_factory: bool,
    pub is_private: bool,
    pub parameters: Vec<ParameterFacts>,
    /// A redirecting factory's target type and constructor name. Unknown targets
    /// remain distinguishable from non-redirecting constructors.
    pub redirect_target: Option<(ResolvedType, String)>,
}

#[derive(Debug, Clone)]
pub struct ParameterFacts {
    pub name: String,
    pub ty: ResolvedType,
    pub is_named: bool,
    pub is_super: bool,
}

#[derive(Debug, Clone)]
pub struct StaticConstFacts {
    pub name: String,
    pub ty: ResolvedType,
    pub initializer: Expr,
    pub is_deprecated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberFacts {
    pub declaring_type: DeclarationIdentity,
    pub kind: MemberKind,
    pub is_static: bool,
    pub is_abstract: bool,
    pub positional_parameter_names: Vec<String>,
    /// Accessor-specific declaration facts; `None` is unavailable, not empty.
    pub parameter_contracts: Option<Arc<[ParameterContract]>>,
    /// The resolved external (SDK or package) annotations on this declaration,
    /// such as `meta`'s `awaitNotRequired`.
    pub annotations: Vec<DeclarationIdentity>,
}

#[derive(Debug, Clone)]
pub struct DeclarationFacts {
    pub annotations: Vec<DeclarationIdentity>,
    pub library_target: bool,
    pub constructors: Vec<ConstructorFacts>,
    pub static_consts: Vec<StaticConstFacts>,
    pub extension_representation: Option<ResolvedType>,
    pub is_abstract: bool,
    pub has_subclass_in_library: bool,
}

#[derive(Debug, Clone)]
struct ExtensionFacts {
    declaring_library: Option<LibraryId>,
    name: Option<String>,
    /// The declaration identity of a named extension.
    identity: Option<DeclarationIdentity>,
    on_type: ResolvedType,
    /// The extension's own type parameters.
    parameters: Vec<TypeParameterId>,
    parameter_bounds: HashMap<TypeParameterId, ResolvedType>,
    /// Signatures of the instance methods and operators, over
    /// [`Self::parameters`].
    methods: FxHashMap<String, ResolvedSignature>,
    /// Resolved annotations of executable extension members.
    annotations: FxHashMap<String, Vec<DeclarationIdentity>>,
    /// Return types of the instance getters, over [`Self::parameters`].
    getters: FxHashMap<String, ResolvedType>,
    members: FxHashSet<String>,
    /// Instance methods, getters, setters (as `name=`), and operators.
    instance_members: FxHashSet<String>,
}

/// Owns the indexed spelling once, while borrowing its unencoded member name.
#[derive(Debug, Clone, Eq, PartialEq)]
struct ExtensionMemberName(String);

impl Borrow<str> for ExtensionMemberName {
    fn borrow(&self) -> &str {
        self.0.strip_suffix('=').unwrap_or(&self.0)
    }
}

impl Hash for ExtensionMemberName {
    fn hash<H: Hasher>(&self, state: &mut H) {
        <Self as Borrow<str>>::borrow(self).hash(state);
    }
}

type ExtensionMembers = [FxHashMap<ExtensionMemberName, Vec<usize>>; 2];
#[derive(Debug, Clone, Copy)]
enum InstanceAccess {
    Read,
    Write,
    Invoke,
}

#[derive(Debug, Clone, Default)]
struct MemberEntry {
    signature: Option<ResolvedSignature>,
    override_shape: Option<inherited::OverrideShape>,
    is_instance_method: bool,
    field: Option<(Vec<TypeParameterId>, ResolvedType)>,
    read: Option<ResolvedType>,
    write: Option<ResolvedType>,
    facts: Vec<MemberFacts>,
    promotable_field: bool,
    implicit_getter: bool,
}

/// The parts of a `const` constructor that constant evaluation runs.
#[derive(Debug, Clone)]
pub struct ConstConstructorBody {
    pub is_primary: bool,
    pub params: Vec<falcon_syntax::ast::FormalParam>,
    pub initializers: Vec<falcon_syntax::ast::ConstructorInitializer>,
    pub redirect: Option<falcon_syntax::ast::RedirectedConstructor>,
}

type ExternalSignatureMap<K, V> = (
    Arc<SignatureIndex>,
    fn(&SignatureIndex) -> &SignatureMap<K, V>,
);

/// Project-owned entries with a borrowed, immutable external table. Mutations
/// always target the project table; a declaration shadows its base entry whole.
#[derive(Clone)]
struct SignatureMap<K, V> {
    local: FxHashMap<K, V>,
    external: Option<ExternalSignatureMap<K, V>>,
}

impl<K: std::fmt::Debug + Eq + Hash, V: std::fmt::Debug> std::fmt::Debug for SignatureMap<K, V> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_map().entries(self.iter()).finish()
    }
}

impl<K, V> Default for SignatureMap<K, V> {
    fn default() -> Self {
        Self {
            local: FxHashMap::default(),
            external: None,
        }
    }
}

impl<K, V> Deref for SignatureMap<K, V> {
    type Target = FxHashMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.local
    }
}

impl<K, V> DerefMut for SignatureMap<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.local
    }
}

impl<K: Eq + Hash, V> SignatureMap<K, V> {
    fn get<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.local.get(key).or_else(|| {
            let (base, field) = self.external.as_ref()?;
            field(base).get(key)
        })
    }

    fn contains_key<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.get(key).is_some()
    }

    fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.local
            .iter()
            .chain(self.external.iter().flat_map(move |(base, field)| {
                field(base)
                    .local
                    .iter()
                    .filter(move |(key, _)| !self.local.contains_key(*key))
            }))
    }
}

// Declare shared maps once so adding a table also wires its external overlay.
macro_rules! signature_index {
    (
        local { $($(#[$local_doc:meta])* $local:ident: $local_type:ty,)* }
        shared { $($(#[$shared_doc:meta])* $shared:ident: $shared_type:ty,)* }
    ) => {
        #[derive(Debug, Clone, Default)]
        pub struct SignatureIndex {
            $($(#[$local_doc])* $local: $local_type,)*
            $($(#[$shared_doc])* $shared: $shared_type,)*
            /// Only external-only snapshots may be retained here, never project facts.
            external: Option<Arc<SignatureIndex>>,
        }

        impl SignatureIndex {
            fn borrowing_external(external: Arc<Self>) -> Self {
                assert!(
                    external.external.is_none(),
                    "external base must not be an overlay"
                );
                Self {
                    $($shared: SignatureMap {
                        local: FxHashMap::default(),
                        external: Some((Arc::clone(&external), |base| &base.$shared)),
                    },)*
                    external: Some(external),
                    ..Self::default()
                }
            }
        }
    };
}

signature_index! {
    local {
        flow_writes: FxHashMap<Option<FileId>, Arc<super::flow_writes::FlowWrites>>,
        /// Getter reads may execute code even when their static type is scalar.
        value_getters: FxHashSet<DeclarationIdentity>,
        /// Cached after member ingestion; overlays consult this without scanning the base.
        has_field_forwarders: bool,
        extensions: Vec<ExtensionFacts>,
        /// Every identity whose supertype graph reaches a cycle, once the index
        /// is complete; `None` while building, when lookups walk the graph.
        cyclic_supertypes: Option<FxHashSet<DeclarationIdentity>>,
        /// Inferred top-level variables in an initializer dependency SCC.
        cyclic_values: FxHashSet<DeclarationIdentity>,
        /// The [`Self::extensions`] declaring each member name (methods, other
        /// members, and instance members), once the index is complete.
        extension_members: Option<ExtensionMembers>,
    }
    shared {
        functions: SignatureMap<DeclarationIdentity, ResolvedSignature>,
        /// Read types of top-level getters and variables, including imported ones.
        value_types: SignatureMap<DeclarationIdentity, ResolvedType>,
        constructors: SignatureMap<DeclarationIdentity, FxHashMap<String, ResolvedSignature>>,
        member_entries: SignatureMap<DeclarationIdentity, FxHashMap<String, MemberEntry>>,
        /// Non-promotable private instance names conflict library-wide.
        blocked_field_promotions: SignatureMap<LibraryId, FxHashSet<String>>,
        supertypes: SignatureMap<DeclarationIdentity, (Vec<TypeParameterId>, Vec<ResolvedType>)>,
        owner_defaults: SignatureMap<TypeParameterId, ResolvedType>,
        /// Whether the declaration of each type parameter of an indexed type,
        /// alias, function or method states a bound; parameters of local
        /// functions are absent.
        bound_declared: SignatureMap<TypeParameterId, bool>,
        member_supertypes: SignatureMap<DeclarationIdentity, Vec<ResolvedType>>,
        declarations: SignatureMap<DeclarationIdentity, DeclarationFacts>,
        constants: SignatureMap<DeclarationIdentity, Expr>,
        constant_types: SignatureMap<DeclarationIdentity, DeclarationIdentity>,
        enum_values: SignatureMap<DeclarationIdentity, Vec<String>>,
        /// Bodies of project `const` constructors, keyed by class and name.
        const_constructors: SignatureMap<DeclarationIdentity, FxHashMap<String, ConstConstructorBody>>,
        /// Instance field initializers of project classes with a `const`
        /// constructor, in declaration order.
        const_field_initializers: SignatureMap<DeclarationIdentity, Vec<(String, Expr)>>,
        /// Resolved external annotations of top-level functions that have any.
        function_annotations: SignatureMap<DeclarationIdentity, Vec<DeclarationIdentity>>,
        /// The declaration kind of every indexed class-like declaration.
        kinds: SignatureMap<DeclarationIdentity, TypeKind>,
        /// Whether each class, mixin, enum, or extension type is itself annotated
        /// with package:meta's `@immutable` (`Unknown` for an unresolvable
        /// `immutable` annotation, which may arrive through an export chain).
        immutable: SignatureMap<DeclarationIdentity, Truth>,
        /// The superclass of classes, mixin classes, and named mixin applications.
        superclasses: SignatureMap<DeclarationIdentity, ResolvedType>,
        /// The `with` clause of each class or enum declaring one, as the index
        /// range of its mixins within the declaration's `supertypes` entry.
        mixin_applications: SignatureMap<DeclarationIdentity, Range<usize>>,
        /// The `on`-clause prefix of each mixin's canonical `supertypes` entry.
        mixin_constraint_counts: SignatureMap<DeclarationIdentity, usize>,
        /// Whether the trailing implicit `Object` subtype edge is also a direct
        /// inherited-interface candidate. When used, it precedes mixins/interfaces.
        inherited_object: SignatureMap<DeclarationIdentity, bool>,
        /// The explicit class modifier of each class, mixin class, and mixin that
        /// declares one (`base` for base mixins and mixin classes).
        restrictions: SignatureMap<DeclarationIdentity, ClassRestriction>,
    }
}

/// A value declaration is either unqualified or import-prefixed. Longer
/// instance chains are handled one receiver at a time, never rescanned.
fn initializer_value_name(expression: &Expr) -> Option<QualifiedName<'_>> {
    match expression.unparenthesized() {
        Expr::Ident(identifier) => Some(identifier.name.as_str().into()),
        Expr::Field { object, field, .. } => {
            let Expr::Ident(prefix) = object.as_ref() else {
                return None;
            };
            Some(QualifiedName::prefixed(&prefix.name, &field.name))
        }
        _ => None,
    }
}

/// The shared initializer traversal keeps lexical names out of both the root
/// cycle graph and readiness checks. Callbacks run after children, allowing
/// receiver chains to reuse their immediate receiver's result.
struct InitializerDependencies<F> {
    environment: TypeEnvironment,
    inspect: F,
}

impl<F: FnMut(&Expr, &TypeEnvironment)> InitializerDependencies<F> {
    fn bind(&mut self, name: &str) {
        self.environment.declare(name, ResolvedType::Unknown);
    }

    fn bind_statements(&mut self, statements: &[falcon_syntax::ast::Stmt]) {
        use falcon_syntax::ast::Stmt;
        for statement in statements {
            match statement {
                Stmt::LocalVar(variable) => {
                    for declarator in &variable.declarators {
                        self.bind(&declarator.name.name);
                    }
                }
                Stmt::LocalFunc(function) => self.bind(&function.name.name),
                Stmt::PatternDecl(declaration) => self.bind_pattern(&declaration.pattern),
                _ => {}
            }
        }
    }

    fn bind_pattern(&mut self, pattern: &falcon_syntax::ast::Pattern) {
        struct Bindings<'a>(&'a mut TypeEnvironment);
        impl Visitor for Bindings<'_> {
            fn visit_pattern(&mut self, pattern: &falcon_syntax::ast::Pattern) {
                if let falcon_syntax::ast::Pattern::Variable { name, .. } = pattern {
                    self.0.declare(name.name.clone(), ResolvedType::Unknown);
                }
                if let falcon_syntax::ast::Pattern::Object(object) = pattern {
                    for field in &object.fields {
                        if field.pattern.is_none() {
                            self.0
                                .declare(field.name.name.clone(), ResolvedType::Unknown);
                        }
                    }
                }
                falcon_syntax::visitor::walk_pattern(self, pattern);
            }
            fn visit_expr(&mut self, _: &Expr) {}
        }
        Bindings(&mut self.environment).visit_pattern(pattern);
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.environment.push_scope();
        visit(self);
        self.environment.pop_scope();
    }

    fn condition(
        &mut self,
        condition: &falcon_syntax::ast::IfCondition,
        then: impl FnOnce(&mut Self),
    ) {
        use falcon_syntax::ast::IfCondition;
        match condition {
            IfCondition::Expr(expression) => {
                self.visit_expr(expression);
                self.scoped(then);
            }
            IfCondition::Case(expression, pattern, guard) => {
                self.visit_expr(expression);
                self.scoped(|visitor| {
                    visitor.bind_pattern(pattern);
                    visitor.visit_pattern(pattern);
                    if let Some(guard) = guard {
                        visitor.visit_expr(guard);
                    }
                    then(visitor);
                });
            }
        }
    }
}

impl<F: FnMut(&Expr, &TypeEnvironment)> Visitor for InitializerDependencies<F> {
    fn visit_expr(&mut self, expression: &Expr) {
        if let Expr::FuncExpr { params, .. } = expression {
            self.scoped(|visitor| {
                for parameter in &params.params {
                    visitor.bind(&parameter.name.name);
                }
                walk_expr(visitor, expression);
                (visitor.inspect)(expression, &visitor.environment);
            });
        } else {
            walk_expr(self, expression);
            (self.inspect)(expression, &self.environment);
        }
    }

    fn visit_block(&mut self, block: &falcon_syntax::ast::Block) {
        self.scoped(|visitor| {
            // Local declarations shadow throughout their block.
            visitor.bind_statements(&block.stmts);
            falcon_syntax::visitor::walk_block(visitor, block);
        });
    }

    fn visit_stmt(&mut self, statement: &falcon_syntax::ast::Stmt) {
        use falcon_syntax::ast::Stmt;
        match statement {
            Stmt::LocalFunc(function) => self.scoped(|visitor| {
                for parameter in &function.params.params {
                    visitor.bind(&parameter.name.name);
                }
                walk_stmt(visitor, statement);
            }),
            Stmt::For(_) => self.scoped(|visitor| walk_stmt(visitor, statement)),
            Stmt::If(statement) => {
                self.condition(&statement.condition, |visitor| {
                    visitor.visit_stmt(&statement.then_branch)
                });
                if let Some(branch) = &statement.else_branch {
                    self.visit_stmt(branch);
                }
            }
            _ => walk_stmt(self, statement),
        }
    }

    fn visit_for_header(&mut self, header: &falcon_syntax::ast::ForHeader) {
        use falcon_syntax::ast::ForInit;
        match &header.init {
            Some(ForInit::VarDecl(declaration)) => {
                for declarator in &declaration.declarators {
                    self.bind(&declarator.name.name);
                }
            }
            Some(ForInit::ForIn {
                name,
                declares_variable: true,
                iterable,
                ..
            }) => {
                self.visit_expr(iterable);
                self.bind(&name.name);
                return;
            }
            Some(ForInit::PatternForIn {
                pattern, iterable, ..
            }) => {
                self.visit_expr(iterable);
                self.bind_pattern(pattern);
                self.visit_pattern(pattern);
                return;
            }
            _ => {}
        }
        falcon_syntax::visitor::walk_for_header(self, header);
    }

    fn visit_catch_clause(&mut self, catch: &falcon_syntax::ast::CatchClause) {
        self.scoped(|visitor| {
            for name in [&catch.exception_var, &catch.stack_trace_var]
                .into_iter()
                .flatten()
            {
                visitor.bind(&name.name);
            }
            falcon_syntax::visitor::walk_catch_clause(visitor, catch);
        });
    }

    fn visit_switch_case(&mut self, case: &falcon_syntax::ast::SwitchCase) {
        self.scoped(|visitor| {
            for kind in &case.cases {
                if let falcon_syntax::ast::SwitchCaseKind::Pattern(pattern, _) = kind {
                    visitor.bind_pattern(pattern);
                }
            }
            visitor.bind_statements(&case.body);
            falcon_syntax::visitor::walk_switch_case(visitor, case);
        });
    }

    fn visit_switch_expr_arm(&mut self, arm: &falcon_syntax::ast::SwitchExprArm) {
        self.scoped(|visitor| {
            visitor.bind_pattern(&arm.pattern);
            falcon_syntax::visitor::walk_switch_expr_arm(visitor, arm);
        });
    }

    fn visit_collection_element(&mut self, element: &falcon_syntax::ast::CollectionElement) {
        use falcon_syntax::ast::CollectionElement;
        match element {
            CollectionElement::For { .. } => self.scoped(|visitor| {
                falcon_syntax::visitor::walk_collection_element(visitor, element)
            }),
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                self.condition(condition, |visitor| {
                    visitor.visit_collection_element(then_elem)
                });
                if let Some(element) = else_elem {
                    self.visit_collection_element(element);
                }
            }
            _ => falcon_syntax::visitor::walk_collection_element(self, element),
        }
    }

    fn visit_map_element(&mut self, element: &falcon_syntax::ast::MapElement) {
        use falcon_syntax::ast::MapElement;
        match element {
            MapElement::For { .. } => {
                self.scoped(|visitor| falcon_syntax::visitor::walk_map_element(visitor, element))
            }
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => {
                self.condition(condition, |visitor| visitor.visit_map_element(then_entry));
                if let Some(element) = else_entry {
                    self.visit_map_element(element);
                }
            }
            _ => falcon_syntax::visitor::walk_map_element(self, element),
        }
    }
}

impl SignatureIndex {
    pub(crate) fn flow_writes(&self, file: FileId) -> Option<&Arc<super::flow_writes::FlowWrites>> {
        self.flow_writes.get(&Some(file))
    }

    fn member_entry(&self, identity: &DeclarationIdentity, name: &str) -> Option<&MemberEntry> {
        self.member_entries.get(identity)?.get(name)
    }

    pub fn from_program(program: &Program, model: &SemanticModel<'_>) -> Self {
        let mut index = Self::default();
        index.add_sdk_signatures();
        index.add_program(program, model);
        index
    }

    /// Precompute lookup facts once declarations and supertype edges are
    /// complete. Declaration type inference does not change these facts.
    fn finish(&mut self) {
        let mut cyclic = self.cyclic_supertype_identities();
        if let Some(base) = &self.external {
            cyclic.extend(base.cyclic_supertypes.iter().flatten().cloned());
        }
        self.cyclic_supertypes = Some(cyclic);
        let mut extension_members = ExtensionMembers::default();
        for (index, extension) in self.extensions.iter().enumerate() {
            let names = extension
                .members
                .iter()
                .chain(&extension.instance_members)
                .chain(extension.methods.keys());
            for name in names {
                let names = &mut extension_members[usize::from(name.ends_with('='))];
                let bare_name = name.strip_suffix('=').unwrap_or(name);
                match names.get_mut(bare_name) {
                    // Names repeat across an extension's member sets.
                    Some(extensions) if extensions.last() == Some(&index) => {}
                    Some(extensions) => extensions.push(index),
                    None => {
                        names.insert(ExtensionMemberName(name.clone()), vec![index]);
                    }
                }
            }
        }
        self.extension_members = Some(extension_members);
    }

    /// The extensions that may declare the member `name`, in declaration
    /// order: every extension until the index is complete.
    fn local_extensions_with_member<'s>(
        &'s self,
        name: &str,
    ) -> impl Iterator<Item = &'s ExtensionFacts> + 's {
        let indexed = self.extension_members.as_ref().map(|index| {
            index[usize::from(name.ends_with('='))]
                .get(name.strip_suffix('=').unwrap_or(name))
                .map_or(&[][..], Vec::as_slice)
        });
        let unindexed = if indexed.is_none() {
            self.extensions.as_slice()
        } else {
            &[]
        };
        indexed
            .unwrap_or_default()
            .iter()
            .map(|&index| &self.extensions[index])
            .chain(unindexed)
    }

    fn extensions_with_member<'s>(
        &'s self,
        name: &str,
    ) -> impl Iterator<Item = &'s ExtensionFacts> + 's {
        let external = self
            .external
            .as_ref()
            .map(|base| base.local_extensions_with_member(name));
        self.local_extensions_with_member(name)
            .chain(external.into_iter().flatten())
    }

    fn all_extensions(&self) -> impl Iterator<Item = &ExtensionFacts> {
        self.extensions
            .iter()
            .chain(self.external.iter().flat_map(|base| base.extensions.iter()))
    }

    pub fn from_project_files(
        files: &[(PathBuf, &Program)],
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        Self::build(files, None, identities, types)
    }

    /// Index only the current project, borrowing external signatures and facts.
    /// `external` must be an external-only, non-overlay snapshot.
    pub fn with_external(
        files: &[(PathBuf, &Program)],
        identities: &IdentityIndex,
        types: &TypeIndex,
        external: Arc<SignatureIndex>,
    ) -> Self {
        let index = Self::borrowing_external(external);
        let all = files
            .iter()
            .map(|(path, program)| (identities.file(path), *program, &[] as &[(usize, usize)]))
            .collect();
        Self::build_programs(index, all, identities, types)
    }

    /// Index project `files` and, when given, the loaded external libraries
    /// (which must also be indexed by `identities`). The curated SDK/package
    /// tables only supply declarations whose sources are not loaded.
    pub fn build(
        files: &[(PathBuf, &Program)],
        external: Option<&ExternalLibraries>,
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        let all: Vec<IndexedProgram<'_>> =
            external
                .into_iter()
                .flat_map(ExternalLibraries::libraries)
                .enumerate()
                .flat_map(|(instance, library)| {
                    library
                        .files
                        .iter()
                        .enumerate()
                        .map(move |(ordinal, file)| {
                            (
                                identities.external_file(instance, ordinal),
                                &file.facts,
                                file.concrete_operators.as_slice(),
                            )
                        })
                })
                .chain(files.iter().map(|(path, program)| {
                    (identities.file(path), *program, &[] as &[(usize, usize)])
                }))
                .collect();
        Self::build_programs(Self::default(), all, identities, types)
    }

    fn build_programs(
        mut index: Self,
        all: Vec<IndexedProgram<'_>>,
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        for (file, program, concrete_operators) in &all {
            let model = SemanticModel::from_file(*file, identities, Some(types));
            index.add_declarations(program, &model);
            // Compact external facts have no bodies: operator abstractness
            // therefore comes from their explicit retained presence metadata.
            for &(declaration, member) in *concrete_operators {
                let (owner, members) = match &program.declarations[declaration] {
                    TopLevelDecl::Class(class) => (&class.name.name, &class.members),
                    TopLevelDecl::MixinClass(class) => (&class.name.name, &class.members),
                    TopLevelDecl::Mixin(mixin) => (&mixin.name.name, &mixin.members),
                    TopLevelDecl::Enum(enumeration) => {
                        (&enumeration.name.name, &enumeration.members)
                    }
                    TopLevelDecl::ExtensionType(extension) => {
                        (&extension.name.name, &extension.members)
                    }
                    _ => continue,
                };
                if !model.owns_declaration(owner) {
                    continue;
                }
                let ClassMember::Operator(operator) = &members[member] else {
                    continue;
                };
                if let Some(identity) = model.resolve_name(owner.as_str())
                    && let Some(entry) = index
                        .member_entries
                        .get_mut(&identity)
                        .and_then(|members| members.get_mut(operator.lookup_name()))
                {
                    for facts in &mut entry.facts {
                        if facts.kind == MemberKind::Operator {
                            facts.is_abstract = false;
                        }
                    }
                }
            }
        }
        if index.external.is_none() {
            index.add_fallback_signatures();
        }
        index.finish();
        let programs: Vec<_> = all
            .iter()
            .map(|(file, program, _)| {
                (
                    *program,
                    SemanticModel::from_file(*file, identities, Some(types)),
                )
            })
            .collect();
        index.infer_programs(&programs);
        index.mark_library_subclasses();
        index.mark_field_forwarders(identities, Some(types));
        index
    }

    /// Seed the curated SDK/package tables for every declaration no indexed
    /// source supplies: without loaded sources they are the fallback; with
    /// them, a declaration's real facts always win as a whole.
    fn add_fallback_signatures(&mut self) {
        let mut fallback = Self::default();
        fallback.add_sdk_signatures();
        fn fill<V>(
            target: &mut FxHashMap<DeclarationIdentity, V>,
            source: SignatureMap<DeclarationIdentity, V>,
        ) {
            for (identity, value) in source.local {
                target.entry(identity).or_insert(value);
            }
        }
        fill(&mut self.functions, fallback.functions);
        fill(&mut self.value_types, fallback.value_types);
        fill(&mut self.constructors, fallback.constructors);
        fill(&mut self.member_entries, fallback.member_entries);
        fill(&mut self.supertypes, fallback.supertypes);
        fill(&mut self.superclasses, fallback.superclasses);
        fill(&mut self.member_supertypes, fallback.member_supertypes);
        fill(&mut self.declarations, fallback.declarations);
        fill(&mut self.constants, fallback.constants);
        fill(&mut self.constant_types, fallback.constant_types);
    }

    /// Visit each initializer once, resolving import prefixes to declaration
    /// identities before finding cycles across all indexed libraries.
    fn collect_value_dependencies(
        program: &Program,
        model: &SemanticModel<'_>,
        graph: &mut FxHashMap<DeclarationIdentity, Vec<DeclarationIdentity>>,
    ) {
        let mut edges = FxHashSet::default();
        for declaration in &program.declarations {
            let TopLevelDecl::Variable(variable) = declaration else {
                continue;
            };
            if variable.var_type.is_some() {
                continue;
            }
            for declarator in &variable.declarators {
                if !model.owns_declaration(&declarator.name.name) {
                    continue;
                }
                if let Some(initializer) = &declarator.initializer
                    && let Some(identity) = model.resolve_value(&declarator.name.name)
                {
                    edges.clear();
                    InitializerDependencies {
                        environment: TypeEnvironment::new(),
                        inspect: |expression: &Expr, environment: &TypeEnvironment| {
                            if let Some(name) = initializer_value_name(expression)
                                && !environment.is_bound(name.prefix.unwrap_or(name.name))
                                && let Some(identity) = model.resolve_value(name)
                            {
                                edges.insert(identity);
                            }
                        },
                    }
                    .visit_expr(initializer);
                    graph.insert(identity, edges.iter().cloned().collect());
                }
            }
        }
    }

    /// Iterative Kosaraju traversal: O(V + E), without revisiting initializer
    /// syntax or marking acyclic consumers of a cycle as cyclic themselves.
    fn cyclic_value_identities(
        graph: &FxHashMap<DeclarationIdentity, Vec<DeclarationIdentity>>,
    ) -> FxHashSet<DeclarationIdentity> {
        let mut visited = FxHashSet::default();
        let mut order = Vec::with_capacity(graph.len());
        let mut reverse: FxHashMap<_, Vec<_>> = FxHashMap::default();
        for (identity, edges) in graph {
            for edge in edges.iter().filter(|edge| graph.contains_key(*edge)) {
                reverse.entry(edge).or_default().push(identity);
            }
        }
        let mut stack = Vec::with_capacity(graph.len());
        for identity in graph.keys() {
            stack.push((identity, false));
            while let Some((identity, finished)) = stack.pop() {
                if finished {
                    order.push(identity);
                } else if visited.insert(identity) {
                    stack.push((identity, true));
                    stack.extend(
                        graph[identity]
                            .iter()
                            .filter(|edge| graph.contains_key(*edge))
                            .map(|edge| (edge, false)),
                    );
                }
            }
        }
        visited.clear();
        let mut cyclic = FxHashSet::default();
        let mut component = Vec::with_capacity(graph.len());
        for identity in order.into_iter().rev() {
            if !visited.insert(identity) {
                continue;
            }
            component.clear();
            component.push(identity);
            let mut cursor = 0;
            while cursor < component.len() {
                if let Some(edges) = reverse.get(component[cursor]) {
                    for &edge in edges {
                        if visited.insert(edge) {
                            component.push(edge);
                        }
                    }
                }
                cursor += 1;
            }
            if component.len() > 1 || graph[identity].contains(identity) {
                cyclic.extend(component.iter().map(|identity| (*identity).clone()));
            }
        }
        cyclic
    }

    fn resolve_value_initializers(
        &mut self,
        program: &Program,
        model: &SemanticModel<'_>,
        pending: &mut FxHashMap<DeclarationIdentity, ResolvedType>,
        retry_partial: bool,
    ) -> bool {
        let mut changed = false;
        for declaration in &program.declarations {
            let TopLevelDecl::Variable(variable) = declaration else {
                continue;
            };
            if variable.var_type.is_some() {
                continue;
            }
            for declarator in &variable.declarators {
                if !model.owns_declaration(&declarator.name.name) {
                    continue;
                }
                let Some(identity) = model.resolve_value(&declarator.name.name) else {
                    continue;
                };
                if self.is_cyclic_value(&identity) {
                    continue;
                }
                if self
                    .value_types
                    .get(&identity)
                    .is_some_and(|ty| !has_inference_holes(ty))
                {
                    continue;
                }
                let Some(initializer) = &declarator.initializer else {
                    continue;
                };
                let ty = TypeEnvironment::new().infer_with_signatures(
                    initializer,
                    model,
                    self,
                    &TypeParameterScope::default(),
                );
                let ty = if ty == ResolvedType::Null {
                    ResolvedType::Dynamic
                } else {
                    ty
                };
                if has_inference_holes(&ty) {
                    // Retain partial evidence until concrete progress stops.
                    // Once published, only accept a hole-free replacement:
                    // feeding partial refinements back would grow recursive
                    // containers indefinitely.
                    if ty != ResolvedType::Unknown
                        && (retry_partial
                            || self
                                .value_types
                                .get(&identity)
                                .is_none_or(|published| *published == ResolvedType::Unknown))
                    {
                        pending.insert(identity, ty);
                    } else {
                        pending.remove(&identity);
                    }
                } else {
                    if retry_partial
                        && !self.declaration_dependencies_known(
                            initializer,
                            model,
                            None,
                            &TypeEnvironment::new(),
                            &TypeParameterScope::default(),
                        )
                    {
                        // Replace stale partial evidence even when this
                        // concrete candidate must wait for a dependency.
                        pending.insert(identity, ty);
                        continue;
                    }
                    pending.remove(&identity);
                    self.value_types.insert(identity, ty);
                    changed = true;
                }
            }
        }
        changed
    }

    fn add_program(&mut self, program: &Program, model: &SemanticModel<'_>) {
        self.add_declarations(program, model);
        self.finish();
        self.infer_programs(&[(program, *model)]);
        self.mark_field_forwarders(model.identities, model.types);
    }

    fn converge(
        &mut self,
        programs: &[(&Program, SemanticModel<'_>)],
        mut step: impl FnMut(&mut Self, &Program, &SemanticModel<'_>) -> bool,
    ) {
        loop {
            let mut changed = false;
            for (program, model) in programs {
                changed |= step(self, program, model);
            }
            if !changed {
                break;
            }
        }
    }

    fn infer_programs(&mut self, programs: &[(&Program, SemanticModel<'_>)]) {
        let mut dependencies = FxHashMap::default();
        for (program, model) in programs {
            Self::collect_value_dependencies(program, model, &mut dependencies);
        }
        self.cyclic_values = Self::cyclic_value_identities(&dependencies);
        // Settle forwarded and inherited headers before initializers can freeze implicit types.
        self.converge(programs, Self::resolve_constructor_formals);
        self.converge(programs, Self::infer_override_types);
        let mut pending_values = FxHashMap::default();
        let mut retry_partial = true;
        loop {
            loop {
                let mut declarations_changed = false;
                let mut values_changed = false;
                for (program, model) in programs {
                    declarations_changed |= self.infer_declaration_types(program, model);
                    values_changed |= self.resolve_value_initializers(
                        program,
                        model,
                        &mut pending_values,
                        retry_partial,
                    );
                }
                if declarations_changed {
                    self.converge(programs, Self::resolve_constructor_formals);
                }
                if !declarations_changed && !values_changed {
                    break;
                }
            }
            if pending_values.is_empty() {
                break;
            }
            // Published partials may become concrete, but never expand to another partial.
            self.value_types.extend(pending_values.drain());
            retry_partial = false;
        }
    }

    /// Propagate callable headers before initializers and revisit dependencies
    /// whose inherited getter or field types were previously unresolved.
    fn infer_override_types(&mut self, program: &Program, model: &SemanticModel<'_>) -> bool {
        let mut changed = false;
        let mut lookup = inherited::OverrideInferenceLookup::default();
        for declaration in &program.declarations {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let (name, members) = match declaration {
                TopLevelDecl::Class(class) => (&class.name, &class.members),
                TopLevelDecl::MixinClass(class) => (&class.name, &class.members),
                TopLevelDecl::Mixin(mixin) => (&mixin.name, &mixin.members),
                TopLevelDecl::Enum(enumeration) => (&enumeration.name, &enumeration.members),
                // Extension type members do not override implemented members.
                _ => continue,
            };
            let Some(identity) = model.resolve_name(&name.name) else {
                continue;
            };
            for member in members
                .iter()
                .filter(|member| inherits_method_types(member))
            {
                changed |= self.inherit_method_types(&identity, member, model, &mut lookup);
            }
        }
        changed
    }

    /// Advance callable, field, and getter inference to a fixed point.
    /// Unresolved initializer dependencies remain holes for later passes.
    fn infer_declaration_types(&mut self, program: &Program, model: &SemanticModel<'_>) -> bool {
        let mut changed = self.infer_override_types(program, model);
        for declaration in &program.declarations {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let (name, parameters, members) = match declaration {
                TopLevelDecl::Class(class) => (&class.name, &class.type_params, &class.members),
                TopLevelDecl::MixinClass(class) => {
                    (&class.name, &class.type_params, &class.members)
                }
                TopLevelDecl::Mixin(mixin) => (&mixin.name, &mixin.type_params, &mixin.members),
                TopLevelDecl::Enum(enumeration) => (
                    &enumeration.name,
                    &enumeration.type_params,
                    &enumeration.members,
                ),
                TopLevelDecl::ExtensionType(extension) => {
                    (&extension.name, &extension.type_params, &extension.members)
                }
                _ => continue,
            };
            let Some(identity) = model.resolve_name(&name.name) else {
                continue;
            };
            if !members.iter().any(|member| match member {
                ClassMember::Field(field) if field.field_type.is_none() => {
                    field.declarators.iter().any(|declarator| {
                        self.member_entry(&identity, &declarator.name.name)
                            .and_then(|entry| entry.field.as_ref())
                            .is_some_and(|(_, ty)| {
                                matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic)
                            })
                    })
                }
                ClassMember::Getter(getter) if getter.return_type.is_none() => self
                    .member_type_pending(
                        &identity,
                        &getter.name.name,
                        model,
                        &mut FxHashSet::default(),
                    ),
                _ => false,
            }) {
                continue;
            }
            let mut scope = TypeParameterScope::default();
            scope.push(parameters, model);
            let receiver = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: identity.clone(),
                arguments: parameters
                    .iter()
                    .filter_map(|parameter| scope.lookup(&parameter.name.name))
                    .collect(),
                nullable: false,
                extension_type: matches!(declaration, TopLevelDecl::ExtensionType(_)),
            };
            let mut environment = TypeEnvironment::new();
            environment.declare("this", receiver);
            for member in members {
                match member {
                    ClassMember::Field(field) if field.field_type.is_none() => {
                        for declarator in &field.declarators {
                            changed |= self.infer_member_type(
                                &identity,
                                &declarator.name.name,
                                declarator.initializer.as_ref(),
                                model,
                                &environment,
                                &scope,
                            );
                        }
                    }
                    ClassMember::Getter(getter)
                        if getter.return_type.is_none()
                            && self.member_type_pending(
                                &identity,
                                &getter.name.name,
                                model,
                                &mut FxHashSet::default(),
                            ) =>
                    {
                        changed |= self.infer_member_type(
                            &identity,
                            &getter.name.name,
                            None,
                            model,
                            &environment,
                            &scope,
                        );
                    }
                    _ => {}
                }
            }
        }
        changed
    }

    /// Fill omitted instance callable types from the native combined inherited
    /// signature, including contravariant parameters and generic alpha-renaming.
    fn inherit_method_types(
        &mut self,
        identity: &DeclarationIdentity,
        member: &ClassMember,
        model: &SemanticModel<'_>,
        lookup: &mut inherited::OverrideInferenceLookup,
    ) -> bool {
        let (name, params, infer_return, setter) = match member {
            ClassMember::Method(method) => (
                method.name.name.as_str(),
                Some(&method.params),
                method.return_type.is_none(),
                false,
            ),
            ClassMember::Operator(operator) => (
                operator.lookup_name(),
                Some(&operator.params),
                operator.return_type.is_none(),
                false,
            ),
            ClassMember::Setter(setter) => (setter.name.name.as_str(), None, false, true),
            _ => return false,
        };
        let unresolved = self.override_inference_unresolved(identity, name, setter, model, lookup);
        let inherited = if unresolved {
            None
        } else {
            self.override_inference_signature(identity, name, setter, model, lookup)
        };
        let fallback = if unresolved {
            ResolvedType::Unknown
        } else {
            ResolvedType::Dynamic
        };
        let Some(entry) = self
            .member_entries
            .get_mut(identity)
            .and_then(|entries| entries.get_mut(name))
        else {
            return false;
        };
        let mut changed = false;
        let mut update = |slot: &mut ResolvedType, ty: ResolvedType| {
            if *slot != ty {
                *slot = ty;
                changed = true;
            }
        };
        if setter {
            if let Some(slot) = entry.write.as_mut() {
                let ty = inherited
                    .as_ref()
                    .and_then(|(positional, ..)| positional.first())
                    .unwrap_or(&fallback)
                    .clone();
                update(slot, ty);
            }
            return changed;
        }
        let Some(signature) = entry.signature.as_mut() else {
            return false;
        };
        let untyped = |param: &FormalParam| {
            param.param_type.is_none()
                && param.function_params.is_none()
                && param.initializing.is_none()
        };
        let params = params.expect("method or operator parameters");
        for (index, (param, slot)) in params
            .positional()
            .zip(&mut signature.positional)
            .enumerate()
        {
            if untyped(param) {
                let ty = inherited
                    .as_ref()
                    .and_then(|(positional, ..)| positional.get(index))
                    .unwrap_or(&fallback)
                    .clone();
                update(slot, ty);
            }
        }
        for param in params.named().filter(|param| untyped(param)) {
            if let Some(slot) = signature.named.get_mut(&param.name.name) {
                let ty = inherited
                    .as_ref()
                    .and_then(|(_, named, _)| named.get(&param.name.name))
                    .unwrap_or(&fallback)
                    .clone();
                update(slot, ty);
            }
        }
        if infer_return {
            let ty = inherited
                .map(|(_, _, return_type)| return_type)
                .unwrap_or(fallback);
            update(&mut signature.return_type, ty);
        }
        changed
    }

    fn infer_member_type(
        &mut self,
        identity: &DeclarationIdentity,
        name: &str,
        expression: Option<&Expr>,
        model: &SemanticModel<'_>,
        environment: &TypeEnvironment,
        scope: &TypeParameterScope,
    ) -> bool {
        let Some(entry) = self.member_entry(identity, name) else {
            return false;
        };
        let Some((_, current)) = &entry.field else {
            return false;
        };
        if !has_inference_holes(current) && !matches!(current, ResolvedType::Dynamic) {
            return false;
        }
        let mut inherited = None;
        if entry.facts.iter().any(|fact| !fact.is_static)
            && let Some((_, supertypes)) = self.supertypes.get(identity)
        {
            for supertype in supertypes {
                let Some((declared, substitutions)) = self.resolved_field(supertype, name, model)
                else {
                    continue;
                };
                let ty = model.substitute(&declared, &substitutions);
                // A super member may itself still need an inference pass.
                // Do not freeze an initializer/body type ahead of that pass.
                if has_inference_holes(&ty) {
                    return false;
                }
                if inherited.as_ref().is_some_and(|previous| *previous != ty) {
                    inherited = Some(ResolvedType::Unknown);
                    break;
                }
                inherited = Some(ty);
            }
        }
        let ty = if let Some(ty) = inherited {
            ty
        } else {
            let Some(expression) = expression else {
                // Getter bodies do not participate in declaration inference.
                return false;
            };
            if !self.declaration_dependencies_known(
                expression,
                model,
                Some(identity),
                environment,
                scope,
            ) {
                return false;
            }
            let lookup = |name: &str| {
                self.member_entry(identity, name)
                    .and_then(|entry| entry.field.as_ref().map(|(_, ty)| ty.clone()))
            };
            let ty = environment
                .infer_with_signatures_using_lookup(expression, model, self, scope, &lookup);
            if has_inference_holes(&ty) {
                return false;
            }
            ty
        };
        if *current == ty {
            return false;
        }
        let entry = self
            .member_entries
            .get_mut(identity)
            .unwrap()
            .get_mut(name)
            .unwrap();
        entry.field.as_mut().unwrap().1 = ty.clone();
        if entry
            .read
            .as_ref()
            .is_some_and(|ty| matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic))
        {
            entry.read = Some(ty.clone());
        }
        if entry
            .write
            .as_ref()
            .is_some_and(|ty| matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic))
        {
            entry.write = Some(ty);
        }
        true
    }

    /// An implicit dynamic getter may still inherit its type in a later
    /// pass. Explicit dynamic getters and dynamic fields are already complete.
    fn member_type_pending(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        model: &SemanticModel<'_>,
        visited: &mut FxHashSet<DeclarationIdentity>,
    ) -> bool {
        if !visited.insert(identity.clone()) {
            return false;
        }
        let Some(entry) = self.member_entry(identity, name) else {
            return false;
        };
        let Some((_, ty)) = &entry.field else {
            return false;
        };
        if has_inference_holes(ty) {
            return true;
        }
        if !entry.implicit_getter
            || !matches!(ty, ResolvedType::Dynamic)
            || entry.facts.iter().all(|fact| fact.is_static)
        {
            return false;
        }
        self.supertypes
            .get(identity)
            .is_some_and(|(_, supertypes)| {
                supertypes.iter().any(|supertype| {
                    let Some(ResolvedType::Interface { identity, .. }) =
                        self.resolved_instance_owner(supertype, name, InstanceAccess::Read, model)
                    else {
                        return false;
                    };
                    self.member_entry(&identity, name)
                        .and_then(|entry| entry.field.as_ref())
                        .is_some_and(|(_, ty)| !matches!(ty, ResolvedType::Dynamic))
                        || self.member_type_pending(&identity, name, model, visited)
                })
            })
    }

    fn declaration_dependencies_known(
        &self,
        expression: &Expr,
        model: &SemanticModel<'_>,
        owner: Option<&DeclarationIdentity>,
        environment: &TypeEnvironment,
        scope: &TypeParameterScope,
    ) -> bool {
        self.declaration_dependencies_known_using_lookup(
            expression,
            model,
            owner,
            environment,
            scope,
            &|name| owner.and_then(|owner| self.field(owner, name).map(|(_, ty)| ty.clone())),
        )
    }

    fn declaration_dependencies_known_using_lookup(
        &self,
        expression: &Expr,
        model: &SemanticModel<'_>,
        owner: Option<&DeclarationIdentity>,
        environment: &TypeEnvironment,
        scope: &TypeParameterScope,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> bool {
        let mut unresolved = false;
        // AST addresses are stable for this traversal only. Each field result
        // is retained once, so a.b.c does not infer a.b again for c.
        let mut receivers: FxHashMap<*const Expr, ResolvedType> = FxHashMap::default();
        let mut member_receivers: FxHashMap<*const Expr, ResolvedType> = FxHashMap::default();
        let mut visited = FxHashSet::default();
        InitializerDependencies {
            environment: environment.clone(),
            inspect: |expression: &Expr, environment: &TypeEnvironment| {
                if unresolved {
                    return;
                }
                if let Some(name) = initializer_value_name(expression)
                    && !environment.is_bound(name.prefix.unwrap_or(name.name))
                {
                    let member_owner = match name.prefix {
                        None => owner.cloned(),
                        Some(prefix) => model.resolve_name(prefix),
                    };
                    let member = member_owner.as_ref().and_then(|owner| {
                        self.member_entry(owner, name.name)
                            .and_then(|entry| entry.field.as_ref().map(|(_, ty)| ty))
                    });
                    let variable = model.resolve_value(name).filter(|identity| {
                        // A cycle's Unknown is terminal, not work waiting for
                        // another pass. Its acyclic consumers can still infer.
                        !self.is_cyclic_value(identity)
                    });
                    unresolved |= member
                        .or_else(|| {
                            variable
                                .as_ref()
                                .and_then(|identity| self.value_type(identity))
                        })
                        .is_some_and(has_inference_holes);
                    if let Some(owner) = member_owner {
                        visited.clear();
                        unresolved |=
                            self.member_type_pending(&owner, name.name, model, &mut visited);
                    }
                }
                if let Expr::Field {
                    object,
                    field,
                    is_null_safe,
                    ..
                } = expression
                {
                    let static_owner = initializer_value_name(object)
                        .filter(|name| !environment.is_bound(name.prefix.unwrap_or(name.name)))
                        .and_then(|name| model.resolve_name(name));
                    if let Some(identity) = &static_owner {
                        visited.clear();
                        unresolved |=
                            self.member_type_pending(identity, &field.name, model, &mut visited);
                    }
                    let receiver = receivers
                        .remove(&(object.as_ref() as *const Expr))
                        .unwrap_or_else(|| {
                            environment.receiver_type(
                                object,
                                &field.name,
                                &InferenceContext {
                                    model,
                                    signatures: self,
                                    type_parameters: scope,
                                    lookup,
                                    argument_fallback: None,
                                },
                                None,
                            )
                        });
                    // Cache reached-branch types separately from the null
                    // introduced by shorting, retaining declared nullability.
                    let receiver = if *is_null_safe {
                        receiver.non_null()
                    } else {
                        receiver
                    };
                    if let Some(ResolvedType::Interface { identity, .. }) = self
                        .resolved_instance_owner(
                            &receiver,
                            &field.name,
                            InstanceAccess::Read,
                            model,
                        )
                    {
                        visited.clear();
                        unresolved |=
                            self.member_type_pending(&identity, &field.name, model, &mut visited);
                    }
                    let ty = static_owner
                        .as_ref()
                        .and_then(|identity| self.field(identity, &field.name))
                        .map(|(_, ty)| ty.clone())
                        .or_else(|| {
                            self.resolved_field(&receiver, &field.name, model)
                                .or_else(|| {
                                    self.resolved_extension_getter(&receiver, &field.name, model)
                                })
                                .map(|(ty, substitutions)| model.substitute(&ty, &substitutions))
                        })
                        .unwrap_or(ResolvedType::Unknown);
                    receivers.insert(expression as *const Expr, ty);
                    member_receivers.insert(expression as *const Expr, receiver);
                } else if let Expr::Call { callee, .. } = expression {
                    let target = match callee.as_ref() {
                        Expr::GenericInstantiation { target, .. } => target.as_ref(),
                        target => target,
                    };
                    if let Some(receiver) = member_receivers.remove(&(target as *const Expr)) {
                        let ty = environment.infer_unshorted(
                            expression,
                            InferenceContext {
                                model,
                                signatures: self,
                                type_parameters: scope,
                                lookup,
                                argument_fallback: None,
                            },
                            None,
                            Some(receiver),
                        );
                        receivers.insert(expression as *const Expr, ty);
                    }
                }
            },
        }
        .visit_expr(expression);
        !unresolved
    }

    /// Block promotion of private getters that a concrete `noSuchMethod`
    /// forwards. Each lookup views members from the library that owns the
    /// declaration or private name, never from an arbitrary indexed file.
    fn mark_field_forwarders(&mut self, identities: &IdentityIndex, types: Option<&TypeIndex>) {
        self.has_field_forwarders = self
            .external
            .as_ref()
            .is_some_and(|base| base.has_field_forwarders)
            || self.member_entries.local.iter().any(|(owner, entries)| {
                !owner.is_sdk("dart:core", "Object")
                    && entries.get("noSuchMethod").is_some_and(|entry| {
                        entry
                            .facts
                            .iter()
                            .any(|fact| !fact.is_static && !fact.is_abstract)
                    })
            });
        if !self.has_field_forwarders {
            return;
        }
        let mut blockers = HashSet::new();
        for (identity, declaration) in self.declarations.local.iter() {
            if declaration.is_abstract {
                continue;
            }
            let receiver = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: identity.clone(),
                arguments: empty_type_arguments(),
                nullable: false,
                extension_type: false,
            };
            let declaring = SemanticModel::in_library(
                identities,
                types,
                identities.declaration_library(identity),
            );
            let Some(ResolvedType::Interface {
                identity: owner, ..
            }) = self.resolved_owner_through(
                &receiver,
                "noSuchMethod",
                InstanceAccess::Invoke,
                false,
                &declaring,
            )
            else {
                continue;
            };
            if owner.is_sdk("dart:core", "Object")
                || !self
                    .member_entry(&owner, "noSuchMethod")
                    .is_some_and(|entry| {
                        entry
                            .facts
                            .iter()
                            .any(|fact| !fact.is_static && !fact.is_abstract)
                    })
            {
                continue;
            }
            let mut getters = HashSet::new();
            self.abstract_private_getters(identity, &mut HashSet::new(), &mut getters);
            for (name, library) in getters {
                if self
                    .blocked_field_promotions
                    .get(&library)
                    .is_some_and(|names| names.contains(name))
                {
                    continue;
                }
                let private_scope = SemanticModel::in_library(identities, types, Some(library));
                let implemented = self.resolved_owner_through(&receiver, name, InstanceAccess::Read, false, &private_scope)
                    .and_then(|owner| match owner {
                        ResolvedType::Interface { identity: owner, .. } => {
                            let same_library = matches!(&owner, DeclarationIdentity::Project { library: owner_library, .. } if *owner_library == library);
                            Some(same_library && self.member_entry(&owner, name).is_some_and(|entry|
                                entry.facts.iter().any(|fact| !fact.is_static && !fact.is_abstract)))
                        }
                        _ => None,
                    }).unwrap_or(false);
                if !implemented {
                    blockers.insert((library, name));
                }
            }
        }
        let blockers: Vec<_> = blockers
            .into_iter()
            .map(|(library, name)| (library, name.to_owned()))
            .collect();
        for (library, name) in blockers {
            self.blocked_field_promotions
                .entry(library)
                .or_default()
                .insert(name);
        }
    }

    fn abstract_private_getters<'a>(
        &'a self,
        identity: &DeclarationIdentity,
        visited: &mut HashSet<DeclarationIdentity>,
        getters: &mut HashSet<(&'a str, LibraryId)>,
    ) {
        if !visited.insert(identity.clone()) {
            return;
        }
        if let DeclarationIdentity::Project { library, .. } = identity
            && let Some(entries) = self.member_entries.get(identity)
        {
            for (name, entry) in entries {
                if name.starts_with('_')
                    && entry.facts.iter().any(|fact| {
                        !fact.is_static && fact.is_abstract && fact.kind == MemberKind::Getter
                    })
                {
                    getters.insert((name.as_str(), *library));
                }
            }
        }
        if let Some((_, supertypes)) = self.supertypes.get(identity) {
            for supertype in supertypes {
                if let ResolvedType::Interface { identity, .. } = supertype {
                    self.abstract_private_getters(identity, visited, getters);
                }
            }
        }
    }

    fn add_declarations(&mut self, program: &Program, model: &SemanticModel<'_>) {
        self.flow_writes
            .entry(model.file)
            .or_insert_with(|| Arc::new(super::flow_writes::FlowWrites::program(program)));
        for declaration in &program.declarations {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let kind = match declaration {
                TopLevelDecl::Class(class) => Some((
                    &class.name,
                    TypeKind::Class,
                    class.extends.is_none(),
                    class.modifiers.restriction,
                )),
                TopLevelDecl::ClassTypeAlias(alias) => Some((
                    &alias.name,
                    TypeKind::Class,
                    false,
                    alias.modifiers.restriction,
                )),
                TopLevelDecl::MixinClass(class) => Some((
                    &class.name,
                    TypeKind::MixinClass,
                    class.extends.is_none(),
                    class.is_base.then_some(ClassRestriction::Base),
                )),
                TopLevelDecl::Mixin(mixin) => Some((
                    &mixin.name,
                    TypeKind::Mixin,
                    mixin.on_clause.is_empty(),
                    mixin.is_base.then_some(ClassRestriction::Base),
                )),
                TopLevelDecl::Enum(enumeration) => Some((
                    &enumeration.name,
                    TypeKind::Enum,
                    !model.identities.has_sdk_sources(),
                    None,
                )),
                TopLevelDecl::ExtensionType(extension) => {
                    Some((&extension.name, TypeKind::ExtensionType, false, None))
                }
                _ => None,
            };
            if let Some((name, kind, inherited_object, restriction)) = kind
                && let Some(identity) = model.resolve_name(&name.name)
            {
                self.inherited_object
                    .insert(identity.clone(), inherited_object);
                if let Some(restriction) = restriction {
                    self.restrictions.insert(identity.clone(), restriction);
                }
                self.kinds.insert(identity, kind);
            }
            match declaration {
                TopLevelDecl::ClassTypeAlias(alias) => {
                    self.add_declaration_facts(
                        &alias.name.name,
                        &alias.annotations,
                        &alias.type_params,
                        &EffectiveMembers::declared(&[]),
                        (None, alias.modifiers.is_abstract),
                        model,
                    );
                    self.add_type(
                        &alias.name.name,
                        &alias.type_params,
                        std::iter::once(&alias.superclass)
                            .chain(&alias.with_clause)
                            .chain(&alias.implements)
                            .collect(),
                        alias
                            .with_clause
                            .iter()
                            .rev()
                            .chain(std::iter::once(&alias.superclass))
                            .collect(),
                        &EffectiveMembers::declared(&[]),
                        model,
                    );
                    if let Some(identity) = model.resolve_name(&alias.name.name) {
                        let mut scope = TypeParameterScope::default();
                        scope.push(&alias.type_params, model);
                        self.superclasses.insert(
                            identity.clone(),
                            model.resolve_type_in(&alias.superclass, &scope),
                        );
                        self.mixin_applications
                            .insert(identity, 1..1 + alias.with_clause.len());
                    }
                }
                // Alias parameters are the inference variables of alias
                // constructor calls and tear-offs (`alias_template`).
                TopLevelDecl::TypeAlias(alias) => {
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(&alias.type_params, model);
                    record_bound_declarations(
                        &mut self.bound_declared,
                        &parameters,
                        &alias.type_params,
                    );
                    self.add_declaration_facts(
                        &alias.name.name,
                        &alias.annotations,
                        &alias.type_params,
                        &EffectiveMembers::declared(&[]),
                        (None, false),
                        model,
                    );
                }
                TopLevelDecl::Function(function)
                    if matches!(function.kind, FunctionKind::Function(_)) =>
                {
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(&function.type_params, model);
                    record_bound_declarations(
                        &mut self.bound_declared,
                        &parameters,
                        &function.type_params,
                    );
                    if let Some(identity) = model.resolve_value(&function.name.name) {
                        let annotations = resolve_annotations(&function.annotations, model);
                        if !annotations.is_empty() {
                            self.function_annotations
                                .insert(identity.clone(), annotations);
                        }
                        self.functions.insert(
                            identity,
                            signature(
                                &parameters,
                                &[],
                                &function.type_params,
                                function.params(),
                                function.return_type.as_ref(),
                                None,
                                model,
                            ),
                        );
                    }
                }
                TopLevelDecl::Function(function)
                    if matches!(function.kind, FunctionKind::Getter) =>
                {
                    if let Some(identity) = model.resolve_value(&function.name.name) {
                        self.value_getters.insert(identity.clone());
                        self.value_types.insert(
                            identity,
                            function
                                .return_type
                                .as_ref()
                                .map(|ty| model.resolve_type(ty))
                                .unwrap_or(ResolvedType::Dynamic),
                        );
                    }
                }
                TopLevelDecl::Variable(variable) => {
                    for declarator in &variable.declarators {
                        if !model.owns_declaration(&declarator.name.name) {
                            continue;
                        }
                        let Some(identity) = model.resolve_value(&declarator.name.name) else {
                            continue;
                        };
                        self.value_types.insert(
                            identity.clone(),
                            variable
                                .var_type
                                .as_ref()
                                .map(|ty| model.resolve_type(ty))
                                .unwrap_or_else(|| {
                                    if declarator.initializer.is_none() {
                                        ResolvedType::Dynamic
                                    } else {
                                        ResolvedType::Unknown
                                    }
                                }),
                        );
                        if variable.keyword == VarKeyword::Const
                            && let Some(initializer) = &declarator.initializer
                        {
                            if let Some(constant_type) = constructed_identity(initializer, model) {
                                self.constant_types.insert(identity.clone(), constant_type);
                            }
                            self.constants.insert(identity, initializer.clone());
                        }
                    }
                }
                TopLevelDecl::Class(class) => {
                    let members = class.element_members();
                    let supertypes = class
                        .extends
                        .iter()
                        .chain(&class.with_clause)
                        .chain(&class.implements)
                        .collect();
                    let member_supertypes = class
                        .with_clause
                        .iter()
                        .rev()
                        .chain(&class.extends)
                        .collect();
                    self.add_type(
                        &class.name.name,
                        &class.type_params,
                        supertypes,
                        member_supertypes,
                        &members,
                        model,
                    );
                    if !class.with_clause.is_empty()
                        && let Some(identity) = model.resolve_name(&class.name.name)
                    {
                        let start = usize::from(class.extends.is_some());
                        self.mixin_applications
                            .insert(identity, start..start + class.with_clause.len());
                    }
                    self.add_declaration_facts(
                        &class.name.name,
                        &class.annotations,
                        &class.type_params,
                        &members,
                        (None, class.modifiers.is_abstract),
                        model,
                    );
                }
                TopLevelDecl::MixinClass(class) => {
                    let members = class.element_members();
                    let supertypes = class
                        .extends
                        .iter()
                        .chain(&class.with_clause)
                        .chain(&class.implements)
                        .collect();
                    let member_supertypes = class
                        .with_clause
                        .iter()
                        .rev()
                        .chain(&class.extends)
                        .collect();
                    self.add_type(
                        &class.name.name,
                        &class.type_params,
                        supertypes,
                        member_supertypes,
                        &members,
                        model,
                    );
                    self.add_declaration_facts(
                        &class.name.name,
                        &class.annotations,
                        &class.type_params,
                        &members,
                        (None, class.is_abstract),
                        model,
                    );
                }
                TopLevelDecl::Extension(extension) => {
                    if let Some(name) = &extension.name {
                        self.add_type(
                            &name.name,
                            &extension.type_params,
                            Vec::new(),
                            Vec::new(),
                            &EffectiveMembers::declared(&extension.members),
                            model,
                        );
                    }
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(&extension.type_params, model);
                    let owner_parameters = parameter_ids(&parameters, &extension.type_params);
                    let methods = extension
                        .members
                        .iter()
                        .filter_map(|member| match member {
                            ClassMember::Method(method) if !method.is_static => {
                                parameters.push(&method.type_params, model);
                                record_bound_declarations(
                                    &mut self.bound_declared,
                                    &parameters,
                                    &method.type_params,
                                );
                                let resolved = signature(
                                    &parameters,
                                    &owner_parameters,
                                    &method.type_params,
                                    Some(&method.params),
                                    method.return_type.as_ref(),
                                    None,
                                    model,
                                );
                                parameters.pop();
                                Some((method.name.name.clone(), resolved))
                            }
                            ClassMember::Operator(operator) => Some((
                                operator.lookup_name().to_owned(),
                                signature(
                                    &parameters,
                                    &owner_parameters,
                                    &[],
                                    Some(&operator.params),
                                    operator.return_type.as_ref(),
                                    None,
                                    model,
                                ),
                            )),
                            _ => None,
                        })
                        .collect();
                    let getters = extension
                        .members
                        .iter()
                        .filter_map(|member| match member {
                            ClassMember::Getter(getter) if !getter.is_static => Some((
                                getter.name.name.clone(),
                                getter
                                    .return_type
                                    .as_ref()
                                    .map_or(ResolvedType::Dynamic, |ty| {
                                        model.resolve_type_in(ty, &parameters)
                                    }),
                            )),
                            _ => None,
                        })
                        .collect();
                    self.extensions.push(ExtensionFacts {
                        declaring_library: model.library,
                        name: extension.name.as_ref().map(|name| name.name.clone()),
                        identity: extension
                            .name
                            .as_ref()
                            .and_then(|name| model.resolve_extension(&name.name)),
                        on_type: model.resolve_type_in(&extension.on_type, &parameters),
                        parameters: owner_parameters,
                        parameter_bounds: extension
                            .type_params
                            .iter()
                            .filter_map(|parameter| {
                                let ResolvedType::TypeParameter { id, bound, .. } =
                                    parameters.lookup(&parameter.name.name)?
                                else {
                                    return None;
                                };
                                Some((
                                    id,
                                    if parameter.bound.is_some() {
                                        bound.as_ref().clone()
                                    } else {
                                        ResolvedType::Dynamic
                                    },
                                ))
                            })
                            .collect(),
                        methods,
                        annotations: extension
                            .members
                            .iter()
                            .filter_map(|member| {
                                let (name, annotations) = match member {
                                    ClassMember::Method(method) => {
                                        (method.name.name.as_str(), &method.annotations)
                                    }
                                    ClassMember::Getter(getter) => {
                                        (getter.name.name.as_str(), &getter.annotations)
                                    }
                                    ClassMember::Operator(operator) => {
                                        (operator.lookup_name(), &operator.annotations)
                                    }
                                    _ => return None,
                                };
                                let annotations = resolve_annotations(annotations, model);
                                (!annotations.is_empty()).then(|| (name.to_owned(), annotations))
                            })
                            .collect(),
                        getters,
                        members: extension
                            .members
                            .iter()
                            .filter_map(|member| match member {
                                ClassMember::Method(method) => Some(method.name.name.clone()),
                                _ => None,
                            })
                            .collect(),
                        instance_members: extension
                            .members
                            .iter()
                            .filter_map(|member| match member {
                                ClassMember::Method(method) if !method.is_static => {
                                    Some(method.name.name.clone())
                                }
                                ClassMember::Getter(getter) if !getter.is_static => {
                                    Some(getter.name.name.clone())
                                }
                                ClassMember::Setter(setter) if !setter.is_static => {
                                    Some(format!("{}=", setter.name.name))
                                }
                                ClassMember::Operator(operator) => {
                                    Some(operator.lookup_name().to_owned())
                                }
                                _ => None,
                            })
                            .collect(),
                    });
                }
                TopLevelDecl::ExtensionType(extension) => {
                    let members = extension.element_members();
                    self.add_type(
                        &extension.name.name,
                        &extension.type_params,
                        extension.implements.iter().collect(),
                        Vec::new(),
                        &members,
                        model,
                    );
                    self.add_declaration_facts(
                        &extension.name.name,
                        &extension.annotations,
                        &extension.type_params,
                        &members,
                        (
                            extension
                                .representation()
                                .and_then(|param| param.param_type.as_ref()),
                            false,
                        ),
                        model,
                    );
                }
                TopLevelDecl::Mixin(mixin) => {
                    let supertypes = mixin.on_clause.iter().chain(&mixin.implements).collect();
                    self.add_type(
                        &mixin.name.name,
                        &mixin.type_params,
                        supertypes,
                        Vec::new(),
                        &EffectiveMembers::declared(&mixin.members),
                        model,
                    );
                    if let Some(identity) = model.resolve_name(&mixin.name.name) {
                        self.mixin_constraint_counts
                            .insert(identity, mixin.on_clause.len());
                    }
                    // Mixins are never instantiated: abstract for every consumer.
                    self.add_declaration_facts(
                        &mixin.name.name,
                        &mixin.annotations,
                        &mixin.type_params,
                        &EffectiveMembers::declared(&mixin.members),
                        (None, true),
                        model,
                    );
                }
                TopLevelDecl::Enum(enumeration) => {
                    let members = enumeration.element_members();
                    self.add_type(
                        &enumeration.name.name,
                        &enumeration.type_params,
                        enumeration
                            .with_clause
                            .iter()
                            .chain(&enumeration.implements)
                            .collect(),
                        enumeration.with_clause.iter().rev().collect(),
                        &members,
                        model,
                    );
                    self.add_declaration_facts(
                        &enumeration.name.name,
                        &enumeration.annotations,
                        &enumeration.type_params,
                        &members,
                        (None, false),
                        model,
                    );
                    // Every enum implicitly extends `Enum`; only meaningful
                    // when `dart:core` sources describe it.
                    if model.identities.has_sdk_sources()
                        && let Some(identity) = model.resolve_name(&enumeration.name.name)
                    {
                        let enum_type = model.core_type("Enum", false);
                        if let Some((_, supertypes)) = self.supertypes.get_mut(&identity) {
                            // Inherited interfaces start with Enum, then apply
                            // mixins in source order and finally interfaces.
                            supertypes.insert(0, enum_type.clone());
                        }
                        if let Some(supertypes) = self.member_supertypes.get_mut(&identity) {
                            // Concrete lookup searches mixins in reverse order
                            // before the superclass.
                            let object = supertypes.len().saturating_sub(1);
                            supertypes.insert(object, enum_type);
                        }
                    }
                    if let Some(identity) = model.resolve_name(&enumeration.name.name) {
                        if !enumeration.with_clause.is_empty() {
                            let start = usize::from(model.identities.has_sdk_sources());
                            self.mixin_applications.insert(
                                identity.clone(),
                                start..start + enumeration.with_clause.len(),
                            );
                        }
                        // The language synthesizes a static const list, including
                        // the enum's instantiation to bounds for generic enums.
                        let enum_type = self.with_raw_arguments(&ResolvedType::interface_identity(
                            identity.clone(),
                        ));
                        let values_type = ResolvedType::Interface {
                            occurrence: Default::default(),
                            identity: DeclarationIdentity::sdk("dart:core", "List"),
                            arguments: vec![enum_type].into(),
                            nullable: false,
                            extension_type: false,
                        };
                        let entry = self
                            .member_entries
                            .entry(identity.clone())
                            .or_default()
                            .entry("values".to_string())
                            .or_default();
                        entry.field = Some((Vec::new(), values_type));
                        entry.facts.push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind: MemberKind::Field {
                                writable: false,
                                covariant: false,
                            },
                            is_static: true,
                            is_abstract: false,
                            positional_parameter_names: Vec::new(),
                            parameter_contracts: Some(empty_parameter_contracts()),
                            annotations: Vec::new(),
                        });
                        self.enum_values.insert(
                            identity,
                            enumeration
                                .variants
                                .iter()
                                .map(|variant| variant.name.name.clone())
                                .collect(),
                        );
                    }
                }
                _ => {}
            }
        }
        self.add_immutability(program, model);
    }

    fn add_immutability(&mut self, program: &Program, model: &SemanticModel<'_>) {
        for declaration in &program.declarations {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let (name, annotations, superclass) = match declaration {
                TopLevelDecl::Class(node) => (&node.name, &node.annotations, node.extends.as_ref()),
                TopLevelDecl::MixinClass(node) => {
                    (&node.name, &node.annotations, node.extends.as_ref())
                }
                TopLevelDecl::ClassTypeAlias(node) => {
                    // Its superclass was already resolved in the alias's
                    // generic scope while indexing the mixin application.
                    (&node.name, &node.annotations, None)
                }
                TopLevelDecl::Mixin(node) => (&node.name, &node.annotations, None),
                TopLevelDecl::Enum(node) => (&node.name, &node.annotations, None),
                TopLevelDecl::ExtensionType(node) => (&node.name, &node.annotations, None),
                _ => continue,
            };
            let Some(identity) = model.resolve_name(&*name.name) else {
                continue;
            };
            let immutable = if annotations.iter().any(|annotation| {
                QualifiedName::from_identifiers(&annotation.name)
                    .and_then(|name| {
                        model
                            .resolve_value(name)
                            .or_else(|| model.resolve_imported_member(name))
                    })
                    .is_some_and(|identity| identity.is_meta_immutable())
            }) {
                Truth::Yes
            } else if annotations.iter().any(|annotation| {
                // An `immutable` annotation that resolves to nothing may come
                // through an export chain falcon does not follow.
                annotation
                    .name
                    .last()
                    .is_some_and(|last| matches!(&*last.name, "immutable" | "Immutable"))
                    && QualifiedName::from_identifiers(&annotation.name)
                        .and_then(|name| {
                            model
                                .resolve_value(name)
                                .or_else(|| model.resolve_name(name))
                        })
                        .is_none()
            }) {
                Truth::Unknown
            } else {
                Truth::No
            };
            self.immutable.insert(identity.clone(), immutable);
            if let Some(superclass) = superclass {
                let mut scope = TypeParameterScope::default();
                match declaration {
                    TopLevelDecl::Class(node) => scope.push(&node.type_params, model),
                    TopLevelDecl::MixinClass(node) => scope.push(&node.type_params, model),
                    _ => {}
                }
                self.superclasses
                    .insert(identity, model.resolve_type_in(superclass, &scope));
            }
        }
    }

    fn resolve_constructor_formals(
        &mut self,
        program: &Program,
        model: &SemanticModel<'_>,
    ) -> bool {
        let mut resolved = false;
        for _ in 0..program.declarations.len() {
            let mut changed = false;
            for declaration in &program.declarations {
                if !model.owns_top_level_declaration(declaration) {
                    continue;
                }
                if let TopLevelDecl::ClassTypeAlias(alias) = declaration {
                    changed |= self.forward_alias_constructors(&alias.name.name, model);
                    continue;
                }
                let (name, type_parameters, superclass, members) = match declaration {
                    TopLevelDecl::Class(class) => (
                        &class.name.name,
                        &class.type_params,
                        class.extends.as_ref(),
                        class.element_members(),
                    ),
                    TopLevelDecl::MixinClass(class) => (
                        &class.name.name,
                        &class.type_params,
                        class.extends.as_ref(),
                        class.element_members(),
                    ),
                    TopLevelDecl::Enum(enumeration) => (
                        &enumeration.name.name,
                        &enumeration.type_params,
                        None,
                        EffectiveMembers::declared(&enumeration.members),
                    ),
                    TopLevelDecl::ExtensionType(extension) => (
                        &extension.name.name,
                        &extension.type_params,
                        None,
                        extension.element_members(),
                    ),
                    _ => continue,
                };
                if !members.constructors().any(|constructor| {
                    constructor
                        .params
                        .params
                        .iter()
                        .any(|parameter| parameter.initializing.is_some())
                }) {
                    continue;
                }
                let Some(identity) = model.resolve_name(name) else {
                    continue;
                };
                let mut scope = TypeParameterScope::default();
                scope.push(type_parameters, model);
                let superclass = superclass.map(|ty| model.resolve_type_in(ty, &scope));
                for constructor in members.constructors() {
                    let constructor_name = constructor
                        .constructor_name
                        .as_ref()
                        .map(|name| name.name.as_str())
                        .unwrap_or("new");
                    for parameter in &constructor.params.params {
                        if parameter.param_type.is_some()
                            || parameter.initializing != Some(InitializingFormal::This)
                        {
                            continue;
                        }
                        let Some(ty) = self
                            .member_entry(&identity, &parameter.name.name)
                            .filter(|entry| {
                                entry.facts.iter().any(|fact| {
                                    matches!(fact.kind, MemberKind::Field { .. }) && !fact.is_static
                                })
                            })
                            .and_then(|entry| entry.field.as_ref())
                            .map(|(_, ty)| ty.clone())
                        else {
                            continue;
                        };
                        changed |= self.update_constructor_parameter_type(
                            &identity,
                            constructor_name,
                            constructor.params,
                            parameter,
                            ty,
                        );
                    }
                    let Some(superclass) = &superclass else {
                        continue;
                    };
                    let super_constructor = constructor
                        .initializers
                        .iter()
                        .find_map(|initializer| match initializer {
                            falcon_syntax::ast::ConstructorInitializer::SuperCall {
                                call_name,
                                ..
                            } => Some(
                                call_name
                                    .as_ref()
                                    .map(|name| name.name.as_str())
                                    .unwrap_or("new"),
                            ),
                            _ => None,
                        })
                        .unwrap_or("new");
                    let positional_super = constructor.params.positional().filter(|parameter| {
                        parameter.initializing == Some(InitializingFormal::Super)
                    });
                    let named_super = constructor.params.named().filter(|parameter| {
                        parameter.initializing == Some(InitializingFormal::Super)
                    });
                    let super_formals = positional_super
                        .enumerate()
                        .map(|(ordinal, parameter)| (SuperFormal::Positional(ordinal), parameter))
                        .chain(named_super.map(|parameter| {
                            (SuperFormal::Named(&parameter.name.name), parameter)
                        }));
                    for (formal, parameter) in super_formals {
                        if parameter.default_value.is_none()
                            && matches!(
                                parameter.kind,
                                ParamKind::OptionalPositional
                                    | ParamKind::Named { required: false }
                            )
                        {
                            changed |= self.inherit_super_default(
                                (&identity, constructor_name),
                                &parameter.name.name,
                                (superclass, super_constructor),
                                formal,
                            );
                        }
                        if parameter.param_type.is_some() {
                            continue;
                        }
                        let Some(ty) = self.resolved_constructor_parameter_type(
                            superclass,
                            super_constructor,
                            formal,
                            model,
                        ) else {
                            continue;
                        };
                        changed |= self.update_constructor_parameter_type(
                            &identity,
                            constructor_name,
                            constructor.params,
                            parameter,
                            ty,
                        );
                    }
                }
            }
            resolved |= changed;
            if !changed {
                break;
            }
        }
        resolved
    }

    /// Materialize mixin-application constructors in the alias's parameter
    /// scope. Run alongside super-formal refinement so chained applications
    /// see both inferred parameter types and defaults from the declaring library.
    fn forward_alias_constructors(&mut self, name: &str, model: &SemanticModel<'_>) -> bool {
        let Some(identity) = model.resolve_name(name) else {
            return false;
        };
        let Some(ResolvedType::Interface {
            identity: superclass,
            arguments,
            ..
        }) = self.superclasses.get(&identity)
        else {
            return false;
        };
        let Some(source_facts) = self.declarations.get(superclass) else {
            return false;
        };
        let Some(owner_parameters) = self.type_parameters(&identity) else {
            return false;
        };
        let mixins_have_no_fields = self
            .mixin_applications
            .get(&identity)
            .and_then(|range| self.supertypes.get(&identity)?.1.get(range.clone()))
            .is_some_and(|mixins| {
                mixins.iter().all(|mixin| {
                    let ResolvedType::Interface { identity, .. } = mixin else {
                        return false;
                    };
                    self.kinds.contains_key(identity)
                        && self.member_entries.get(identity).is_some_and(|entries| {
                            entries.values().all(|entry| {
                                entry.facts.iter().all(|fact| {
                                    fact.is_static || !matches!(fact.kind, MemberKind::Field { .. })
                                })
                            })
                        })
                })
            });
        let owner_parameters = owner_parameters.to_vec();
        let mut forwarded = Vec::with_capacity(source_facts.constructors.len());
        for facts in &source_facts.constructors {
            if facts.is_factory || (facts.is_private && !same_library(superclass, &identity)) {
                continue;
            }
            let Some(source) = self.constructor(superclass, &facts.name) else {
                continue;
            };
            let is_const = facts.is_const && mixins_have_no_fields;
            let substitutions = self.owner_substitutions(&source.owner_parameters, arguments, None);
            let positional = source
                .positional
                .iter()
                .map(|ty| model.substitute(ty, &substitutions))
                .collect::<Vec<_>>();
            let named = source
                .named
                .iter()
                .map(|(name, ty)| (name.clone(), model.substitute(ty, &substitutions)))
                .collect::<HashMap<_, _>>();
            if self
                .constructor(&identity, &facts.name)
                .is_some_and(|existing| {
                    existing.positional == positional
                        && existing.named == named
                        && existing.defaults.len() == source.defaults.len()
                        && self.declaration(&identity).is_some_and(|declaration| {
                            declaration.constructors.iter().any(|existing| {
                                existing.name == facts.name
                                    && existing.is_const == is_const
                                    && existing.annotations.is_empty()
                            })
                        })
                })
            {
                continue;
            }
            let mut signature = source.clone();
            signature.owner_parameters = owner_parameters.clone();
            signature.positional = positional;
            signature.named = named;
            for default in signature.defaults.values_mut() {
                if !matches!(default, ParameterDefault::Inherited { .. }) {
                    *default = ParameterDefault::Inherited {
                        owner: superclass.clone(),
                        default: Box::new(default.clone()),
                    };
                }
            }
            let mut facts = facts.clone();
            // These are generated declarations, not annotated declarations
            // inherited from the superclass (notably package:meta's `literal`).
            facts.annotations.clear();
            facts.is_const = is_const;
            for parameter in &mut facts.parameters {
                parameter.ty = model.substitute(&parameter.ty, &substitutions);
                parameter.is_super = false;
            }
            forwarded.push((facts, signature));
        }
        let changed = !forwarded.is_empty();
        for (facts, signature) in forwarded {
            self.constructors
                .entry(identity.clone())
                .or_default()
                .insert(facts.name.clone(), signature);
            if let Some(declaration) = self.declarations.get_mut(&identity) {
                if let Some(existing) = declaration
                    .constructors
                    .iter_mut()
                    .find(|existing| existing.name == facts.name)
                {
                    *existing = facts;
                } else {
                    declaration.constructors.push(facts);
                }
            }
        }
        changed
    }

    /// Give a super formal without a declared default the default of the
    /// super-constructor parameter it forwards to, if that is known.
    fn inherit_super_default(
        &mut self,
        (identity, constructor_name): (&DeclarationIdentity, &str),
        name: &str,
        (superclass, super_constructor): (&ResolvedType, &str),
        parameter: SuperFormal<'_>,
    ) -> bool {
        let ResolvedType::Interface {
            identity: super_identity,
            ..
        } = superclass
        else {
            return false;
        };
        let Some(super_signature) = self.constructor(super_identity, super_constructor) else {
            return false;
        };
        let super_name = match parameter {
            SuperFormal::Named(name) => Some(name),
            SuperFormal::Positional(ordinal) => super_signature
                .positional_parameter_names
                .get(ordinal)
                .map(String::as_str),
        };
        let Some(default) = super_name.and_then(|name| super_signature.defaults.get(name)) else {
            return false;
        };
        let inherited = match default {
            ParameterDefault::Inherited { .. } => default.clone(),
            _ => ParameterDefault::Inherited {
                owner: super_identity.clone(),
                default: Box::new(default.clone()),
            },
        };
        let Some(signature) = self
            .constructors
            .get_mut(identity)
            .and_then(|constructors| constructors.get_mut(constructor_name))
        else {
            return false;
        };
        if signature.defaults.contains_key(name) {
            return false;
        }
        signature.defaults.insert(name.to_string(), inherited);
        true
    }

    /// The type of the super-constructor parameter a super formal forwards
    /// to: a named super formal matches by name, the `ordinal`-th positional
    /// super formal matches the super constructor's `ordinal`-th positional
    /// parameter (positional super formals require an invocation without
    /// positional arguments).
    fn resolved_constructor_parameter_type(
        &self,
        superclass: &ResolvedType,
        constructor_name: &str,
        parameter: SuperFormal<'_>,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let ResolvedType::Interface {
            occurrence,
            identity,
            arguments,
            ..
        } = superclass
        else {
            return None;
        };
        let signature = self.constructor(identity, constructor_name)?;
        let ty = match parameter {
            SuperFormal::Named(name) => signature.named.get(name)?,
            SuperFormal::Positional(ordinal) => signature.positional.get(ordinal)?,
        };
        let substitutions =
            self.owner_substitutions(&signature.owner_parameters, arguments, Some(occurrence));
        Some(model.substitute(ty, &substitutions))
    }

    fn update_constructor_parameter_type(
        &mut self,
        identity: &DeclarationIdentity,
        constructor_name: &str,
        params: &FormalParamList,
        parameter: &FormalParam,
        ty: ResolvedType,
    ) -> bool {
        let mut changed = false;
        if let Some(signature) = self
            .constructors
            .get_mut(identity)
            .and_then(|constructors| constructors.get_mut(constructor_name))
        {
            if matches!(parameter.kind, falcon_syntax::ast::ParamKind::Named { .. }) {
                if signature.named.get(&parameter.name.name) != Some(&ty) {
                    signature
                        .named
                        .insert(parameter.name.name.clone(), ty.clone());
                    changed = true;
                }
            } else if let Some(index) = params
                .positional()
                .position(|candidate| candidate.name.span == parameter.name.span)
                && signature.positional.get(index) != Some(&ty)
            {
                signature.positional[index] = ty.clone();
                changed = true;
            }
        }
        if let Some(facts) = self.declarations.get_mut(identity)
            && let Some(constructor) = facts
                .constructors
                .iter_mut()
                .find(|constructor| constructor.name == constructor_name)
            && let Some(parameter) = constructor
                .parameters
                .iter_mut()
                .find(|candidate| candidate.name == parameter.name.name)
            && parameter.ty != ty
        {
            parameter.ty = ty;
            changed = true;
        }
        changed
    }

    fn add_const_constructor_bodies(
        &mut self,
        identity: &DeclarationIdentity,
        members: &EffectiveMembers<'_>,
    ) {
        let bodies: FxHashMap<String, ConstConstructorBody> = members
            .constructors()
            .filter(|constructor| constructor.is_const && !constructor.is_external)
            .map(|constructor| {
                let mut params = constructor.params.params.clone();
                if constructor.is_primary {
                    for parameter in &mut params {
                        if members.representation || parameter.is_var || parameter.is_final {
                            parameter.initializing = Some(InitializingFormal::This);
                        }
                    }
                }
                (
                    constructor
                        .constructor_name
                        .as_ref()
                        .map_or_else(|| "new".to_string(), |name| name.name.clone()),
                    ConstConstructorBody {
                        is_primary: constructor.is_primary,
                        params,
                        initializers: constructor.initializers.to_vec(),
                        redirect: constructor.redirect.cloned(),
                    },
                )
            })
            .collect();
        if bodies.is_empty() {
            return;
        }
        let fields = members
            .declared
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field) if !field.is_static => Some(field),
                _ => None,
            })
            .flat_map(|field| {
                field.declarators.iter().filter_map(|declarator| {
                    Some((
                        declarator.name.name.clone(),
                        declarator.initializer.clone()?,
                    ))
                })
            })
            .collect();
        self.const_field_initializers
            .insert(identity.clone(), fields);
        self.const_constructors.insert(identity.clone(), bodies);
    }

    fn add_declaration_facts(
        &mut self,
        name: &str,
        annotations: &[falcon_syntax::ast::Annotation],
        type_parameters: &[TypeParam],
        members: &EffectiveMembers<'_>,
        declaration_kind: (Option<&DartType>, bool),
        model: &SemanticModel<'_>,
    ) {
        let (extension_representation, is_abstract) = declaration_kind;
        let Some(identity) = model.resolve_name(name) else {
            return;
        };
        self.add_const_constructor_bodies(&identity, members);
        let mut scope = TypeParameterScope::default();
        scope.push(type_parameters, model);
        let library_target = annotations
            .iter()
            .any(|annotation| annotation_targets_library(annotation, model));
        let annotations = resolve_annotations(annotations, model);
        let constructors = members
            .constructors()
            .map(|constructor| ConstructorFacts {
                name: constructor
                    .constructor_name
                    .as_ref()
                    .map(|name| name.name.clone())
                    .unwrap_or_else(|| "new".to_string()),
                annotations: resolve_annotations(constructor.annotations, model),
                is_const: constructor.is_const,
                is_factory: constructor.is_factory,
                is_private: constructor
                    .constructor_name
                    .as_ref()
                    .is_some_and(|name| name.name.starts_with('_')),
                redirect_target: constructor.redirect.as_ref().map(|redirect| {
                    (
                        model.resolve_type(&redirect.type_),
                        redirect
                            .constructor_name
                            .as_ref()
                            .map_or_else(|| "new".to_string(), |name| name.name.clone()),
                    )
                }),
                parameters: constructor
                    .params
                    .params
                    .iter()
                    .map(|parameter| ParameterFacts {
                        name: parameter.name.name.clone(),
                        ty: parameter
                            .param_type
                            .as_ref()
                            .map(|ty| model.resolve_type_in(ty, &scope))
                            .or_else(|| {
                                (parameter.initializing == Some(InitializingFormal::This)).then(
                                    || {
                                        self.member_entry(&identity, &parameter.name.name)
                                            .filter(|entry| {
                                                entry.facts.iter().any(|fact| {
                                                    matches!(fact.kind, MemberKind::Field { .. })
                                                        && !fact.is_static
                                                })
                                            })
                                            .and_then(|entry| entry.field.as_ref())
                                            .map(|(_, ty)| ty.clone())
                                            .unwrap_or(ResolvedType::Unknown)
                                    },
                                )
                            })
                            .unwrap_or(ResolvedType::Unknown),
                        is_named: matches!(
                            parameter.kind,
                            falcon_syntax::ast::ParamKind::Named { .. }
                        ),
                        is_super: parameter.initializing == Some(InitializingFormal::Super),
                    })
                    .collect(),
            })
            .collect();
        let static_consts = members
            .declared
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field)
                    if field.is_static && field.keyword == VarKeyword::Const =>
                {
                    Some(field)
                }
                _ => None,
            })
            .flat_map(|field| {
                let field_scope = scope.clone();
                let declared_ty = field
                    .field_type
                    .as_ref()
                    .map(|ty| model.resolve_type_in(ty, &scope));
                let is_deprecated = field.annotations.iter().any(|annotation| {
                    annotation.name.last().is_some_and(|name| {
                        matches!(name.name.as_str(), "deprecated" | "Deprecated")
                    })
                });
                field.declarators.iter().filter_map(move |declarator| {
                    declarator
                        .initializer
                        .as_ref()
                        .map(|initializer| StaticConstFacts {
                            name: declarator.name.name.clone(),
                            ty: declared_ty.clone().unwrap_or_else(|| {
                                infer_const_initializer_type(initializer, model, &field_scope)
                            }),
                            initializer: initializer.clone(),
                            is_deprecated,
                        })
                })
            })
            .collect();
        self.declarations.insert(
            identity,
            DeclarationFacts {
                annotations,
                library_target,
                constructors,
                static_consts,
                extension_representation: extension_representation
                    .map(|ty| model.resolve_type_in(ty, &scope)),
                is_abstract,
                has_subclass_in_library: false,
            },
        );
    }

    /// The linter's `_hasSubclassInDefiningCompilationUnit`: a class whose
    /// `extends` chain (not `implements`, `with`, or mixin `on` clauses)
    /// reaches a declaration of its own library marks that declaration.
    fn mark_library_subclasses(&mut self) {
        let mut marked = Vec::new();
        let mut visited = HashSet::new();
        for subtype in self.superclasses.local.keys() {
            let DeclarationIdentity::Project { library, .. } = subtype else {
                continue;
            };
            visited.clear();
            visited.insert(subtype);
            let mut current = self.superclasses.get(subtype);
            while let Some(ResolvedType::Interface { identity, .. }) = current {
                if !visited.insert(identity) {
                    break;
                }
                if matches!(identity, DeclarationIdentity::Project { library: other, .. } if other == library)
                {
                    marked.push(identity.clone());
                }
                current = self.superclasses.get(identity);
            }
        }
        for identity in marked {
            if let Some(facts) = self.declarations.get_mut(&identity) {
                facts.has_subclass_in_library = true;
            }
        }
    }

    fn add_sdk_signatures(&mut self) {
        for name in ["TestOn", "Timeout", "Tags", "OnPlatform"] {
            self.declarations.insert(
                DeclarationIdentity::package("test", name),
                DeclarationFacts {
                    annotations: Vec::new(),
                    library_target: true,
                    constructors: Vec::new(),
                    static_consts: Vec::new(),
                    extension_representation: None,
                    is_abstract: false,
                    has_subclass_in_library: false,
                },
            );
        }
        let add = |index: &mut Self,
                   library: &'static str,
                   type_name: &'static str,
                   arity: usize,
                   member: &str,
                   positional: Vec<ResolvedType>,
                   return_type: ResolvedType| {
            let parameters: Vec<_> = (0..arity)
                .map(|ordinal| sdk_parameter(type_name, ordinal))
                .collect();
            let identity = DeclarationIdentity::sdk(library, type_name);
            let entry = index
                .member_entries
                .entry(identity)
                .or_default()
                .entry(member.to_string())
                .or_default();
            entry.is_instance_method = true;
            entry.signature = Some(ResolvedSignature {
                owner_parameters: parameters,
                shape: Some(FunctionTypeShape::empty()),
                positional,
                ..ResolvedSignature::returning(return_type)
            });
        };
        let parameter = |type_name: &'static str, ordinal: usize| ResolvedType::TypeParameter {
            id: sdk_parameter(type_name, ordinal),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let object = DeclarationIdentity::sdk("dart:core", "Object");
        for (name, kind, parameters) in [
            ("==", MemberKind::Operator, vec!["other".to_string()]),
            (
                "noSuchMethod",
                MemberKind::Method,
                vec!["invocation".to_string()],
            ),
            ("hashCode", MemberKind::Getter, Vec::new()),
            ("runtimeType", MemberKind::Getter, Vec::new()),
            ("toString", MemberKind::Method, Vec::new()),
        ] {
            self.member_entries
                .entry(object.clone())
                .or_default()
                .entry(name.to_string())
                .or_default()
                .facts
                .push(MemberFacts {
                    declaring_type: object.clone(),
                    kind,
                    is_static: false,
                    is_abstract: false,
                    parameter_contracts: Some(if parameters.is_empty() {
                        empty_parameter_contracts()
                    } else {
                        parameters
                            .iter()
                            .map(|name| ParameterContract {
                                name: name.clone(),
                                kind: ParamKind::Positional,
                                is_covariant: false,
                                default_value_code: None,
                            })
                            .collect()
                    }),
                    positional_parameter_names: parameters,
                    annotations: Vec::new(),
                });
        }
        for (library, name, arity, member, positional, return_type) in [
            (
                "dart:core",
                "List",
                1,
                "add",
                vec![parameter("List", 0)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "List",
                1,
                "[]=",
                vec![ResolvedType::Dynamic, parameter("List", 0)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "Set",
                1,
                "add",
                vec![parameter("Set", 0)],
                ResolvedType::interface_identity(DeclarationIdentity::sdk("dart:core", "bool")),
            ),
            (
                "dart:collection",
                "Queue",
                1,
                "add",
                vec![parameter("Queue", 0)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "Map",
                2,
                "[]=",
                vec![parameter("Map", 0), parameter("Map", 1)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "String",
                0,
                "toUpperCase",
                Vec::new(),
                ResolvedType::interface_identity(DeclarationIdentity::sdk("dart:core", "String")),
            ),
        ] {
            add(self, library, name, arity, member, positional, return_type);
        }
        for (library, name) in [
            ("dart:core", "List"),
            ("dart:core", "Set"),
            ("dart:collection", "Queue"),
        ] {
            let parameter_id = sdk_parameter(name, 0);
            let parameter = parameter(name, 0);
            let identity = DeclarationIdentity::sdk(library, name);
            let supertype = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::sdk("dart:core", "Iterable"),
                arguments: vec![parameter.clone()].into(),
                nullable: false,
                extension_type: false,
            };
            self.supertypes.insert(
                identity.clone(),
                (vec![parameter_id], vec![supertype.clone()]),
            );
            self.member_supertypes.insert(identity, vec![supertype]);
        }
        let iterable = DeclarationIdentity::sdk("dart:core", "Iterable");
        let map_parameter = sdk_parameter("Iterable.map", 0);
        let map_type = parameter("Iterable.map", 0);
        let entry = self
            .member_entries
            .entry(iterable)
            .or_default()
            .entry("map".to_string())
            .or_default();
        entry.is_instance_method = true;
        entry.signature = Some(ResolvedSignature {
            owner_parameters: vec![sdk_parameter("Iterable", 0)],
            call_parameters: vec![map_parameter],
            shape: Some(FunctionTypeShape::new(Vec::new(), vec![map_type.clone()])),
            positional: vec![ResolvedType::Function {
                shape: Some(FunctionTypeShape::empty()),
                alias: FunctionTypeAlias::default(),
                optional_positional: 0,
                return_type: Arc::new(map_type.clone()),
                positional: vec![parameter("Iterable", 0)].into(),
                named: Vec::new().into(),
                nullable: false,
            }],
            ..ResolvedSignature::returning(ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::sdk("dart:core", "Iterable"),
                arguments: vec![map_type].into(),
                nullable: false,
                extension_type: false,
            })
        });

        let widget = DeclarationIdentity::flutter("Widget");
        let key = ResolvedType::interface_identity(DeclarationIdentity::flutter("Key"))
            .with_nullable(true);
        for name in [
            "StatelessWidget",
            "StatefulWidget",
            "InheritedWidget",
            "ProxyWidget",
            "RenderObjectWidget",
            "Text",
            "Container",
            "Builder",
            "ElevatedButton",
            "GestureDetector",
            "MaterialApp",
            "SizedBox",
        ] {
            let identity = DeclarationIdentity::flutter(name);
            let supertype = ResolvedType::interface_identity(widget.clone());
            self.supertypes
                .insert(identity.clone(), (Vec::new(), vec![supertype.clone()]));
            self.superclasses
                .insert(identity.clone(), supertype.clone());
            self.member_supertypes
                .insert(identity.clone(), vec![supertype]);
            self.constructors.entry(identity).or_default().insert(
                "new".to_string(),
                ResolvedSignature {
                    named: HashMap::from([("key".to_string(), key.clone())]),
                    ..ResolvedSignature::returning(ResolvedType::Unknown)
                },
            );
        }
        self.constructors.entry(widget).or_default().insert(
            "new".to_string(),
            ResolvedSignature {
                named: HashMap::from([("key".to_string(), key)]),
                ..ResolvedSignature::returning(ResolvedType::Unknown)
            },
        );
        self.add_sdk_hierarchy();
    }

    /// Seeds [`SDK_HIERARCHY`] plus the stream/file members whose results
    /// resource-tracking rules need to type (`Stream.listen`,
    /// `StreamController.stream`, `File.openWrite`).
    fn add_sdk_hierarchy(&mut self) {
        let arity = |library: &str, name: &str| {
            SDK_HIERARCHY
                .iter()
                .find(|entry| entry.0 == library && entry.1 == name)
                .map_or(0, |entry| entry.2)
        };
        let sdk = |library: &'static str, name: &'static str, arguments: Vec<ResolvedType>| {
            ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::sdk(library, name),
                arguments: arguments.into(),
                nullable: false,
                extension_type: false,
            }
        };
        for &(library, name, own_arity, supertypes) in SDK_HIERARCHY {
            let parameters: Vec<_> = (0..own_arity)
                .map(|ordinal| sdk_parameter(name, ordinal))
                .collect();
            let resolved = supertypes
                .iter()
                .map(|&(super_library, super_name)| {
                    let arguments =
                        if own_arity > 0 && arity(super_library, super_name) == own_arity {
                            parameters
                                .iter()
                                .map(|id| ResolvedType::TypeParameter {
                                    id: id.clone(),
                                    bound: Arc::new(ResolvedType::Dynamic),
                                    nullable: false,
                                })
                                .collect()
                        } else if (super_library, super_name) == ("dart:core", "Comparable") {
                            // Every listed implementer is `Comparable<Self>`.
                            vec![sdk(library, name, Vec::new())]
                        } else {
                            Vec::new()
                        };
                    sdk(super_library, super_name, arguments)
                })
                .collect();
            self.supertypes
                .entry(DeclarationIdentity::sdk(library, name))
                .or_insert((parameters, resolved));
        }
        let element = |owner: &'static str| ResolvedType::TypeParameter {
            id: sdk_parameter(owner, 0),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let listen = self
            .member_entries
            .entry(DeclarationIdentity::sdk("dart:async", "Stream"))
            .or_default()
            .entry("listen".to_string())
            .or_default();
        listen.is_instance_method = true;
        listen.signature = Some(ResolvedSignature {
            owner_parameters: vec![sdk_parameter("Stream", 0)],
            positional: vec![ResolvedType::Function {
                shape: Some(FunctionTypeShape::empty()),
                alias: FunctionTypeAlias::default(),
                optional_positional: 0,
                return_type: Arc::new(ResolvedType::Void),
                positional: vec![element("Stream")].into(),
                named: Vec::new().into(),
                nullable: true,
            }],
            ..ResolvedSignature::returning(sdk(
                "dart:async",
                "StreamSubscription",
                vec![element("Stream")],
            ))
        });
        self.member_entries
            .entry(DeclarationIdentity::sdk("dart:async", "StreamController"))
            .or_default()
            .entry("stream".to_string())
            .or_default()
            .read = Some(sdk(
            "dart:async",
            "Stream",
            vec![element("StreamController")],
        ));
        let open_write = self
            .member_entries
            .entry(DeclarationIdentity::sdk("dart:io", "File"))
            .or_default()
            .entry("openWrite".to_string())
            .or_default();
        open_write.is_instance_method = true;
        open_write.signature = Some(ResolvedSignature::returning(sdk(
            "dart:io",
            "IOSink",
            Vec::new(),
        )));
    }

    fn add_type(
        &mut self,
        name: &str,
        type_parameters: &[TypeParam],
        supertypes: Vec<&DartType>,
        member_supertypes: Vec<&DartType>,
        members: &EffectiveMembers<'_>,
        model: &SemanticModel<'_>,
    ) {
        let type_identity = model.resolve_name(name);
        let is_extension = type_identity.is_none();
        let Some(identity) = type_identity.or_else(|| model.resolve_extension(name)) else {
            return;
        };
        let mut scope = TypeParameterScope::default();
        scope.push(type_parameters, model);
        let owner_parameters = parameter_ids(&scope, type_parameters);
        record_bound_declarations(&mut self.bound_declared, &scope, type_parameters);
        let bounds: HashMap<_, _> = owner_parameters
            .iter()
            .zip(type_parameters)
            .map(|(id, parameter)| {
                (
                    id.clone(),
                    parameter
                        .bound
                        .as_ref()
                        .map(|bound| model.resolve_type_in(bound, &scope))
                        .unwrap_or(ResolvedType::Dynamic),
                )
            })
            .collect();
        for id in &owner_parameters {
            self.owner_defaults.insert(
                id.clone(),
                instantiate_parameter_bound(id, &bounds, &mut HashSet::new(), model),
            );
        }
        let mut field_types = members
            .declared
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field) if !field.is_static => Some(field),
                _ => None,
            })
            .flat_map(|field| {
                let field_type = field
                    .field_type
                    .as_ref()
                    .map(|ty| model.resolve_type_in(ty, &scope))
                    .unwrap_or(ResolvedType::Unknown);
                field
                    .declarators
                    .iter()
                    .map(move |declarator| (declarator.name.name.clone(), field_type.clone()))
            })
            .collect::<HashMap<_, _>>();
        for parameter in members.declaring_params() {
            field_types.insert(
                parameter.name.name.clone(),
                parameter
                    .param_type
                    .as_ref()
                    .map(|ty| model.resolve_type_in(ty, &scope))
                    .unwrap_or(ResolvedType::Unknown),
            );
        }
        let mut resolved_supertypes: Vec<_> = supertypes
            .into_iter()
            .map(|supertype| model.resolve_type_in(supertype, &scope))
            .collect();
        // Extension types retain only their declared superinterfaces: their
        // representation may admit null, so an implicit Object edge is unsound.
        let inherits_object = !is_extension
            && identity != DeclarationIdentity::OBJECT
            && self.type_kind(&identity) != Some(TypeKind::ExtensionType);
        if inherits_object {
            resolved_supertypes.push(model.core_type("Object", false));
        }
        self.supertypes.insert(
            identity.clone(),
            (owner_parameters.clone(), resolved_supertypes),
        );
        let mut resolved_member_supertypes: Vec<_> = member_supertypes
            .into_iter()
            .map(|supertype| model.resolve_type_in(supertype, &scope))
            .collect();
        if inherits_object {
            resolved_member_supertypes.push(model.core_type("Object", false));
        }
        self.member_supertypes
            .insert(identity.clone(), resolved_member_supertypes);
        let entries = self.member_entries.entry(identity.clone()).or_default();
        for member in members.declared {
            match member {
                ClassMember::Method(method) => {
                    let entry = entries.entry(method.name.name.clone()).or_default();
                    scope.push(&method.type_params, model);
                    record_bound_declarations(
                        &mut self.bound_declared,
                        &scope,
                        &method.type_params,
                    );
                    entry.override_shape = Some(inherited::OverrideShape::new(
                        &method.params,
                        &method.type_params,
                        &scope,
                    ));
                    entry.is_instance_method |= !method.is_static;
                    entry.signature = Some(signature(
                        &scope,
                        &owner_parameters,
                        &method.type_params,
                        Some(&method.params),
                        method.return_type.as_ref(),
                        None,
                        model,
                    ));
                    entry.facts.push(MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Method,
                        is_static: method.is_static,
                        // An `external` method has no body but is concrete.
                        is_abstract: method.is_abstract && !method.is_external,
                        positional_parameter_names: positional_parameter_names(&method.params),
                        annotations: resolve_annotations(&method.annotations, model),
                        parameter_contracts: Some(parameter_contracts(&method.params)),
                    });
                    scope.pop();
                }
                ClassMember::Constructor(constructor) => {
                    self.constructors
                        .entry(identity.clone())
                        .or_default()
                        .insert(
                            constructor
                                .constructor_name
                                .as_ref()
                                .map(|name| name.name.clone())
                                .unwrap_or_else(|| "new".to_string()),
                            signature(
                                &scope,
                                &owner_parameters,
                                &[],
                                Some(&constructor.params),
                                None,
                                Some(&field_types),
                                model,
                            ),
                        );
                }
                ClassMember::Field(field) => {
                    let field_type = field
                        .field_type
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Unknown);
                    for declarator in &field.declarators {
                        let kind = MemberKind::field(field, declarator);
                        let entry = entries.entry(declarator.name.name.clone()).or_default();
                        entry.promotable_field = declarator.name.name.starts_with('_')
                            && !field.is_static
                            && !field.is_abstract
                            && !field.is_external
                            && matches!(field.keyword, VarKeyword::Final | VarKeyword::Const);
                        entry.facts.push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind,
                            is_static: field.is_static,
                            is_abstract: field.is_abstract,
                            positional_parameter_names: Vec::new(),
                            parameter_contracts: Some(if kind.has_setter() {
                                setter_parameter_contracts(
                                    "value",
                                    matches!(
                                        kind,
                                        MemberKind::Field {
                                            covariant: true,
                                            ..
                                        }
                                    ),
                                )
                            } else {
                                empty_parameter_contracts()
                            }),
                            annotations: resolve_annotations(&field.annotations, model),
                        });
                        entry.field = Some((owner_parameters.clone(), field_type.clone()));
                        if !field.is_static {
                            entry.read = Some(field_type.clone());
                            if kind.has_setter() {
                                entry.write = Some(field_type.clone());
                            }
                        }
                    }
                }
                ClassMember::Getter(getter) => {
                    let entry = entries.entry(getter.name.name.clone()).or_default();
                    entry.facts.push(MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Getter,
                        is_static: getter.is_static,
                        is_abstract: getter.is_abstract,
                        positional_parameter_names: Vec::new(),
                        parameter_contracts: Some(empty_parameter_contracts()),
                        annotations: resolve_annotations(&getter.annotations, model),
                    });
                    let return_type = getter
                        .return_type
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Dynamic);
                    entry.field = Some((owner_parameters.clone(), return_type.clone()));
                    entry.implicit_getter = getter.return_type.is_none();
                    if !getter.is_static {
                        entry.read = Some(return_type);
                    }
                }
                ClassMember::Setter(setter) => {
                    let entry = entries.entry(setter.name.name.clone()).or_default();
                    entry.facts.push(MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Setter,
                        is_static: setter.is_static,
                        is_abstract: setter.is_abstract,
                        positional_parameter_names: vec![setter.param.name.clone()],
                        parameter_contracts: Some(setter_parameter_contracts(
                            &setter.param.name,
                            setter.is_covariant,
                        )),
                        annotations: resolve_annotations(&setter.annotations, model),
                    });
                    let parameter_type = setter
                        .param_type
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Dynamic);
                    entry
                        .field
                        .get_or_insert_with(|| (owner_parameters.clone(), parameter_type.clone()));
                    if !setter.is_static {
                        entry.write = Some(parameter_type);
                    }
                }
                ClassMember::Operator(operator) => {
                    let entry = entries
                        .entry(operator.lookup_name().to_owned())
                        .or_default();
                    entry.is_instance_method = true;
                    entry.override_shape =
                        Some(inherited::OverrideShape::new(&operator.params, &[], &scope));
                    entry.signature = Some(signature(
                        &scope,
                        &owner_parameters,
                        &[],
                        Some(&operator.params),
                        operator.return_type.as_ref(),
                        None,
                        model,
                    ));
                    entry.facts.push(MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Operator,
                        is_static: false,
                        is_abstract: operator.body.is_none() && !operator.is_external,
                        positional_parameter_names: positional_parameter_names(&operator.params),
                        annotations: resolve_annotations(&operator.annotations, model),
                        parameter_contracts: Some(parameter_contracts(&operator.params)),
                    });
                }
                // Indexed through the constructor it completes (`element_members`).
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => {}
            }
        }
        for parameter in members.declaring_params() {
            let is_final = members.representation || parameter.is_final;
            let field_type = field_types[&parameter.name.name].clone();
            let entry = entries.entry(parameter.name.name.clone()).or_default();
            entry.promotable_field = parameter.name.name.starts_with('_') && is_final;
            entry.facts.push(MemberFacts {
                declaring_type: identity.clone(),
                kind: MemberKind::Field {
                    writable: !is_final,
                    covariant: parameter.is_covariant,
                },
                is_static: false,
                is_abstract: false,
                positional_parameter_names: Vec::new(),
                parameter_contracts: Some(if is_final {
                    empty_parameter_contracts()
                } else {
                    setter_parameter_contracts("value", parameter.is_covariant)
                }),
                annotations: resolve_annotations(&parameter.annotations, model),
            });
            entry.field = Some((owner_parameters.clone(), field_type.clone()));
            entry.read = Some(field_type.clone());
            if !is_final {
                entry.write = Some(field_type);
            }
        }
        if let Some(primary) = members.primary {
            let mut primary_signature = signature(
                &scope,
                &owner_parameters,
                &[],
                Some(&primary.params),
                None,
                Some(&field_types),
                model,
            );
            for parameter in primary.params.named().filter(|parameter| {
                (parameter.is_final || parameter.is_var || members.representation)
                    && parameter.name.name.starts_with('_')
            }) {
                let name = &parameter.name.name;
                let label = name.trim_start_matches('_');
                if let Some(ty) = primary_signature.named.remove(name) {
                    primary_signature.named.insert(label.to_string(), ty);
                }
                if let Some(default) = primary_signature.defaults.remove(name) {
                    primary_signature
                        .defaults
                        .insert(label.to_string(), default);
                }
                if primary_signature.non_optional_defaults.remove(name) {
                    primary_signature
                        .non_optional_defaults
                        .insert(label.to_string());
                }
            }
            if let Some(shape) = &primary_signature.shape {
                primary_signature.shape = Some(FunctionTypeShape::new(
                    shape
                        .required_named
                        .iter()
                        .map(|name| {
                            if primary.params.named().any(|parameter| {
                                parameter.name.name == *name
                                    && (parameter.is_final
                                        || parameter.is_var
                                        || members.representation)
                            }) {
                                name.trim_start_matches('_').to_string()
                            } else {
                                name.clone()
                            }
                        })
                        .collect(),
                    shape.type_parameters.to_vec(),
                ));
            }
            self.constructors
                .entry(identity.clone())
                .or_default()
                .insert(
                    primary
                        .constructor_name
                        .as_ref()
                        .map_or_else(|| "new".to_string(), |name| name.name.clone()),
                    primary_signature,
                );
        }
        if let DeclarationIdentity::Project { library, .. } = identity
            && !is_extension
        {
            for (name, entry) in entries {
                if name.starts_with('_')
                    && !entry.promotable_field
                    && entry.facts.iter().any(|facts| {
                        !facts.is_static
                            && !facts.is_abstract
                            && matches!(facts.kind, MemberKind::Field { .. } | MemberKind::Getter)
                    })
                {
                    let blockers = self.blocked_field_promotions.entry(library).or_default();
                    if !blockers.contains(name) {
                        blockers.insert(name.clone());
                    }
                }
            }
        }
    }

    pub fn declaration(&self, identity: &DeclarationIdentity) -> Option<&DeclarationFacts> {
        self.declarations.get(identity)
    }

    /// Type of a closure parameter or return that `context` does not supply:
    /// Dart's `dynamic` without a context or under a resolved non-function
    /// one, but `Unknown` when the context exists and is unresolved.
    pub fn uncontextual_closure_type(&self, context: Option<&ResolvedType>) -> ResolvedType {
        match context {
            Some(
                ResolvedType::Unknown
                | ResolvedType::TypeParameter { .. }
                | ResolvedType::NonNullTypeParameter { .. }
                | ResolvedType::Function { .. },
            ) => ResolvedType::Unknown,
            Some(ResolvedType::Interface { identity, .. })
                if identity.is_sdk("dart:async", "FutureOr")
                    || (!matches!(identity, DeclarationIdentity::Sdk { .. })
                        && self.declaration(identity).is_none()) =>
            {
                ResolvedType::Unknown
            }
            _ => ResolvedType::Dynamic,
        }
    }

    /// The number of type parameters an indexed class, mixin, enum, or
    /// extension type declares (project or loaded external declaration);
    /// `None` for unindexed types.
    pub fn type_parameter_count(&self, identity: &DeclarationIdentity) -> Option<usize> {
        self.type_parameters(identity)
            .map(|parameters| parameters.len())
    }

    /// The declaration kind (class, mixin, enum, ...) of the indexed type
    /// `identity`, project or external; `None` when unindexed.
    pub fn type_kind(&self, identity: &DeclarationIdentity) -> Option<TypeKind> {
        self.kinds.get(identity).copied()
    }

    pub fn constant_initializer(&self, identity: &DeclarationIdentity) -> Option<&Expr> {
        self.constants.get(identity)
    }

    pub fn constant_type(&self, identity: &DeclarationIdentity) -> Option<&DeclarationIdentity> {
        self.constant_types.get(identity)
    }

    /// Whether `name` is a value of the project enum `identity`.
    pub fn is_enum_value(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        self.enum_values
            .get(identity)
            .is_some_and(|values| values.iter().any(|value| value == name))
    }

    /// Whether `identity` is a project enum.
    pub fn is_enum(&self, identity: &DeclarationIdentity) -> bool {
        self.enum_values.contains_key(identity)
    }

    /// The explicit `extends` type of an indexed class or mixin class.
    pub fn superclass(&self, identity: &DeclarationIdentity) -> Option<&ResolvedType> {
        self.superclasses.get(identity)
    }

    /// The body of the project `const` constructor `identity.name`.
    pub fn const_constructor(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&ConstConstructorBody> {
        self.const_constructors.get(identity)?.get(name)
    }

    /// Instance field initializers of a project class with a `const`
    /// constructor.
    pub fn const_field_initializers(&self, identity: &DeclarationIdentity) -> &[(String, Expr)] {
        self.const_field_initializers
            .get(identity)
            .map_or(&[], Vec::as_slice)
    }

    /// `ty` viewed as an instance of `target` (the analyzer's `asInstanceOf`):
    /// `ty` itself or the supertype instantiation naming `target`.
    pub fn as_instance_of(
        &self,
        ty: &ResolvedType,
        target: &DeclarationIdentity,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.instantiated_identity(ty, target, model)
    }

    pub fn is_subtype_of(
        &self,
        ty: &ResolvedType,
        target: &DeclarationIdentity,
        model: &SemanticModel<'_>,
    ) -> Truth {
        if matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic) {
            return Truth::Unknown;
        }
        if self.instantiated_identity(ty, target, model).is_some() {
            Truth::Yes
        } else {
            Truth::Unknown
        }
    }

    /// The static read type of a top-level variable or getter.
    pub fn value_type(&self, identity: &DeclarationIdentity) -> Option<&ResolvedType> {
        self.value_types.get(identity)
    }

    /// Whether reading this top-level value invokes a getter.
    pub fn is_value_getter(&self, identity: &DeclarationIdentity) -> bool {
        self.value_getters.contains(identity)
            || self
                .external
                .as_ref()
                .is_some_and(|base| base.is_value_getter(identity))
    }

    fn is_cyclic_value(&self, identity: &DeclarationIdentity) -> bool {
        self.cyclic_values.contains(identity)
            || self
                .external
                .as_ref()
                .is_some_and(|base| base.is_cyclic_value(identity))
    }

    pub fn function(&self, identity: &DeclarationIdentity) -> Option<&ResolvedSignature> {
        self.functions.get(identity)
    }

    /// Resolved external annotations of a top-level function.
    pub fn function_annotations(&self, identity: &DeclarationIdentity) -> &[DeclarationIdentity] {
        self.function_annotations
            .get(identity)
            .map_or(&[], Vec::as_slice)
    }

    /// Includes generative constructors forwarded by named mixin applications,
    /// with parameter types rebound to the application's own generic scope.
    pub fn constructor(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&ResolvedSignature> {
        self.constructors.get(identity)?.get(name)
    }

    /// Whether source facts prove an explicit or implicit constructor exists.
    /// Abstract classes and enums also have implicit unnamed constructors.
    pub fn has_constructor(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        self.constructor(identity, name).is_some()
            || (name == "new"
                && matches!(
                    self.type_kind(identity),
                    Some(TypeKind::Class | TypeKind::MixinClass | TypeKind::Enum)
                )
                && self
                    .declaration(identity)
                    .is_some_and(|facts| facts.constructors.is_empty()))
    }

    /// Whether a constructor can be referenced as a callable tear-off.
    /// Enum and abstract generative constructors exist but cannot be torn off.
    pub fn has_constructor_tearoff(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        self.has_constructor(identity, name)
            && !self.is_enum(identity)
            && self.declaration(identity).is_none_or(|facts| {
                !facts.is_abstract
                    || facts
                        .constructors
                        .iter()
                        .any(|constructor| constructor.name == name && constructor.is_factory)
            })
    }

    /// A constructor tear-off's callable type, including its constructed result
    /// and alias substitutions; shared with ordinary expression inference.
    pub fn constructor_tearoff_type(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        if model.identities.type_alias(identity).is_none()
            && !self.has_constructor_tearoff(identity, name)
        {
            return None;
        }
        // Constructor typing does not inspect lexical flow scopes.
        TypeEnvironment::default().constructor_tearoff(
            identity.clone(),
            name,
            &[],
            &InferenceContext {
                model,
                signatures: self,
                type_parameters: &TypeParameterScope::default(),
                lookup: &|_: &str| None,
                argument_fallback: None,
            },
            None,
        )
    }

    /// The declaration's generic owner parameters in source order.
    pub fn type_parameters(&self, identity: &DeclarationIdentity) -> Option<&[TypeParameterId]> {
        self.supertypes
            .get(identity)
            .map(|(parameters, _)| parameters.as_slice())
    }

    /// `identity` instantiated with its own `parameters`, each bounded by its
    /// instantiate-to-bound default, as the result of a constructor whose type
    /// arguments are inferred.
    pub fn self_instantiation(
        &self,
        identity: DeclarationIdentity,
        parameters: &[TypeParameterId],
        model: &SemanticModel<'_>,
    ) -> ResolvedType {
        let extension_type = self.type_kind(&identity) == Some(TypeKind::ExtensionType);
        ResolvedType::Interface {
            occurrence: Default::default(),
            identity,
            arguments: parameters
                .iter()
                .map(|id| ResolvedType::TypeParameter {
                    id: id.clone(),
                    bound: Arc::new(
                        self.owner_defaults
                            .get(id)
                            .cloned()
                            .unwrap_or_else(|| model.core_type("Object", true)),
                    ),
                    nullable: false,
                })
                .collect(),
            nullable: false,
            extension_type,
        }
    }

    /// The solution for `id` when only a top-type upper constraint
    /// (`candidate`) bounds it: a declared bound joins that constraint, so the
    /// solution is their greatest lower bound, the bound unless the bound is
    /// the more top type. `Object?` thus instantiates `T extends Object?` under
    /// a `dynamic` context. `None` when `candidate` is not a top type, the
    /// declaration omits the bound (which adds no constraint), or the bound
    /// depends on other `parameters` being solved.
    fn declared_bound_meet(
        &self,
        id: &TypeParameterId,
        candidate: &ResolvedType,
        bound: &ResolvedType,
        parameters: &[TypeParameterId],
    ) -> Option<ResolvedType> {
        if self.bound_declared.get(id) != Some(&true) || mentions_any(bound, parameters) {
            return None;
        }
        let rank = top_rank(candidate)?;
        Some(
            if top_rank(bound).is_none_or(|bound_rank| bound_rank < rank) {
                bound.clone()
            } else {
                candidate.clone()
            },
        )
    }

    /// Whether the declaration explicitly records the named constructor as const.
    pub fn is_const_constructor(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        self.declaration(identity).is_some_and(|facts| {
            facts
                .constructors
                .iter()
                .any(|constructor| constructor.name == name && constructor.is_const)
        })
    }

    pub fn resolved_constructor(
        &self,
        receiver: &ResolvedType,
        name: &str,
    ) -> Option<(&ResolvedSignature, HashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = receiver
        else {
            return None;
        };
        let signature = self.constructor(identity, name)?;
        let substitutions = self.owner_substitutions(&signature.owner_parameters, arguments, None);
        Some((signature, substitutions))
    }

    pub fn member(&self, identity: &DeclarationIdentity, name: &str) -> Option<&ResolvedSignature> {
        self.member_entry(identity, name)?.signature.as_ref()
    }

    fn has_instance_declaration(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        access: InstanceAccess,
    ) -> bool {
        let Some(entry) = self.member_entry(identity, name) else {
            return false;
        };
        (match access {
            InstanceAccess::Read | InstanceAccess::Invoke => {
                entry.is_instance_method || entry.read.is_some()
            }
            InstanceAccess::Write => entry.write.is_some(),
        }) || entry.facts.iter().any(|fact| {
            !fact.is_static
                && (match access {
                    InstanceAccess::Read => {
                        fact.kind == MemberKind::Method || fact.kind.has_getter()
                    }
                    InstanceAccess::Write => fact.kind.has_setter(),
                    InstanceAccess::Invoke => fact.kind != MemberKind::Setter,
                })
        })
    }

    fn instance_member(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&ResolvedSignature> {
        let entry = self.member_entry(identity, name)?;
        if entry.is_instance_method {
            entry.signature.as_ref()
        } else {
            None
        }
    }

    pub fn inherited_member_facts(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<Vec<MemberFacts>> {
        let (facts, complete) = self.known_inherited_member_facts(identity, name)?;
        complete.then_some(facts)
    }

    /// Every inherited fact named `name` found in the known part of
    /// `identity`'s supertype graph, and whether that graph is fully known (no
    /// unresolved, dependency, or unlisted SDK supertypes). `None` when
    /// `identity` itself is unknown.
    pub fn known_inherited_member_facts(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<(Vec<MemberFacts>, bool)> {
        let (_, supertypes) = self.supertypes.get(identity)?;
        let mut facts = Vec::new();
        let mut visited = HashSet::new();
        let mut complete = true;
        for supertype in supertypes {
            complete &= self.collect_inherited_member_facts(
                identity,
                supertype,
                name,
                &mut visited,
                &mut facts,
            );
        }
        Some((facts, complete))
    }

    /// Collects the facts of `supertype` and its supertypes; returns whether
    /// every reached type is known.
    fn collect_inherited_member_facts(
        &self,
        owner: &DeclarationIdentity,
        supertype: &ResolvedType,
        name: &str,
        visited: &mut HashSet<DeclarationIdentity>,
        facts: &mut Vec<MemberFacts>,
    ) -> bool {
        let ResolvedType::Interface { identity, .. } = supertype else {
            return false;
        };
        if !visited.insert(identity.clone()) {
            return true;
        }
        let is_object = *identity == DeclarationIdentity::OBJECT;
        let mut complete = true;
        if let DeclarationIdentity::Sdk {
            library,
            name: type_name,
        } = identity
            && !is_object
            && !self.kinds.contains_key(identity)
        {
            // An SDK type whose source is not loaded: the curated supertype
            // edges say nothing about what it declares, so reaching one is
            // unknown unless its members are listed in [`SDK_INTERFACE_MEMBERS`].
            match SDK_INTERFACE_MEMBERS
                .iter()
                .find(|entry| entry.0 == library.as_str() && entry.1 == type_name.as_str())
            {
                Some((.., members)) => facts.extend(
                    members.iter().filter(|member| member.0 == name).map(
                        |&(_, kind, parameters)| MemberFacts {
                            declaring_type: identity.clone(),
                            kind,
                            is_static: false,
                            is_abstract: true,
                            positional_parameter_names: parameters
                                .iter()
                                .map(|&parameter| parameter.to_owned())
                                .collect(),
                            annotations: Vec::new(),
                            parameter_contracts: None,
                        },
                    ),
                ),
                None => complete = false,
            }
        } else if !name.starts_with('_') || same_library(owner, identity) {
            facts.extend(
                self.member_entry(identity, name)
                    .into_iter()
                    .flat_map(|entry| &entry.facts)
                    .filter(|member| !member.is_static)
                    .cloned(),
            );
        }
        if let Some((_, supertypes)) = self.supertypes.get(identity) {
            for supertype in supertypes {
                complete &=
                    self.collect_inherited_member_facts(owner, supertype, name, visited, facts);
            }
        } else if !is_object {
            complete = false;
        }
        complete
    }

    /// Whether a supertype of `identity` other than `Object` that is a class
    /// (not a mixin) declares an instance member: the analyzer's
    /// `interfaceMembers` reaching a non-`Object` `ClassElement`. `Unknown`
    /// when an unresolved, dependency, or unlisted SDK supertype is reached.
    pub fn inherits_class_instance_member(
        &self,
        identity: &DeclarationIdentity,
        model: &SemanticModel<'_>,
    ) -> Truth {
        let Some((_, supertypes)) = self.supertypes.get(identity) else {
            return Truth::Unknown;
        };
        let mut visited = HashSet::new();
        let mut stack: Vec<&ResolvedType> = supertypes.iter().collect();
        let mut complete = true;
        let receiver = ResolvedType::interface_identity(identity.clone());
        while let Some(supertype) = stack.pop() {
            let ResolvedType::Interface { identity, .. } = supertype else {
                complete = false;
                continue;
            };
            if *identity == DeclarationIdentity::OBJECT || !visited.insert(identity) {
                continue;
            }
            if let DeclarationIdentity::Sdk { library, name } = identity
                && !self.kinds.contains_key(identity)
            {
                match SDK_INTERFACE_MEMBERS
                    .iter()
                    .find(|entry| entry.0 == library.as_str() && entry.1 == name.as_str())
                {
                    Some((.., members)) if !members.is_empty() => return Truth::Yes,
                    Some(_) => {}
                    None => {
                        complete = false;
                        continue;
                    }
                }
            } else if self.kinds.get(identity) != Some(&TypeKind::Mixin)
                && self.member_entries.get(identity).is_some_and(|entries| {
                    entries.iter().any(|(name, entry)| {
                        entry.facts.iter().any(|fact| {
                            if fact.is_static || fact.declaring_type != *identity {
                                return false;
                            }
                            let reads = !matches!(fact.kind, MemberKind::Setter);
                            let writes = matches!(
                                fact.kind,
                                MemberKind::Setter | MemberKind::Field { writable: true, .. }
                            );
                            // The lint examines the merged interface, not every
                            // declaration in its ancestry. A mixin can replace
                            // all class-owned accessors of a member.
                            [
                                reads.then_some(InstanceAccess::Read),
                                writes.then_some(InstanceAccess::Write),
                            ]
                            .into_iter()
                            .flatten()
                            .any(|access| {
                                self.resolved_instance_owner(&receiver, name, access, model)
                                    .is_none_or(|owner| match owner {
                                        ResolvedType::Interface { identity, .. } => {
                                            self.kinds.get(&identity) != Some(&TypeKind::Mixin)
                                        }
                                        _ => true,
                                    })
                            })
                        })
                    })
                })
            {
                return Truth::Yes;
            }
            match self.supertypes.get(identity) {
                Some((_, supertypes)) => stack.extend(supertypes),
                None => complete = false,
            }
        }
        if complete { Truth::No } else { Truth::Unknown }
    }

    /// The class modifier of an indexed class, mixin class, or mixin as the
    /// analyzer's element reports it: a sealed class carries the `final`,
    /// `base`, or `interface` modifier its direct supertypes induce (still
    /// `Sealed` when they induce none). `None` when the declaration or a
    /// supertype the induction depends on is not indexed.
    pub fn effective_restriction(
        &self,
        identity: &DeclarationIdentity,
    ) -> Option<Option<ClassRestriction>> {
        self.effective_restriction_inner(identity, &mut HashSet::new())
    }

    fn effective_restriction_inner<'a>(
        &'a self,
        identity: &'a DeclarationIdentity,
        visiting: &mut HashSet<&'a DeclarationIdentity>,
    ) -> Option<Option<ClassRestriction>> {
        use ClassRestriction::{Base, Final, Interface, Sealed};
        if *identity == DeclarationIdentity::OBJECT {
            return Some(None);
        }
        self.kinds.get(identity)?;
        let declared = self.restrictions.get(identity).copied();
        if declared != Some(Sealed) {
            return Some(declared);
        }
        if !visiting.insert(identity) {
            return None;
        }
        let (_, supertypes) = self.supertypes.get(identity)?;
        let mut restriction_of = |ty: &'a ResolvedType| match ty {
            ResolvedType::Interface { identity, .. } => {
                self.effective_restriction_inner(identity, visiting)
            }
            _ => None,
        };
        let has_superclass = self.superclasses.contains_key(identity);
        let mixins = self
            .mixin_applications
            .get(identity)
            .cloned()
            .unwrap_or_else(|| {
                let start = usize::from(has_superclass);
                start..start
            });
        // The trailing implicit `Object` is no declared interface.
        let interfaces = supertypes.get(mixins.end..supertypes.len().saturating_sub(1))?;
        let superclass = match has_superclass {
            true => restriction_of(supertypes.first()?)?,
            false => None,
        };
        let mixins = supertypes
            .get(mixins)?
            .iter()
            .map(&mut restriction_of)
            .collect::<Option<Vec<_>>>()?;
        let interfaces = interfaces
            .iter()
            .map(&mut restriction_of)
            .collect::<Option<Vec<_>>>()?;
        let any = |types: &[Option<ClassRestriction>], wanted: &[ClassRestriction]| {
            types
                .iter()
                .any(|restriction| restriction.is_some_and(|r| wanted.contains(&r)))
        };
        // The analyzer's `InstanceMemberInferrer._setInducedModifier`.
        let induced = if any(&mixins, &[Final]) {
            Final
        } else {
            match superclass {
                Some(Final) => Final,
                Some(Base) if any(&mixins, &[Interface]) => Final,
                Some(Base) => Base,
                Some(Interface) if any(&interfaces, &[Base]) || any(&mixins, &[Base]) => Final,
                Some(Interface) => Interface,
                _ if any(&interfaces, &[Base, Final]) || any(&mixins, &[Base, Final]) => Base,
                _ if any(&mixins, &[Interface]) && any(&interfaces, &[Base]) => Final,
                _ if any(&mixins, &[Interface]) => Interface,
                _ => Sealed,
            }
        };
        visiting.remove(identity);
        Some(Some(induced))
    }

    /// Whether `identity` or one of its supertypes is annotated `@immutable`:
    /// every supertype (the analyzer's `allSupertypes`), or with
    /// `superclass_only` just the `extends` chain. SDK types never carry the
    /// annotation; `Unknown` when an unresolved or dependency type is reached
    /// before an annotated one.
    pub fn has_immutable_supertype(
        &self,
        identity: &DeclarationIdentity,
        superclass_only: bool,
    ) -> Truth {
        let mut visited = HashSet::new();
        let mut stack = vec![identity.clone()];
        let mut complete = true;
        while let Some(current) = stack.pop() {
            if matches!(current, DeclarationIdentity::Sdk { .. })
                || !visited.insert(current.clone())
            {
                continue;
            }
            match self.immutable.get(&current) {
                Some(Truth::Yes) => return Truth::Yes,
                Some(Truth::No) => {}
                Some(Truth::Unknown) => complete = false,
                None => {
                    complete = false;
                    continue;
                }
            }
            let next: Vec<&ResolvedType> = if superclass_only {
                self.superclasses.get(&current).into_iter().collect()
            } else {
                self.supertypes
                    .get(&current)
                    .map(|(_, supertypes)| supertypes.iter().collect())
                    .unwrap_or_default()
            };
            for supertype in next {
                match supertype {
                    ResolvedType::Interface { identity, .. } => stack.push(identity.clone()),
                    _ => complete = false,
                }
            }
        }
        if complete { Truth::No } else { Truth::Unknown }
    }

    /// The analyzer's `InterfaceElement.hasNonFinalField`: whether `identity`,
    /// a class on its superclass chain, or an applied mixin declares a
    /// non-final, non-abstract instance field. `None` when the chain reaches a
    /// type whose members are unknown.
    pub fn has_non_final_field(&self, identity: &DeclarationIdentity) -> Option<bool> {
        self.has_non_final_field_inner(identity, &mut HashSet::new())
    }

    fn has_non_final_field_inner(
        &self,
        identity: &DeclarationIdentity,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> Option<bool> {
        if *identity == DeclarationIdentity::OBJECT || !visited.insert(identity.clone()) {
            return Some(false);
        }
        if matches!(identity, DeclarationIdentity::Sdk { .. }) {
            return None;
        }
        let declares = self.member_entries.get(identity).is_some_and(|entries| {
            entries.values().flat_map(|entry| &entry.facts).any(|fact| {
                fact.declaring_type == *identity
                    && !fact.is_static
                    && !fact.is_abstract
                    && matches!(fact.kind, MemberKind::Field { writable: true, .. })
            })
        });
        if declares {
            return Some(true);
        }
        for supertype in self.member_supertypes.get(identity)? {
            let ResolvedType::Interface { identity: next, .. } = supertype else {
                return None;
            };
            if self.has_non_final_field_inner(next, visited)? {
                return Some(true);
            }
        }
        Some(false)
    }

    /// The concrete, non-static, non-setter member named `name` that
    /// `identity` inherits from its superclass chain and mixin applications —
    /// the analyzer's `getInheritedConcreteMember`. Declared members of
    /// `identity` itself are not considered, mixins contribute only their own
    /// declared members, and the superclass chain is searched before `Object`.
    ///
    /// `Some(None)` when nothing is inherited, `None` when the chain reaches a
    /// type whose members falcon does not know.
    pub fn inherited_concrete_member(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<Option<&MemberFacts>> {
        let mut visited = HashSet::from([identity.clone()]);
        if let Some((found, _)) = self.concrete_member_in_chain(
            identity,
            identity,
            name,
            |fact| fact.kind != MemberKind::Setter,
            &mut visited,
        )? {
            return Some(Some(found));
        }
        Some(
            self.declared_member(identity, &DeclarationIdentity::OBJECT, name, |fact| {
                !fact.is_abstract && fact.kind != MemberKind::Setter
            }),
        )
    }

    fn concrete_member_in_chain(
        &self,
        owner: &DeclarationIdentity,
        identity: &DeclarationIdentity,
        name: &str,
        filter: impl Fn(&MemberFacts) -> bool + Copy,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> Option<Option<(&MemberFacts, &ResolvedType)>> {
        for supertype in self.member_supertypes.get(identity)? {
            let ResolvedType::Interface { identity: next, .. } = supertype else {
                return None;
            };
            if *next == DeclarationIdentity::OBJECT || !visited.insert(next.clone()) {
                continue;
            }
            // Curated SDK entries describe only some members; only SDK
            // declarations indexed from loaded sources are searched.
            if matches!(next, DeclarationIdentity::Sdk { .. }) && !self.kinds.contains_key(next) {
                return None;
            }
            if let Some(found) =
                self.declared_member(owner, next, name, |fact| !fact.is_abstract && filter(fact))
            {
                return Some(Some((found, supertype)));
            }
            if let Some((found, _)) =
                self.concrete_member_in_chain(owner, next, name, filter, visited)?
            {
                return Some(Some((found, supertype)));
            }
        }
        Some(None)
    }

    /// The non-static, non-setter member named `name` in the interface of
    /// `identity` as seen from `owner`: its own declaration, otherwise the
    /// first one found through its supertypes (abstract members included).
    ///
    /// `Some(None)` when the interface has no such member, `None` when a
    /// supertype's members are unknown before a declaration is found.
    pub fn interface_member(
        &self,
        owner: &DeclarationIdentity,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<Option<&MemberFacts>> {
        self.interface_member_inner(owner, identity, name, &mut HashSet::new())
    }

    fn interface_member_inner(
        &self,
        owner: &DeclarationIdentity,
        identity: &DeclarationIdentity,
        name: &str,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> Option<Option<&MemberFacts>> {
        if !visited.insert(identity.clone()) {
            return Some(None);
        }
        let is_object = *identity == DeclarationIdentity::OBJECT;
        if matches!(identity, DeclarationIdentity::Sdk { .. })
            && !is_object
            && !self.kinds.contains_key(identity)
        {
            return None;
        }
        if let Some(found) = self.declared_member(owner, identity, name, |fact| {
            fact.kind != MemberKind::Setter
        }) {
            return Some(Some(found));
        }
        if is_object {
            return Some(None);
        }
        let (_, supertypes) = self.supertypes.get(identity)?;
        for supertype in supertypes {
            let ResolvedType::Interface { identity: next, .. } = supertype else {
                return None;
            };
            if let Some(found) = self.interface_member_inner(owner, next, name, visited)? {
                return Some(Some(found));
            }
        }
        Some(None)
    }

    /// `identity`'s own non-static declaration of `name` matching `filter`,
    /// if visible from `owner` (private names are library-scoped).
    fn declared_member(
        &self,
        owner: &DeclarationIdentity,
        identity: &DeclarationIdentity,
        name: &str,
        filter: impl Fn(&MemberFacts) -> bool,
    ) -> Option<&MemberFacts> {
        if name.starts_with('_') && !same_library(owner, identity) {
            return None;
        }
        self.member_entry(identity, name)?
            .facts
            .iter()
            .find(|fact| !fact.is_static && fact.declaring_type == *identity && filter(fact))
    }

    pub fn unrelated(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Truth {
        if let (
            ResolvedType::Interface {
                identity: left_identity,
                ..
            },
            ResolvedType::Interface {
                identity: right_identity,
                ..
            },
        ) = (left, right)
        {
            if let Some(instantiated) = self.instantiated_identity(left, right_identity, model) {
                return model.unrelated(&instantiated, right);
            }
            if let Some(instantiated) = self.instantiated_identity(right, left_identity, model) {
                return model.unrelated(left, &instantiated);
            }
        }
        model.unrelated(left, right)
    }

    /// Whether the supertype graph of `identity` reaches a cycle.
    fn has_cyclic_supertypes(&self, identity: &DeclarationIdentity) -> bool {
        match &self.cyclic_supertypes {
            Some(cyclic) => cyclic.contains(identity),
            None => self.reaches_supertype_cycle(
                identity,
                &mut HashSet::new(),
                &mut HashSet::new(),
                &mut HashSet::new(),
            ),
        }
    }

    fn cyclic_supertype_identities(&self) -> FxHashSet<DeclarationIdentity> {
        let (mut cyclic, mut acyclic) = (HashSet::new(), HashSet::new());
        for identity in self.supertypes.local.keys() {
            self.reaches_supertype_cycle(identity, &mut HashSet::new(), &mut cyclic, &mut acyclic);
        }
        cyclic.into_iter().cloned().collect()
    }

    /// Depth-first cycle search; `cyclic`/`acyclic` memoize finished
    /// identities. An identity reaching an ancestor still on the search path
    /// lies on that cycle itself, so every answer is exact.
    fn reaches_supertype_cycle<'s>(
        &'s self,
        identity: &'s DeclarationIdentity,
        visiting: &mut HashSet<&'s DeclarationIdentity>,
        cyclic: &mut HashSet<&'s DeclarationIdentity>,
        acyclic: &mut HashSet<&'s DeclarationIdentity>,
    ) -> bool {
        if !self.supertypes.local.contains_key(identity)
            && let Some(base) = &self.external
        {
            return base.has_cyclic_supertypes(identity);
        }
        if cyclic.contains(identity) || visiting.contains(identity) {
            return true;
        }
        if acyclic.contains(identity) {
            return false;
        }
        visiting.insert(identity);
        let reaches = self
            .supertypes
            .get(identity)
            .is_some_and(|(_, supertypes)| {
                supertypes.iter().any(|supertype| match supertype {
                    ResolvedType::Interface { identity, .. } => {
                        self.reaches_supertype_cycle(identity, visiting, cyclic, acyclic)
                    }
                    _ => false,
                })
            });
        visiting.remove(identity);
        if reaches {
            cyclic.insert(identity);
        } else {
            acyclic.insert(identity);
        }
        reaches
    }

    /// Whether `identity` or one of its transitive supertypes satisfies
    /// `matches`: `No` only when every reached supertype list is recorded for
    /// a project or SDK declaration.
    pub fn supertype_matches(
        &self,
        identity: &DeclarationIdentity,
        matches: &impl Fn(&DeclarationIdentity) -> bool,
    ) -> Truth {
        let mut visited = HashSet::new();
        let mut stack = vec![identity];
        let mut complete = true;
        while let Some(identity) = stack.pop() {
            if !visited.insert(identity) {
                continue;
            }
            if matches(identity) {
                return Truth::Yes;
            }
            match self.supertypes.get(identity) {
                Some((_, supertypes))
                    if !matches!(identity, DeclarationIdentity::Package { .. }) =>
                {
                    for supertype in supertypes {
                        match supertype {
                            ResolvedType::Interface { identity, .. } => stack.push(identity),
                            _ => complete = false,
                        }
                    }
                }
                Some((_, supertypes)) => {
                    complete = false;
                    stack.extend(supertypes.iter().filter_map(|supertype| match supertype {
                        ResolvedType::Interface { identity, .. } => Some(identity),
                        _ => None,
                    }));
                }
                None => complete &= *identity == DeclarationIdentity::OBJECT,
            }
        }
        if complete { Truth::No } else { Truth::Unknown }
    }

    /// A raw use of a generic interface (no type arguments) denotes its
    /// instantiation to bounds; any other type is returned unchanged.
    fn with_raw_arguments(&self, ty: &ResolvedType) -> ResolvedType {
        match ty {
            ResolvedType::Interface {
                occurrence,
                identity,
                arguments,
                nullable,
                extension_type,
            } if arguments.is_empty() => match self.supertypes.get(identity) {
                Some((parameters, _)) if !parameters.is_empty() => ResolvedType::Interface {
                    occurrence: occurrence.clone(),
                    identity: identity.clone(),
                    arguments: parameters
                        .iter()
                        .map(|id| {
                            self.owner_defaults
                                .get(id)
                                .cloned()
                                .unwrap_or(ResolvedType::Dynamic)
                        })
                        .collect(),
                    nullable: *nullable,
                    extension_type: *extension_type,
                },
                _ => ty.clone(),
            },
            _ => ty.clone(),
        }
    }

    /// `ty` with every raw use of an indexed generic interface, at any depth,
    /// replaced by its instantiation to bounds, as the analyzer's type of a
    /// written raw type. Inserted bound defaults are not normalized again.
    pub fn instantiate_to_bounds(&self, ty: &ResolvedType) -> ResolvedType {
        match ty {
            ResolvedType::Interface {
                occurrence,
                identity,
                arguments,
                nullable,
                extension_type,
            } => {
                if arguments.is_empty() {
                    return self.with_raw_arguments(ty);
                }
                ResolvedType::Interface {
                    occurrence: occurrence.clone(),
                    identity: identity.clone(),
                    arguments: arguments
                        .iter()
                        .map(|argument| self.instantiate_to_bounds(argument))
                        .collect(),
                    nullable: *nullable,
                    extension_type: *extension_type,
                }
            }
            ResolvedType::Function {
                alias,
                return_type,
                positional,
                named,
                nullable,
                optional_positional,
                shape,
            } => ResolvedType::Function {
                shape: shape
                    .as_ref()
                    .map(|shape| shape.map_bounds(|bound| self.instantiate_to_bounds(bound))),
                optional_positional: *optional_positional,
                alias: alias.clone(),
                return_type: Arc::new(self.instantiate_to_bounds(return_type)),
                positional: positional
                    .iter()
                    .map(|parameter| self.instantiate_to_bounds(parameter))
                    .collect(),
                named: named
                    .iter()
                    .map(|(name, parameter)| (name.clone(), self.instantiate_to_bounds(parameter)))
                    .collect(),
                nullable: *nullable,
            },
            ResolvedType::Record {
                occurrence,
                positional,
                named,
                nullable,
            } => ResolvedType::Record {
                occurrence: occurrence.clone(),
                positional: positional
                    .iter()
                    .map(|field| self.instantiate_to_bounds(field))
                    .collect(),
                named: named
                    .iter()
                    .map(|(name, field)| (name.clone(), self.instantiate_to_bounds(field)))
                    .collect(),
                nullable: *nullable,
            },
            _ => ty.clone(),
        }
    }

    pub(crate) fn instantiated_identity(
        &self,
        ty: &ResolvedType,
        target: &DeclarationIdentity,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.instantiated_matching(
            ty,
            &|identity| identity.matches(target),
            model,
            &mut FxHashSet::default(),
        )
    }

    fn instantiated_matching(
        &self,
        ty: &ResolvedType,
        target: &impl Fn(&DeclarationIdentity) -> bool,
        model: &SemanticModel<'_>,
        visited: &mut FxHashSet<DeclarationIdentity>,
    ) -> Option<ResolvedType> {
        if let ResolvedType::NonNullTypeParameter { bound, .. } = ty {
            return self.instantiated_matching(&bound.non_null(), target, model, visited);
        }
        if let ResolvedType::TypeParameter {
            bound, nullable, ..
        } = ty
        {
            return self.instantiated_matching(
                &bound.with_nullable(*nullable || bound.nullable()),
                target,
                model,
                visited,
            );
        }
        let ResolvedType::Interface {
            occurrence,
            identity,
            arguments,
            ..
        } = ty
        else {
            return None;
        };
        if !visited.insert(identity.clone()) || self.has_cyclic_supertypes(identity) {
            return None;
        }
        if target(identity) {
            return Some(self.with_raw_arguments(ty));
        }
        let (parameters, supertypes) = self.supertypes.get(identity)?;
        let substitutions = self.owner_substitutions(parameters, arguments, Some(occurrence));
        supertypes.iter().find_map(|supertype| {
            let instantiated = model.substitute(supertype, &substitutions);
            self.instantiated_matching(&instantiated, target, model, visited)
        })
    }

    pub fn instantiated_supertype(
        &self,
        ty: &ResolvedType,
        library: &str,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.instantiated_matching(
            ty,
            &|identity| identity.is_sdk(library, name),
            model,
            &mut FxHashSet::default(),
        )
    }

    /// Dart's least upper bound of interface types where neither is a subtype
    /// of the other and their classes differ (the analyzer's
    /// `InterfaceLeastUpperBoundHelper`): of the superinterfaces both share
    /// with identical type arguments, the unique one of greatest depth, so
    /// `Object` when no deeper level has exactly one. `None` when a candidate's
    /// depth or a possibly shared superinterface is unknown.
    pub(crate) fn common_superinterface(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let (left, left_exact) = self.superinterfaces(left, model)?;
        let (right, right_exact) = self.superinterfaces(right, model)?;
        // An inexact set misses only supertypes above an SDK or package
        // declaration or an enum's implicit superclass, which are SDK or package
        // declarations too: one may be shared when the exact set lists it and
        // the inexact one lacks it.
        let lists = |set: &[ResolvedType], identity: &DeclarationIdentity| {
            set.iter().any(
                |ty| matches!(ty, ResolvedType::Interface { identity: listed, .. } if listed == identity),
            )
        };
        let external_beyond = |exact: &[ResolvedType], inexact: &[ResolvedType]| {
            exact.iter().any(|ty| match ty {
                ResolvedType::Interface { identity, .. } => {
                    !matches!(identity, DeclarationIdentity::Project { .. })
                        && *identity != DeclarationIdentity::OBJECT
                        && !lists(inexact, identity)
                }
                _ => true,
            })
        };
        let hidden = match (left_exact, right_exact) {
            (true, true) => false,
            (false, false) => true,
            (false, true) => external_beyond(&right, &left),
            (true, false) => external_beyond(&left, &right),
        };
        if hidden {
            return None;
        }
        let mut depths = HashMap::new();
        let mut candidates = left
            .into_iter()
            .filter(|candidate| right.contains(candidate))
            .map(|candidate| {
                let ResolvedType::Interface { identity, .. } = &candidate else {
                    return None;
                };
                let depth = self.superinterface_depth(identity, &mut depths)?;
                Some((depth, candidate))
            })
            .collect::<Option<Vec<_>>>()?;
        candidates.sort_by_key(|(depth, _)| std::cmp::Reverse(*depth));
        candidates
            .chunk_by(|(left, _), (right, _)| left == right)
            .find(|level| level.len() == 1)
            .map(|level| level[0].1.clone())
    }

    /// Every superinterface of `ty`, itself included, instantiated and
    /// non-nullable, with whether the set is exact. Declarations indexed from
    /// project or loaded external sources are expanded; an SDK or package
    /// declaration known only from the curated tables is listed but not
    /// expanded, and an enum's implicit superclass may be unrecorded, so
    /// either makes the set inexact. `None` for an extension type, a cyclic
    /// hierarchy, an unknown type argument, a project declaration without
    /// indexed source, or supertypes unrecorded, unresolved or instantiated twice.
    fn superinterfaces(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Option<(Vec<ResolvedType>, bool)> {
        let ResolvedType::Interface { identity, .. } = ty else {
            return None;
        };
        if self.has_cyclic_supertypes(identity) {
            return None;
        }
        let mut found: Vec<ResolvedType> = Vec::new();
        let mut exact = true;
        let mut stack = vec![ty.with_nullable(false)];
        while let Some(ty) = stack.pop() {
            let ty = self.with_raw_arguments(&ty);
            let ResolvedType::Interface {
                occurrence,
                identity,
                arguments,
                extension_type: false,
                ..
            } = &ty
            else {
                return None;
            };
            // An unknown argument may equal, or differ from, any other.
            if has_inference_holes(&ty) {
                return None;
            }
            let seen = found.iter().find(|seen| {
                matches!(seen, ResolvedType::Interface { identity: known, .. } if known == identity)
            });
            if let Some(seen) = seen {
                // Implementing one interface with two instantiations is an error.
                if *seen != ty {
                    return None;
                }
                continue;
            }
            if *identity != DeclarationIdentity::OBJECT {
                match self.kinds.get(identity) {
                    Some(kind) => {
                        exact &= *kind != TypeKind::Enum;
                        let (parameters, supertypes) = self.supertypes.get(identity)?;
                        if parameters.len() != arguments.len() {
                            return None;
                        }
                        let substitutions =
                            self.owner_substitutions(parameters, arguments, Some(occurrence));
                        stack.extend(
                            supertypes
                                .iter()
                                .map(|supertype| model.substitute(supertype, &substitutions)),
                        );
                    }
                    None if matches!(identity, DeclarationIdentity::Project { .. }) => {
                        return None;
                    }
                    None => exact = false,
                }
            }
            found.push(ty);
        }
        Some((found, exact))
    }

    /// The length of the longest supertype path from `identity`, a declaration
    /// indexed from source, to `Object`, counting the anonymous class each
    /// mixin application adds to a superclass chain; `None` through any other
    /// declaration. Callers ensure the hierarchy is acyclic.
    fn superinterface_depth(
        &self,
        identity: &DeclarationIdentity,
        depths: &mut HashMap<DeclarationIdentity, usize>,
    ) -> Option<usize> {
        if *identity == DeclarationIdentity::OBJECT {
            return Some(0);
        }
        if let Some(&depth) = depths.get(identity) {
            return Some(depth);
        }
        if !self.kinds.contains_key(identity) {
            return None;
        }
        let (_, supertypes) = self.supertypes.get(identity)?;
        let mut depth_of = |supertype: &ResolvedType| match supertype {
            ResolvedType::Interface { identity, .. } => self.superinterface_depth(identity, depths),
            _ => None,
        };
        let mixins = self
            .mixin_applications
            .get(identity)
            .cloned()
            .unwrap_or(0..0);
        // `S with M1, M2` is `(S & M1) & M2`; an omitted superclass is `Object`.
        let mut chain = match mixins.start {
            0 => 0,
            _ => depth_of(supertypes.first()?)?,
        };
        for mixin in supertypes.get(mixins.clone())? {
            chain = chain.max(depth_of(mixin)?) + 1;
        }
        let mut depth = chain;
        for supertype in supertypes.get(mixins.end..)? {
            depth = depth.max(depth_of(supertype)?);
        }
        depths.insert(identity.clone(), depth + 1);
        Some(depth + 1)
    }

    /// Three-valued "`ty` implements SDK declaration `library.name`".
    ///
    /// `Yes` when any declared supertype path reaches the target. `No` only when
    /// every reachable edge resolved to a declaration whose own supertype list is
    /// recorded (project declarations and the curated `SDK_HIERARCHY`);
    /// anything else — unresolved supertypes, unlisted SDK or package types,
    /// non-interface types — is `Unknown`. An extension type implements what
    /// its `implements` clause reaches, as the analyzer's `allSupertypes`.
    pub fn implements_sdk(&self, ty: &ResolvedType, library: &str, name: &str) -> Truth {
        match ty {
            ResolvedType::TypeParameter { bound, .. }
            | ResolvedType::NonNullTypeParameter { bound, .. } => {
                self.implements_sdk(bound, library, name)
            }
            ResolvedType::Interface { identity, .. } => self.identity_implements(
                identity,
                &|identity| identity.is_sdk(library, name),
                &mut HashSet::new(),
            ),
            _ => Truth::Unknown,
        }
    }

    /// Match the analyzer linter's interface checks, which inspect extension
    /// representation types and type-parameter bounds rather than subtyping.
    pub fn interface_check_sdk(
        &self,
        ty: &ResolvedType,
        library: &str,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Truth {
        let mut ty = ty;
        while let ResolvedType::TypeParameter { bound, .. }
        | ResolvedType::NonNullTypeParameter { bound, .. } = ty
        {
            ty = bound;
        }
        if matches!(
            ty,
            ResolvedType::Interface {
                extension_type: true,
                ..
            }
        ) {
            return self
                .extension_type_erasure(ty, model)
                .map_or(Truth::Unknown, |erased| {
                    self.implements_sdk(&erased, library, name)
                });
        }
        self.implements_sdk(ty, library, name)
    }

    /// Three-valued "`identity` is, or has a supertype path to, a declaration
    /// satisfying `is_target`"; see [`Self::implements_sdk`] for when `No` is
    /// proven.
    fn identity_implements(
        &self,
        identity: &DeclarationIdentity,
        is_target: &impl Fn(&DeclarationIdentity) -> bool,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> Truth {
        if is_target(identity) {
            return Truth::Yes;
        }
        if *identity == DeclarationIdentity::OBJECT || !visited.insert(identity.clone()) {
            return Truth::No;
        }
        let Some((_, supertypes)) = self.supertypes.get(identity) else {
            return SDK_HIERARCHY
                .iter()
                .find(|(library, name, _, _)| identity.is_sdk(library, name))
                .map_or(Truth::Unknown, |(_, _, _, supertypes)| {
                    let mut result = Truth::No;
                    for (library, name) in *supertypes {
                        match self.identity_implements(
                            &DeclarationIdentity::sdk(library, name),
                            is_target,
                            visited,
                        ) {
                            Truth::Yes => return Truth::Yes,
                            Truth::Unknown => result = Truth::Unknown,
                            Truth::No => {}
                        }
                    }
                    result
                });
        };
        let mut result = Truth::No;
        for supertype in supertypes {
            let found = match supertype {
                ResolvedType::Interface { identity, .. } => {
                    self.identity_implements(identity, is_target, visited)
                }
                _ => Truth::Unknown,
            };
            match found {
                Truth::Yes => return Truth::Yes,
                Truth::Unknown => result = Truth::Unknown,
                Truth::No => {}
            }
        }
        result
    }

    /// The applicable extension's identity, preserving receiver nullability
    /// and giving instance members precedence over implicit extensions.
    pub fn resolved_extension_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        self.applicable_extension(receiver, name, model)?
            .0
            .identity
            .clone()
    }

    /// Borrows the stored extension spelling for a read or setter lookup.
    /// Setter names are indexed separately, so callers never construct `name=`.
    fn extension_access_name<'s>(&'s self, name: &'s str, write: bool) -> Option<&'s str> {
        if !write {
            return Some(name);
        }
        if let Some(index) = &self.extension_members {
            if let Some((stored, _)) = index[1].get_key_value(name) {
                return Some(&stored.0);
            }
        } else if let Some(stored) = self
            .extensions
            .iter()
            .flat_map(|extension| &extension.instance_members)
            .find(|stored| stored.strip_suffix('=') == Some(name))
        {
            return Some(stored);
        }
        self.external.as_ref()?.extension_access_name(name, write)
    }

    /// The applicable extension owner for a borrowed read or write member name.
    pub fn resolved_extension_access_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        write: bool,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        self.resolved_extension_owner(receiver, self.extension_access_name(name, write)?, model)
    }

    /// Applicable extension on-types without constructing setter spellings.
    pub fn applicable_extension_access_on_types(
        &self,
        receiver: &ResolvedType,
        name: &str,
        write: bool,
        model: &SemanticModel<'_>,
    ) -> Option<Vec<&ResolvedType>> {
        match self.extension_access_name(name, write) {
            Some(name) => self.applicable_extension_on_types(receiver, name, model),
            None => Some(Vec::new()),
        }
    }

    /// The unique most-specific visible extension among those declaring the
    /// instance member `name`, admitted by `admit`, that apply to `receiver`,
    /// with its inferred type arguments. `None` for dynamic or unknown
    /// receivers, when any candidate's applicability is unknown, or when no
    /// single candidate is more specific than every other.
    fn most_specific_extension(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
        admit: impl Fn(&ExtensionFacts) -> bool,
    ) -> Option<(&ExtensionFacts, HashMap<TypeParameterId, ResolvedType>)> {
        if matches!(receiver, ResolvedType::Dynamic | ResolvedType::Unknown) {
            return None;
        }
        let mut candidates: Vec<_> = self
            .extensions_with_member(name)
            .filter(|extension| {
                extension.instance_members.contains(name)
                    && model
                        .extension_visible(extension.declaring_library, extension.name.as_deref())
                    && admit(extension)
            })
            .filter_map(|extension| {
                let (on_type, applicability, substitutions) =
                    self.instantiated_extension_on_type(receiver, extension, model);
                (applicability != Truth::No).then_some((
                    extension,
                    on_type,
                    applicability,
                    substitutions,
                ))
            })
            .collect();
        let is_platform = |extension: &ExtensionFacts| {
            matches!(extension.identity, Some(DeclarationIdentity::Sdk { .. }))
                || extension
                    .declaring_library
                    .and_then(|library| model.identities.library_uri(library))
                    .is_some_and(|uri| uri.starts_with("dart:"))
        };
        // Any applicable non-platform extension takes priority over platform
        // extensions, regardless of their on-types' relative specificity.
        if candidates.iter().any(|(extension, _, applicability, _)| {
            *applicability == Truth::Yes && !is_platform(extension)
        }) {
            candidates.retain(|(extension, _, _, _)| !is_platform(extension));
        }
        if candidates
            .iter()
            .any(|(_, _, applicability, _)| *applicability == Truth::Unknown)
        {
            return None;
        }
        let mut selected = None;
        for (index, (candidate, on_type, _, _)) in candidates.iter().enumerate() {
            let most_specific = candidates.iter().all(|(other, other_on_type, _, _)| {
                if std::ptr::eq(*candidate, *other) {
                    return true;
                }
                match (on_type.as_ref(), other_on_type.as_ref()) {
                    (_, ResolvedType::Dynamic) => on_type != other_on_type,
                    _ => match (
                        self.subtype(on_type, other_on_type, model),
                        self.subtype(other_on_type, on_type, model),
                    ) {
                        (Truth::Yes, Truth::No) => true,
                        (Truth::Yes, Truth::Yes) => {
                            let candidate_bound =
                                self.extension_on_type_at_bounds(candidate, model);
                            let other_bound = self.extension_on_type_at_bounds(other, model);
                            self.subtype(&candidate_bound, &other_bound, model) == Truth::Yes
                                && self.subtype(&other_bound, &candidate_bound, model) == Truth::No
                        }
                        _ => false,
                    },
                }
            });
            if most_specific {
                if selected.is_some() {
                    return None;
                }
                selected = Some(index);
            }
        }
        let (extension, _, _, substitutions) = candidates.swap_remove(selected?);
        Some((extension, substitutions))
    }

    /// Whether a visible extension getter may apply and therefore have effects.
    pub fn has_possible_extension_getter(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> bool {
        let receiver = receiver.with_nullable(false);
        self.extensions_with_member(name).any(|extension| {
            extension.instance_members.contains(name)
                && !extension.methods.contains_key(name)
                && !name.ends_with('=')
                && model.extension_visible(extension.declaring_library, extension.name.as_deref())
                && self
                    .instantiated_extension_on_type(&receiver, extension, model)
                    .1
                    != Truth::No
        })
    }

    /// Facts for the members named `name` that `identity` itself declares.
    pub fn declared_member_facts(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> &[MemberFacts] {
        self.member_entry(identity, name)
            .map_or(&[], |entry| entry.facts.as_slice())
    }

    /// On-types of the visible extensions declaring the instance member `name`
    /// (setters as `name=`, operators by symbol) that apply to the non-nullable
    /// form of `receiver`; `None` when some candidate's applicability is unknown.
    pub fn applicable_extension_on_types(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<Vec<&ResolvedType>> {
        let receiver = receiver.with_nullable(false);
        let mut on_types = Vec::new();
        for extension in self.extensions_with_member(name) {
            if !extension.instance_members.contains(name)
                || !model.extension_visible(extension.declaring_library, extension.name.as_deref())
            {
                continue;
            }
            match self
                .instantiated_extension_on_type(&receiver, extension, model)
                .1
            {
                Truth::Yes => on_types.push(&extension.on_type),
                Truth::No => {}
                Truth::Unknown => return None,
            }
        }
        Some(on_types)
    }

    fn extension_on_type_at_bounds<'e>(
        &self,
        extension: &'e ExtensionFacts,
        model: &SemanticModel<'_>,
    ) -> Cow<'e, ResolvedType> {
        if extension.parameters.is_empty() {
            return self.normalized_raw_type(Cow::Borrowed(&extension.on_type));
        }
        let mut visiting = HashSet::new();
        let substitutions = extension
            .parameters
            .iter()
            .map(|id| {
                visiting.clear();
                (
                    id.clone(),
                    instantiate_parameter_bound(
                        id,
                        &extension.parameter_bounds,
                        &mut visiting,
                        model,
                    ),
                )
            })
            .collect();
        self.normalized_raw_type(Cow::Owned(
            model.substitute(&extension.on_type, &substitutions),
        ))
    }

    fn infer_extension_substitutions(
        &self,
        receiver: &ResolvedType,
        extension: &ExtensionFacts,
        model: &SemanticModel<'_>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
        let mut inferred = HashMap::with_capacity(extension.parameters.len());
        let mut upper = HashMap::new();
        if extension.parameters.is_empty() {
            return inferred;
        }
        let receiver = self.normalized_raw_type(Cow::Borrowed(receiver));
        infer_type_constraints(
            &extension.on_type,
            receiver.as_ref(),
            true,
            &mut TypeConstraintContext {
                parameters: &extension.parameters,
                inferred: &mut inferred,
                upper: &mut upper,
                signatures: self,
                model,
                poison_unknown: true,
                partial: None,
            },
        );
        let mut visiting = HashSet::new();
        for id in &extension.parameters {
            if !upper.contains_key(id) && inferred.contains_key(id) {
                continue;
            }
            let argument = inference_candidate(inferred.get(id), upper.get(id), self, model)
                .unwrap_or_else(|| {
                    if mentions_any(&extension.on_type, std::slice::from_ref(id)) {
                        return ResolvedType::Unknown;
                    }
                    visiting.clear();
                    instantiate_parameter_bound(
                        id,
                        &extension.parameter_bounds,
                        &mut visiting,
                        model,
                    )
                });
            if let Some(value) = inferred.get_mut(id) {
                *value = argument;
            } else {
                inferred.insert(id.clone(), argument);
            }
        }
        inferred
    }

    fn normalized_raw_type<'t>(&self, ty: Cow<'t, ResolvedType>) -> Cow<'t, ResolvedType> {
        if let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = ty.as_ref()
            && arguments.is_empty()
            && self
                .supertypes
                .get(identity)
                .is_some_and(|(parameters, _)| !parameters.is_empty())
        {
            Cow::Owned(self.with_raw_arguments(ty.as_ref()))
        } else {
            ty
        }
    }

    /// Infer the on-type before checking both the receiver and parameter bounds.
    /// A failed constraint is rejection; missing semantic facts remain unknown.
    fn instantiated_extension_on_type<'e>(
        &self,
        receiver: &ResolvedType,
        extension: &'e ExtensionFacts,
        model: &SemanticModel<'_>,
    ) -> (
        Cow<'e, ResolvedType>,
        Truth,
        HashMap<TypeParameterId, ResolvedType>,
    ) {
        let substitutions = self.infer_extension_substitutions(receiver, extension, model);
        // A missing argument fits a universal top bound, unless another
        // parameter's bound depends on its actual (still unknown) identity.
        // Proof substitutions must never escape into specificity or signatures.
        let mut proof = Cow::Borrowed(&substitutions);
        for (id, argument) in &substitutions {
            if matches!(argument, ResolvedType::Unknown)
                && matches!(
                    extension.parameter_bounds.get(id),
                    Some(ResolvedType::Dynamic)
                )
                && !extension.parameter_bounds.iter().any(|(other, bound)| {
                    other != id && mentions_any(bound, std::slice::from_ref(id))
                })
            {
                proof.to_mut().insert(id.clone(), ResolvedType::Dynamic);
            }
        }
        let on_type = self.normalized_raw_type(if extension.parameters.is_empty() {
            Cow::Borrowed(&extension.on_type)
        } else {
            Cow::Owned(model.substitute(&extension.on_type, &substitutions))
        });
        let proof_on_type = if matches!(proof, Cow::Owned(_)) {
            Some(self.normalized_raw_type(Cow::Owned(
                model.substitute(&extension.on_type, proof.as_ref()),
            )))
        } else {
            None
        };
        let checked_on_type = proof_on_type.as_deref().unwrap_or(on_type.as_ref());
        let receiver = self.normalized_raw_type(Cow::Borrowed(receiver));
        let mut applicability = if receiver.as_ref() == on_type.as_ref()
            && !has_inference_holes(&extension.on_type)
            && (!has_inference_holes(receiver.as_ref())
                || matches!(extension.on_type, ResolvedType::TypeParameter { .. }))
        {
            Truth::Yes
        } else {
            self.subtype_for_extension(receiver.as_ref(), checked_on_type, model, true)
        };
        for (id, bound) in &extension.parameter_bounds {
            let argument = &proof[id];
            let bound =
                self.normalized_raw_type(Cow::Owned(model.substitute(bound, proof.as_ref())));
            match self.subtype_for_extension(argument, bound.as_ref(), model, true) {
                Truth::No => return (on_type, Truth::No, substitutions),
                Truth::Unknown if applicability == Truth::Yes => applicability = Truth::Unknown,
                _ => {}
            }
        }
        (on_type, applicability, substitutions)
    }

    /// The on-type of the visible extension named `name`.
    pub fn visible_extension_on_type(
        &self,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<&ResolvedType> {
        self.all_extensions()
            .find(|extension| {
                extension.name.as_deref() == Some(name)
                    && model.extension_visible(extension.declaring_library, Some(name))
            })
            .map(|extension| &extension.on_type)
    }

    /// The declared on-type of a resolved explicit extension override.
    pub fn extension_on_type(&self, identity: &DeclarationIdentity) -> Option<&ResolvedType> {
        self.all_extensions()
            .find(|extension| extension.identity.as_ref() == Some(identity))
            .map(|extension| &extension.on_type)
    }

    /// The declared signature of method `name` of the named extension
    /// `identity`, as invoked through an explicit extension override.
    pub fn extension_override_method(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&ResolvedSignature> {
        self.all_extensions()
            .find(|extension| extension.identity.as_ref() == Some(identity))?
            .methods
            .get(name)
    }
    /// The superclass chain supplies inherited implementations, ending in
    /// `Object`; only when it lacks `name` does a member that an implemented
    /// interface or a mixin's `on` type declares make up the type's interface.
    fn resolved_instance_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        if !receiver.is_non_nullable() {
            return self
                .has_instance_declaration(&DeclarationIdentity::OBJECT, name, access)
                .then(|| model.core_type("Object", false));
        }
        self.resolved_owner_through(receiver, name, access, false, model)
            .or_else(|| {
                self.has_instance_declaration(&DeclarationIdentity::OBJECT, name, access)
                    .then(|| model.core_type("Object", false))
            })
            .or_else(|| self.combined_instance_owner(receiver, name, access, model))
    }

    /// The declaration owning the instance member `name` of `receiver` below
    /// `Object`, through the member supertypes (mixins, then the superclass)
    /// only, or through every supertype (implemented interfaces and a mixin's
    /// `on` types too) when `interfaces`.
    fn resolved_owner_through(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        interfaces: bool,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        fn visit(
            index: &SignatureIndex,
            receiver: &ResolvedType,
            name: &str,
            access: InstanceAccess,
            interfaces: bool,
            model: &SemanticModel<'_>,
            visited: &mut FxHashSet<DeclarationIdentity>,
        ) -> Option<ResolvedType> {
            // Nullable receivers expose Object's members, not the members of
            // their non-null branch. Object is handled by the caller.
            if !receiver.is_non_nullable() {
                return None;
            }
            if let ResolvedType::TypeParameter { bound, .. } = receiver {
                return visit(index, bound, name, access, interfaces, model, visited);
            }
            if let ResolvedType::NonNullTypeParameter { bound, .. } = receiver {
                return visit(
                    index,
                    &bound.non_null(),
                    name,
                    access,
                    interfaces,
                    model,
                    visited,
                );
            }
            let ResolvedType::Interface {
                occurrence,
                identity,
                arguments,
                ..
            } = receiver
            else {
                return None;
            };
            // Valid inheritance reaches each declaration with consistent
            // arguments. Keep its first instantiated owner, but never expand
            // the same declaration again along another diamond path.
            if identity.is_sdk("dart:core", "Object")
                || !visited.insert(identity.clone())
                || index.has_cyclic_supertypes(identity)
            {
                return None;
            }
            if model.member_visible(identity, name)
                && index.has_instance_declaration(identity, name, access)
            {
                return Some(receiver.clone());
            }
            let (parameters, supertypes) = index.supertypes.get(identity)?;
            let substitutions = index.owner_substitutions(parameters, arguments, Some(occurrence));
            let supertypes = if interfaces {
                supertypes
            } else {
                index.member_supertypes.get(identity)?
            };
            supertypes.iter().find_map(|supertype| {
                let instantiated = model.substitute(supertype, &substitutions);
                visit(
                    index,
                    &instantiated,
                    name,
                    access,
                    interfaces,
                    model,
                    visited,
                )
            })
        }
        visit(
            self,
            receiver,
            name,
            access,
            interfaces,
            model,
            &mut FxHashSet::default(),
        )
    }

    /// The declaration owning the instance member `name` read on `receiver`
    /// (field, getter or method tear-off).
    pub fn resolved_readable_member_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        self.resolved_access_owner(receiver, name, InstanceAccess::Read, model)
    }

    /// The declaration owning the instance member `name` written on
    /// `receiver` (settable field or setter).
    pub fn resolved_writable_member_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        self.resolved_access_owner(receiver, name, InstanceAccess::Write, model)
    }

    /// Resolves a `super` read or write, checking applied mixins before `extends`
    /// and excluding declarations on the enclosing class itself.
    pub fn resolved_super_member_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        write: bool,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        let ResolvedType::Interface { identity, .. } = receiver else {
            return None;
        };
        let access = if write {
            InstanceAccess::Write
        } else {
            InstanceAccess::Read
        };
        if self.type_kind(identity) == Some(TypeKind::Mixin) {
            let (_, constraints) = self.supertypes.get(identity)?;
            let count = *self.mixin_constraint_counts.get(identity)?;
            return constraints[..count]
                .iter()
                .find_map(|constraint| {
                    self.resolved_owner_through(constraint, name, access, false, model)
                })
                .or_else(|| {
                    self.has_instance_declaration(&DeclarationIdentity::OBJECT, name, access)
                        .then(|| model.core_type("Object", false))
                })
                .or_else(|| {
                    constraints.iter().find_map(|constraint| {
                        self.resolved_owner_through(constraint, name, access, true, model)
                    })
                })
                .and_then(|owner| match owner {
                    ResolvedType::Interface { identity, .. } => Some(identity),
                    _ => None,
                });
        }
        self.member_supertypes
            .get(identity)?
            .iter()
            .find_map(|supertype| {
                match self.resolved_owner_through(supertype, name, access, false, model)? {
                    ResolvedType::Interface { identity, .. } => Some(identity),
                    _ => None,
                }
            })
            .or_else(|| {
                self.has_instance_declaration(&DeclarationIdentity::OBJECT, name, access)
                    .then_some(DeclarationIdentity::OBJECT)
            })
            .or_else(|| {
                self.supertypes
                    .get(identity)?
                    .1
                    .iter()
                    .find_map(|supertype| {
                        match self.resolved_owner_through(supertype, name, access, true, model)? {
                            ResolvedType::Interface { identity, .. } => Some(identity),
                            _ => None,
                        }
                    })
            })
    }

    fn resolved_access_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        match self.resolved_instance_owner(receiver, name, access, model)? {
            ResolvedType::Interface { identity, .. } => Some(identity),
            _ => None,
        }
    }

    pub fn resolved_member_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        let ResolvedType::Interface { identity, .. } =
            self.resolved_instance_owner(receiver, name, InstanceAccess::Invoke, model)?
        else {
            return None;
        };
        Some(identity)
    }

    pub fn resolved_member_facts(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<&[MemberFacts]> {
        let owner = self.resolved_member_owner(receiver, name, model)?;
        let facts = &self.member_entry(&owner, name)?.facts;
        (!facts.is_empty()).then_some(facts.as_slice())
    }

    pub fn is_promotable_field(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> bool {
        let Some(ResolvedType::Interface { identity, .. }) =
            self.resolved_instance_owner(receiver, name, InstanceAccess::Read, model)
        else {
            return false;
        };
        if !self
            .member_entry(&identity, name)
            .is_some_and(|entry| entry.promotable_field)
        {
            return false;
        }
        let DeclarationIdentity::Project { library, .. } = identity else {
            return false;
        };
        !self
            .blocked_field_promotions
            .get(&library)
            .is_some_and(|names| names.contains(name))
    }

    /// Whether the instance member `name` that `receiver` resolves to, or a
    /// declaration of `name` in any supertype of its declaring type, carries
    /// an annotation matching `annotation` (the analyzer's
    /// `isAwaitNotRequired`-style inherited-annotation lookup).
    pub fn resolved_member_annotated(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
        annotation: impl Fn(&DeclarationIdentity) -> bool,
    ) -> bool {
        let Some(owner) = self.resolved_member_owner(receiver, name, model) else {
            return self
                .applicable_extension(receiver, name, model)
                .and_then(|(extension, _)| extension.annotations.get(name))
                .is_some_and(|annotations| annotations.iter().any(annotation));
        };
        let annotated = |facts: &[MemberFacts]| {
            facts
                .iter()
                .any(|fact| !fact.is_static && fact.annotations.iter().any(&annotation))
        };
        self.member_entry(&owner, name)
            .is_some_and(|entry| annotated(&entry.facts))
            || self
                .inherited_member_facts(&owner, name)
                .is_some_and(|facts| annotated(&facts))
    }

    /// Resolve a static read's declaration, including interface members provided
    /// by a class's `noSuchMethod` implementation.
    pub fn resolved_interface_member_facts(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<&[MemberFacts]> {
        if let Some(facts) = self.resolved_member_facts(receiver, name, model) {
            return Some(facts);
        }
        self.interface_member_facts(receiver, name, model, &mut HashSet::new())
    }

    fn interface_member_facts(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> Option<&[MemberFacts]> {
        let ResolvedType::Interface { identity, .. } = receiver else {
            return None;
        };
        if !visited.insert(identity.clone()) {
            return None;
        }
        if model.member_visible(identity, name)
            && self.has_instance_declaration(identity, name, InstanceAccess::Read)
            && let Some(entry) = self.member_entry(identity, name)
            && !entry.facts.is_empty()
        {
            return Some(&entry.facts);
        }
        self.supertypes
            .get(identity)?
            .1
            .iter()
            .find_map(|supertype| self.interface_member_facts(supertype, name, model, visited))
    }

    fn owner_substitutions(
        &self,
        parameters: &[TypeParameterId],
        arguments: &[ResolvedType],
        occurrence: Option<&FunctionTypeAlias>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
        parameters
            .iter()
            .enumerate()
            .map(|(index, id)| {
                (
                    id.clone(),
                    arguments
                        .get(index)
                        .or_else(|| self.owner_defaults.get(id))
                        .cloned()
                        .map(|ty| match occurrence {
                            Some(owner) => ty.projected_from(owner),
                            None => ty,
                        })
                        .unwrap_or(ResolvedType::Dynamic),
                )
            })
            .collect()
    }

    /// The supertype of `this_type` through which `super.name` resolves: its
    /// mixins (last first), then its superclass; for a mixin, its `on` types.
    pub fn super_receiver(
        &self,
        this_type: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.super_receiver_accesses(
            this_type,
            name,
            &[InstanceAccess::Read, InstanceAccess::Invoke],
            model,
        )
    }

    /// The supertype through which a `super.name` assignment resolves.
    pub fn super_receiver_for_write(
        &self,
        this_type: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.super_receiver_accesses(this_type, name, &[InstanceAccess::Write], model)
    }

    fn super_receiver_accesses(
        &self,
        this_type: &ResolvedType,
        name: &str,
        accesses: &[InstanceAccess],
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let ResolvedType::Interface {
            occurrence,
            identity,
            arguments,
            ..
        } = this_type
        else {
            return None;
        };
        let (parameters, supertypes) = self.supertypes.get(identity)?;
        let substitutions = self.owner_substitutions(parameters, arguments, Some(occurrence));
        let supertypes = if self.type_kind(identity) == Some(TypeKind::Mixin) {
            supertypes
        } else {
            self.member_supertypes.get(identity)?
        };
        let supertypes: Vec<_> = supertypes
            .iter()
            .map(|supertype| model.substitute(supertype, &substitutions))
            .collect();
        // Every candidate's implementation chain precedes any interface.
        let position = supertypes
            .iter()
            .position(|supertype| {
                accesses.iter().copied().any(|access| {
                    self.resolved_owner_through(supertype, name, access, false, model)
                        .is_some()
                })
            })
            .or_else(|| {
                supertypes.iter().position(|supertype| {
                    accesses.iter().copied().any(|access| {
                        self.resolved_instance_owner(supertype, name, access, model)
                            .is_some()
                    })
                })
            })?;
        supertypes.into_iter().nth(position)
    }

    pub fn resolved_member(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(&ResolvedSignature, HashMap<TypeParameterId, ResolvedType>)> {
        let Some(owner) =
            self.resolved_instance_owner(receiver, name, InstanceAccess::Invoke, model)
        else {
            return self.resolved_extension_method(receiver, name, model);
        };
        let ResolvedType::Interface {
            occurrence,
            identity,
            arguments,
            ..
        } = owner
        else {
            return None;
        };
        let signature = self.instance_member(&identity, name)?;
        let substitutions =
            self.owner_substitutions(&signature.owner_parameters, &arguments, Some(&occurrence));
        Some((signature, substitutions))
    }

    /// The most specific visible extension declaring instance member `name`
    /// that applies to `receiver`. Instance members, including method
    /// tear-offs, take precedence. `None` when no extension declares it, one's
    /// applicability is unknown, or no single candidate is most specific.
    fn applicable_extension(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(&ExtensionFacts, HashMap<TypeParameterId, ResolvedType>)> {
        let extension = self.most_specific_extension(receiver, name, model, |_| true)?;
        (self.resolved_instance_owner(receiver, name, InstanceAccess::Read, model).is_none()
            // An incompatible interface still declares an instance member;
            // an extension cannot repair its missing combined signature.
            && self.resolved_owner_through(receiver, name, InstanceAccess::Read, true, model).is_none())
            .then_some(extension)
    }

    /// The declared signature of the extension method `name` invoked on
    /// `receiver`, with its optional named identity and declaring library.
    pub fn extension_method(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(
        Option<&DeclarationIdentity>,
        Option<LibraryId>,
        &ResolvedSignature,
    )> {
        let (extension, _) = self.applicable_extension(receiver, name, model)?;
        Some((
            extension.identity.as_ref(),
            extension.declaring_library,
            extension.methods.get(name)?,
        ))
    }

    /// An annotation on a named extension's explicitly selected member.
    pub fn extension_member_annotated(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        annotation: impl Fn(&DeclarationIdentity) -> bool,
    ) -> bool {
        self.all_extensions()
            .find(|extension| extension.identity.as_ref() == Some(identity))
            .and_then(|extension| extension.annotations.get(name))
            .is_some_and(|annotations| annotations.iter().any(annotation))
    }

    /// The method `name` of the single visible extension that applies to
    /// `receiver`, with the extension's type parameters inferred from matching
    /// its `on` type against the receiver. `None` when no extension, several,
    /// or one whose applicability or instantiation is unknown, declares it.
    fn resolved_extension_method(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(&ResolvedSignature, HashMap<TypeParameterId, ResolvedType>)> {
        let (extension, substitutions) = self.applicable_extension(receiver, name, model)?;
        Some((extension.methods.get(name)?, substitutions))
    }

    /// Resolve an explicitly selected extension method, inferring its owner's
    /// type arguments from the override receiver unless they are supplied.
    pub fn resolved_extension_override_method(
        &self,
        identity: &DeclarationIdentity,
        receiver: &ResolvedType,
        type_args: &[ResolvedType],
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(&ResolvedSignature, HashMap<TypeParameterId, ResolvedType>)> {
        let extension = self
            .all_extensions()
            .find(|extension| extension.identity.as_ref() == Some(identity))?;
        if type_args.is_empty() {
            self.resolve_extension_signature(extension, receiver, name, model)
        } else if type_args.len() == extension.parameters.len() {
            Some((
                extension.methods.get(name)?,
                extension
                    .parameters
                    .iter()
                    .cloned()
                    .zip(type_args.iter().cloned())
                    .collect(),
            ))
        } else {
            None
        }
    }

    /// The getter type of the single applicable visible extension.
    pub fn resolved_extension_getter(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(ResolvedType, HashMap<TypeParameterId, ResolvedType>)> {
        let (extension, substitutions) = self.applicable_extension(receiver, name, model)?;
        Some((extension.getters.get(name)?.clone(), substitutions))
    }

    fn resolve_extension_signature<'a>(
        &self,
        extension: &'a ExtensionFacts,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(
        &'a ResolvedSignature,
        HashMap<TypeParameterId, ResolvedType>,
    )> {
        let substitutions = self.infer_extension_substitutions(receiver, extension, model);
        Some((extension.methods.get(name)?, substitutions))
    }

    /// Resolve an instance field or getter, never an extension getter.
    pub fn resolved_field(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(ResolvedType, HashMap<TypeParameterId, ResolvedType>)> {
        let owner = self.resolved_instance_owner(receiver, name, InstanceAccess::Read, model)?;
        let ResolvedType::Interface {
            occurrence,
            identity,
            arguments,
            ..
        } = owner
        else {
            return None;
        };
        let field = self.member_entry(&identity, name)?.read.as_ref()?;
        let (parameters, _) = self.supertypes.get(&identity)?;
        let substitutions = self.owner_substitutions(parameters, &arguments, Some(&occurrence));
        Some((field.clone(), substitutions))
    }

    pub fn resolved_writable_field(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(ResolvedType, HashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType::Interface {
            occurrence,
            identity,
            arguments,
            ..
        } = self.resolved_instance_owner(receiver, name, InstanceAccess::Write, model)?
        else {
            return None;
        };
        let field = self.member_entry(&identity, name)?.write.as_ref()?;
        let (parameters, _) = self.supertypes.get(&identity)?;
        let substitutions = self.owner_substitutions(parameters, &arguments, Some(&occurrence));
        Some((field.clone(), substitutions))
    }

    pub fn field(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&(Vec<TypeParameterId>, ResolvedType)> {
        self.member_entry(identity, name)?.field.as_ref()
    }
}

fn constructed_identity(
    expression: &Expr,
    model: &SemanticModel<'_>,
) -> Option<DeclarationIdentity> {
    match expression.unparenthesized() {
        Expr::New {
            dart_type,
            constructor_name,
            ..
        } => resolve_new_constructor(dart_type, constructor_name.as_ref(), model)
            .map(|(identity, _)| identity),
        Expr::Call { callee, .. } => {
            resolve_call_constructor(callee, model, false).map(|(identity, _)| identity)
        }
        _ => None,
    }
}

pub fn expression_segments(
    expression: &Expr,
    allow_generic_instantiation: bool,
) -> Option<Vec<&str>> {
    let mut current = expression;
    let mut segments = Vec::new();
    loop {
        match current.unparenthesized() {
            Expr::Ident(identifier) => {
                segments.push(identifier.name.as_str());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field { object, field, .. } => {
                segments.push(field.name.as_str());
                current = object;
            }
            Expr::GenericInstantiation { target, .. } if allow_generic_instantiation => {
                current = target;
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
    callee: &'a Expr,
    model: &SemanticModel<'_>,
    allow_generic_instantiation: bool,
) -> Option<(DeclarationIdentity, &'a str)> {
    let mut segments = expression_segments(callee, allow_generic_instantiation)?;
    if let Some(identity) = model.resolve_dotted(&segments) {
        return Some((identity, "new"));
    }
    let constructor = segments.pop()?;
    Some((model.resolve_dotted(&segments)?, constructor))
}

/// Resolve an explicit `new`/`const` constructor without assuming a dotted
/// type name denotes an import prefix rather than a named constructor.
pub fn resolve_new_constructor<'a>(
    dart_type: &'a DartType,
    constructor_name: Option<&'a falcon_syntax::ast::Identifier>,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str)> {
    let DartType::Named(named) = dart_type else {
        return None;
    };
    if let Some(identity) =
        QualifiedName::from_identifiers(&named.segments).and_then(|name| model.resolve_name(name))
    {
        return Some((
            identity,
            constructor_name.map_or("new", |name| name.name.as_str()),
        ));
    }
    if constructor_name.is_some() {
        return None;
    }
    let (constructor, qualifier) = named.segments.split_last()?;
    Some((
        QualifiedName::from_identifiers(qualifier).and_then(|name| model.resolve_name(name))?,
        constructor.name.as_str(),
    ))
}

fn annotation_targets_library(
    annotation: &falcon_syntax::ast::Annotation,
    model: &SemanticModel<'_>,
) -> bool {
    if !matches!(
        QualifiedName::from_identifiers(&annotation.name).and_then(|name| model.resolve_imported_member(name)),
        Some(identity) if identity.is_package("meta", "Target")
    ) {
        return false;
    }
    let Some(args) = &annotation.args else {
        return false;
    };
    args.positional
        .iter()
        .chain(args.named.iter().map(|argument| &argument.value))
        .any(|expression| target_kind_library(expression, model))
}

fn target_kind_library(expression: &Expr, model: &SemanticModel<'_>) -> bool {
    match expression.unparenthesized() {
        Expr::Field { object, field, .. } if field.name == "library" => {
            let Some(segments) = expression_segments(object, false) else {
                return false;
            };
            matches!(
                QualifiedName::from_segments(&segments).and_then(|name| model.resolve_imported_member(name)),
                Some(identity) if identity.is_package("meta", "TargetKind")
            )
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            elements.iter().any(|element| {
                matches!(element, falcon_syntax::ast::CollectionElement::Expr(expression)
                if target_kind_library(expression, model))
            })
        }
        _ => false,
    }
}

fn infer_const_initializer_type(
    initializer: &Expr,
    model: &SemanticModel<'_>,
    scope: &TypeParameterScope,
) -> ResolvedType {
    match initializer {
        Expr::New { dart_type, .. } => model.resolve_type_in(dart_type, scope).with_nullable(false),
        Expr::Call { callee, .. } => resolve_call_constructor(callee, model, false)
            .map_or(ResolvedType::Unknown, |(identity, _)| {
                ResolvedType::interface_identity(identity)
            }),
        Expr::Conditional {
            then_expr,
            else_expr,
            ..
        } => {
            let left = infer_const_initializer_type(then_expr, model, scope);
            let right = infer_const_initializer_type(else_expr, model, scope);
            if left == right {
                left
            } else {
                ResolvedType::Unknown
            }
        }
        Expr::IntLit { .. } => model.core_type("int", false),
        Expr::DoubleLit { .. } => model.core_type("double", false),
        Expr::StringLit(_) => model.core_type("String", false),
        Expr::BoolLit { .. } => model.core_type("bool", false),
        _ => ResolvedType::Unknown,
    }
}

fn positional_parameter_names(params: &FormalParamList) -> Vec<String> {
    params
        .positional()
        .map(|parameter| parameter.name.name.clone())
        .collect()
}

fn same_library(left: &DeclarationIdentity, right: &DeclarationIdentity) -> bool {
    matches!(
        (left, right),
        (
            DeclarationIdentity::Project { library: left, .. },
            DeclarationIdentity::Project { library: right, .. }
        ) if left == right
    ) || matches!(
        (left, right),
        (
            DeclarationIdentity::Sdk { library: left, .. },
            DeclarationIdentity::Sdk { library: right, .. }
        ) if left == right
    ) || matches!(
        (left, right),
        (
            DeclarationIdentity::Package {
                package: left,
                copy: left_copy,
                ..
            },
            DeclarationIdentity::Package {
                package: right,
                copy: right_copy,
                ..
            }
        ) if left == right && left_copy == right_copy
    )
}

/// Whether an instance callable omits a type supplied by override inference.
fn inherits_method_types(member: &ClassMember) -> bool {
    let (params, infer_return) = match member {
        ClassMember::Method(method) if !method.is_static => {
            (&method.params, method.return_type.is_none())
        }
        ClassMember::Operator(operator) => (&operator.params, operator.return_type.is_none()),
        ClassMember::Setter(setter) => return !setter.is_static && setter.param_type.is_none(),
        _ => return false,
    };
    infer_return
        || params.params.iter().any(|param| {
            param.param_type.is_none()
                && param.function_params.is_none()
                && param.initializing.is_none()
        })
}

fn signature(
    scope: &TypeParameterScope,
    owner_parameters: &[TypeParameterId],
    call_parameters: &[TypeParam],
    params: Option<&FormalParamList>,
    return_type: Option<&DartType>,
    field_types: Option<&HashMap<String, ResolvedType>>,
    model: &SemanticModel<'_>,
) -> ResolvedSignature {
    let parameter_type = |param: &FormalParam| {
        if param.param_type.is_some() || param.function_params.is_some() {
            model.resolve_formal_parameter_in(param, scope)
        } else if param.initializing == Some(InitializingFormal::This) {
            field_types
                .and_then(|fields| fields.get(&param.name.name))
                .cloned()
                .unwrap_or(ResolvedType::Unknown)
        } else if param.initializing == Some(InitializingFormal::Super) {
            ResolvedType::Unknown
        } else {
            ResolvedType::Dynamic
        }
    };
    let optional = |param: &FormalParam| {
        matches!(
            param.kind,
            ParamKind::OptionalPositional | ParamKind::Named { required: false }
        )
    };
    // Super formals without a declared default inherit theirs once the super
    // constructor resolves (`resolve_constructor_formals`).
    let default = |param: &FormalParam| match &param.default_value {
        Some(value) => Some(ParameterDefault::Declared(Arc::new(value.clone()))),
        None if optional(param) && param.initializing != Some(InitializingFormal::Super) => {
            Some(ParameterDefault::Null)
        }
        None => None,
    };
    let has_default = |param: &FormalParam| param.default_value.is_some() || optional(param);
    ResolvedSignature {
        optional_positional: params
            .into_iter()
            .flat_map(|params| params.positional())
            .filter(|param| param.kind == ParamKind::OptionalPositional)
            .count(),
        owner_parameters: owner_parameters.to_vec(),
        call_parameters: parameter_ids(scope, call_parameters),
        shape: FunctionTypeShape::from_scope(
            params
                .into_iter()
                .flat_map(|params| params.named())
                .filter(|param| matches!(param.kind, ParamKind::Named { required: true }))
                .map(|param| param.name.name.clone())
                .collect(),
            call_parameters,
            scope,
        ),
        positional_parameter_names: params
            .filter(|params| params.positional().any(has_default))
            .map_or_else(Vec::new, |params| {
                params
                    .positional()
                    .map(|param| {
                        if has_default(param) {
                            param.name.name.clone()
                        } else {
                            String::new()
                        }
                    })
                    .collect()
            }),
        non_optional_defaults: params
            .into_iter()
            .flat_map(|params| &params.params)
            .filter(|param| {
                has_default(param)
                    && (!optional(param)
                        || param.annotations.iter().any(|annotation| {
                            annotation
                                .name
                                .last()
                                .is_some_and(|name| name.name == "required")
                        }))
            })
            .map(|param| param.name.name.clone())
            .collect(),
        positional: params
            .into_iter()
            .flat_map(|params| params.positional())
            .map(&parameter_type)
            .collect(),
        named: params
            .into_iter()
            .flat_map(|params| params.named())
            .map(|param| (param.name.name.clone(), parameter_type(param)))
            .collect(),
        defaults: params
            .into_iter()
            .flat_map(|params| &params.params)
            .filter_map(|param| Some((param.name.name.clone(), default(param)?)))
            .collect(),
        return_type: return_type
            .map(|ty| model.resolve_type_in(ty, scope))
            .unwrap_or(ResolvedType::Dynamic),
    }
}

/// Whether `ty` mentions any of `parameters`.
fn mentions_any(ty: &ResolvedType, parameters: &[TypeParameterId]) -> bool {
    if parameters.is_empty() {
        return false;
    }
    find_type_including_bounds(
        ty,
        &mut |ty| {
            matches!(
                ty,
                ResolvedType::TypeParameter { id, .. }
                    | ResolvedType::NonNullTypeParameter { id, .. } if parameters.contains(id)
            )
        },
        &mut FxHashSet::default(),
        true,
    )
    .is_some()
}

fn parameter_ids(scope: &TypeParameterScope, parameters: &[TypeParam]) -> Vec<TypeParameterId> {
    parameters
        .iter()
        .filter_map(|parameter| match scope.lookup(&parameter.name.name) {
            Some(ResolvedType::TypeParameter { id, .. }) => Some(id),
            _ => None,
        })
        .collect()
}

/// Records whether the declaration of each of `parameters` states a bound.
fn record_bound_declarations(
    declared: &mut FxHashMap<TypeParameterId, bool>,
    scope: &TypeParameterScope,
    parameters: &[TypeParam],
) {
    for parameter in parameters {
        if let Some(ResolvedType::TypeParameter { id, .. }) = scope.lookup(&parameter.name.name) {
            declared.insert(id, parameter.bound.is_some());
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct FlowBinding {
    declared: ResolvedType,
    /// Source identity of an AST binding, absent for synthetic bindings.
    declaration: Option<Span>,
    /// Dart's promotion chain: each type a subtype of the previous; the last is current.
    promoted: Vec<ResolvedType>,
    /// Types tested or promoted to, kept across writes as types of interest.
    tested: Vec<ResolvedType>,
    promotable: bool,
    captured: bool,
    local_function: bool,
    fields: HashMap<String, ResolvedType>,
}

impl FlowBinding {
    fn push_promotion(&mut self, ty: ResolvedType) {
        if !self.tested.contains(&ty) {
            self.tested.push(ty.clone());
        }
        if self.promoted.last() != Some(&ty) {
            self.promoted.push(ty);
        }
    }
}

/// Lexical flow state with copy-on-write scopes for independent branch snapshots.
#[derive(Debug, Clone, Default)]
pub struct TypeEnvironment {
    // Branch snapshots share scopes until a write touches that scope.
    scopes: Vec<Arc<FxHashMap<String, FlowBinding>>>,
    callable_boundaries: Vec<usize>,
    /// Whether the library's language version promotes private fields.
    field_promotions: bool,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        Self {
            scopes: vec![Arc::new(FxHashMap::default())],
            callable_boundaries: Vec::new(),
            field_promotions: false,
        }
    }

    /// An empty environment whose expression inference promotes private
    /// fields when `field_promotions`.
    pub fn with_field_promotions(field_promotions: bool) -> Self {
        Self {
            field_promotions,
            ..Self::new()
        }
    }

    /// The flow analysis promoting within expressions this environment infers.
    fn flow<'a, 'model>(
        &self,
        model: &'a SemanticModel<'model>,
        signatures: &'a SignatureIndex,
        type_parameters: &'a TypeParameterScope,
    ) -> super::flow::FlowContext<'a, 'model> {
        super::flow::FlowContext {
            model,
            signatures,
            type_parameters,
            field_promotions: self.field_promotions,
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Arc::new(FxHashMap::default()));
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn declare(&mut self, name: impl Into<String>, ty: ResolvedType) {
        let name = name.into();
        if let ResolvedType::NonNullTypeParameter { id, bound } = ty {
            // Inferred locals declare `T`, then promote to `T & Object`.
            // A later write of `T` must be able to discard the intersection.
            let declared = ResolvedType::TypeParameter {
                id: id.clone(),
                bound: bound.clone(),
                nullable: false,
            };
            self.declare_binding(name, declared, true)
                .promoted
                .push(ResolvedType::NonNullTypeParameter { id, bound });
        } else {
            self.declare_binding(name, ty, true);
        }
    }

    /// Declares an AST binding, retaining its lexical declaration identity.
    pub fn declare_at(&mut self, name: impl Into<String>, ty: ResolvedType, span: Span) {
        self.declare_binding(name.into(), ty, true).declaration = Some(span);
    }

    /// The declaration of the nearest visible binding, not its reusable scope depth.
    pub fn binding_declaration(&self, name: &str) -> Option<Span> {
        self.binding(name)?.declaration
    }

    pub fn declare_unpromotable(&mut self, name: impl Into<String>, ty: ResolvedType) {
        self.declare_binding(name.into(), ty, false);
    }

    fn declare_binding(
        &mut self,
        name: String,
        ty: ResolvedType,
        promotable: bool,
    ) -> &mut FlowBinding {
        Arc::make_mut(self.scopes.last_mut().expect("root scope"))
            .entry(name)
            .insert_entry(FlowBinding {
                declared: ty,
                declaration: None,
                promoted: Vec::new(),
                tested: Vec::new(),
                promotable,
                captured: false,
                local_function: false,
                fields: HashMap::new(),
            })
            .into_mut()
    }
    pub fn declare_local_function(&mut self, name: impl Into<String>, ty: ResolvedType) {
        self.declare_binding(name.into(), ty, true).local_function = true;
    }

    /// Declares a local function with the identity of its source name.
    pub fn declare_local_function_at(
        &mut self,
        name: impl Into<String>,
        ty: ResolvedType,
        span: Span,
    ) {
        let binding = self.declare_binding(name.into(), ty, true);
        binding.local_function = true;
        binding.declaration = Some(span);
    }

    pub fn is_local_function(&self, name: &str) -> bool {
        self.binding(name)
            .is_some_and(|binding| binding.local_function)
    }

    pub fn mark_captured(&mut self, name: &str) {
        if self.binding(name).is_some_and(|binding| {
            !binding.captured
                || binding.promotable
                || !binding.promoted.is_empty()
                || !binding.fields.is_empty()
        }) && let Some(binding) = self.binding_mut(name)
        {
            binding.captured = true;
            binding.promotable = false;
            binding.promoted.clear();
            binding.fields.clear();
        }
    }

    pub fn promoted_this_field(&self, field: &str) -> Option<ResolvedType> {
        let scope = self
            .scopes
            .iter()
            .rev()
            .find(|scope| scope.contains_key("this"))?;
        if let Some(binding) = scope.get(field) {
            return binding.promoted.last().cloned();
        }
        scope.get("this")?.fields.get(field).cloned()
    }

    pub fn promote_this_field(&mut self, field: &str, ty: ResolvedType) {
        if let Some(scope) = self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key("this"))
        {
            if let Some(binding) = scope.get(field) {
                if !binding.promotable
                    || binding.captured
                    || (binding.promoted.last() == Some(&ty) && binding.tested.contains(&ty))
                {
                    return;
                }
            } else if scope
                .get("this")
                .is_some_and(|binding| binding.fields.get(field) == Some(&ty))
            {
                return;
            }
            let scope = Arc::make_mut(scope);
            if let Some(binding) = scope.get_mut(field) {
                if binding.promotable && !binding.captured {
                    binding.push_promotion(ty);
                }
            } else if let Some(binding) = scope.get_mut("this") {
                binding.fields.insert(field.to_owned(), ty);
            }
        }
    }

    fn binding(&self, name: &str) -> Option<&FlowBinding> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }

    fn binding_mut(&mut self, name: &str) -> Option<&mut FlowBinding> {
        let scope = self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key(name))?;
        Arc::make_mut(scope).get_mut(name)
    }

    pub fn declared_type(&self, name: &str) -> ResolvedType {
        self.binding(name)
            .map(|binding| binding.declared.clone())
            .unwrap_or(ResolvedType::Unknown)
    }

    /// The promotion chain of `name`, most general first.
    pub fn promotion_chain(&self, name: &str) -> &[ResolvedType] {
        self.binding(name)
            .map_or(&[], |binding| binding.promoted.as_slice())
    }

    /// The types of interest recorded for `name`, besides its declared type.
    pub fn tested_types(&self, name: &str) -> &[ResolvedType] {
        self.binding(name)
            .map_or(&[], |binding| binding.tested.as_slice())
    }

    /// Records the actual tested type, independently of the branch's promotion.
    pub(crate) fn record_tested_type(&mut self, name: &str, ty: ResolvedType) {
        if ty != ResolvedType::Unknown
            && self
                .binding(name)
                .is_some_and(|binding| !binding.tested.contains(&ty))
            && let Some(binding) = self.binding_mut(name)
        {
            binding.tested.push(ty);
        }
    }

    /// Applies a flow refinement without making it a tested type of interest.
    pub(crate) fn promote_without_test(&mut self, name: &str, ty: ResolvedType) {
        if ty != ResolvedType::Unknown
            && self.binding(name).is_some_and(|binding| {
                binding.promotable && !binding.captured && binding.promoted.last() != Some(&ty)
            })
            && let Some(binding) = self.binding_mut(name)
        {
            binding.promoted.push(ty);
        }
    }

    /// Records a write of `name` whose promotion chain becomes `promoted`.
    pub fn assign(&mut self, name: &str, promoted: Vec<ResolvedType>) -> bool {
        self.record_write(name);
        let Some(binding) = self.binding_mut(name) else {
            return false;
        };
        binding.fields.clear();
        if binding.promotable && !binding.captured {
            binding.promoted = promoted;
        } else {
            binding.promoted.clear();
        }
        true
    }

    pub fn promote(&mut self, name: &str, ty: ResolvedType) {
        if ty != ResolvedType::Unknown
            && self.binding(name).is_some_and(|binding| {
                binding.promotable
                    && !binding.captured
                    && (binding.promoted.last() != Some(&ty) || !binding.tested.contains(&ty))
            })
            && let Some(binding) = self.binding_mut(name)
        {
            binding.push_promotion(ty);
        }
    }

    pub fn demote(&mut self, name: &str) {
        self.record_write(name);
        self.forget(name);
    }

    /// Drops the promotions of `name` without recording a write.
    pub fn forget(&mut self, name: &str) {
        if self
            .binding(name)
            .is_some_and(|binding| !binding.promoted.is_empty() || !binding.fields.is_empty())
            && let Some(binding) = self.binding_mut(name)
        {
            binding.promoted.clear();
            binding.fields.clear();
        }
    }

    /// The state after `try`/`finally`: `self` (after the body and catches)
    /// with every binding the `finally` block changed, from `entry` to `end`,
    /// taken from `end`.
    pub fn attach_finally(&mut self, entry: &Self, end: &Self) {
        for (index, scope) in self.scopes.iter_mut().enumerate() {
            if entry
                .scopes
                .get(index)
                .zip(end.scopes.get(index))
                .is_some_and(|(before, after)| Arc::ptr_eq(before, after))
            {
                continue;
            }
            let changed = scope.keys().any(|name| {
                let before = entry.scopes.get(index).and_then(|scope| scope.get(name));
                let after = end.scopes.get(index).and_then(|scope| scope.get(name));
                after.is_some() && before != after
            });
            if !changed {
                continue;
            }
            for (name, binding) in Arc::make_mut(scope) {
                let before = entry.scopes.get(index).and_then(|scope| scope.get(name));
                if let Some(after) = end.scopes.get(index).and_then(|scope| scope.get(name))
                    && before != Some(after)
                {
                    binding.clone_from(after);
                }
            }
        }
    }

    pub fn enter_callable(&mut self) {
        self.callable_boundaries.push(self.scopes.len());
    }

    fn record_write(&mut self, name: &str) {
        if let Some(boundary) = self.callable_boundaries.last()
            && let Some(index) = self.binding_depth(name)
            && index < *boundary
            && self.scopes[index]
                .get(name)
                .is_some_and(|binding| !binding.captured)
            && let Some(binding) = Arc::make_mut(&mut self.scopes[index]).get_mut(name)
        {
            binding.captured = true;
        }
    }

    pub fn demote_captured(&mut self) {
        for scope in &mut self.scopes {
            if !scope.values().any(|binding| {
                binding.captured && (!binding.promoted.is_empty() || !binding.fields.is_empty())
            }) {
                continue;
            }
            for binding in Arc::make_mut(scope).values_mut() {
                if binding.captured {
                    binding.promoted.clear();
                    binding.fields.clear();
                }
            }
        }
    }

    fn receiver_name(object: &Expr) -> Option<&str> {
        match object {
            Expr::Ident(identifier) => Some(&identifier.name),
            Expr::This { .. } => Some("this"),
            _ => None,
        }
    }

    pub fn promoted_field(&self, object: &Expr, field: &str) -> Option<ResolvedType> {
        if matches!(object, Expr::This { .. }) {
            return self.promoted_this_field(field);
        }
        self.binding(Self::receiver_name(object)?)?
            .fields
            .get(field)
            .cloned()
    }

    pub fn promote_field(&mut self, object: &Expr, field: &str, ty: ResolvedType) {
        if matches!(object, Expr::This { .. }) {
            self.promote_this_field(field, ty);
            return;
        }
        if let Some(name) = Self::receiver_name(object)
            && self.binding(name).is_some_and(|binding| {
                !binding.captured && binding.promotable && binding.fields.get(field) != Some(&ty)
            })
            && let Some(binding) = self.binding_mut(name)
        {
            binding.fields.insert(field.to_owned(), ty);
        }
    }

    pub fn join(&self, other: &Self) -> Self {
        let mut joined = self.clone();
        for (index, scope) in joined.scopes.iter_mut().enumerate() {
            // Identical snapshots already share every promotion and type of interest.
            // Capture may still disable promotion even when both branches agree.
            if other
                .scopes
                .get(index)
                .is_some_and(|peer| Arc::ptr_eq(scope, peer))
                && scope
                    .values()
                    .all(|binding| !binding.captured || !binding.promotable)
            {
                continue;
            }
            for (name, binding) in Arc::make_mut(scope) {
                let peer = other.scopes.get(index).and_then(|scope| scope.get(name));
                binding.captured |= peer.is_some_and(|peer| peer.captured);
                if binding.captured {
                    binding.promotable = false;
                }
                match peer {
                    // Dart keeps the types both promotion chains share, in order.
                    Some(peer) => {
                        binding.promoted.retain(|ty| peer.promoted.contains(ty));
                        for ty in &peer.tested {
                            if !binding.tested.contains(ty) {
                                binding.tested.push(ty.clone());
                            }
                        }
                    }
                    None => binding.promoted.clear(),
                }
                binding
                    .fields
                    .retain(|name, ty| peer.is_some_and(|peer| peer.fields.get(name) == Some(ty)));
            }
        }
        joined
    }

    pub fn bind_params(
        &mut self,
        params: &FormalParamList,
        model: &SemanticModel<'_>,
        type_params: &TypeParameterScope,
    ) {
        for param in params.params.iter() {
            let ty = model.resolve_formal_parameter_in(param, type_params);
            self.declare_at(param.name.name.clone(), ty, param.name.span);
        }
    }

    /// The number of open scopes.
    pub fn depth(&self) -> usize {
        self.scopes.len()
    }

    /// The index of the innermost open scope binding `name`.
    pub fn binding_depth(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rposition(|scope| scope.contains_key(name))
    }

    pub fn is_bound(&self, name: &str) -> bool {
        self.binding(name).is_some()
    }

    /// Whether the nearest declaration of `name` belongs to the root scope.
    /// An inner declaration shadows the root, even when their types agree.
    pub fn is_bound_in_root_scope(&self, name: &str) -> bool {
        self.binding_depth(name) == Some(0)
    }

    pub fn lookup(&self, name: &str) -> ResolvedType {
        self.binding(name)
            .map(|binding| binding.promoted.last().unwrap_or(&binding.declared).clone())
            .unwrap_or(ResolvedType::Unknown)
    }

    /// Solves generic call parameters; owner and explicit substitutions are fixed.
    pub fn infer_signature_substitutions<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        signature: &ResolvedSignature,
        args: &falcon_syntax::ast::ArgList,
        type_args: &[DartType],
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
    pub fn infer_signature_substitutions_with_result<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        signature: (
            &ResolvedSignature,
            &[TypeParameterId],
            &ResolvedType,
            &HashMap<TypeParameterId, ResolvedType>,
        ),
        args: &falcon_syntax::ast::ArgList,
        type_args: &[DartType],
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        mut argument_contexts: Option<&mut HashMap<usize, ResolvedType>>,
    ) -> HashMap<TypeParameterId, ResolvedType> {
        let (signature, call_parameters, return_type, owner) = signature;
        let mut substitutions = owner.clone();
        if call_parameters.is_empty() {
            return substitutions;
        }
        if !type_args.is_empty() {
            substitutions.extend(call_parameters.iter().enumerate().map(|(index, id)| {
                (
                    id.clone(),
                    type_args.get(index).map_or(ResolvedType::Unknown, |ty| {
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
             argument: &Expr,
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
             argument: &Expr,
             substitutions: &HashMap<TypeParameterId, ResolvedType>| {
                if !call_parameters
                    .iter()
                    .any(|id| !substitutions.contains_key(id))
                {
                    return false;
                }
                let Expr::FuncExpr { params, .. } = argument else {
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
                params
                    .positional()
                    .zip(positional.iter())
                    .any(|(param, ty)| {
                        param.param_type.is_none()
                            && param.function_params.is_none()
                            && depends_on_call(ty)
                    })
                    || params.named().any(|param| {
                        param.param_type.is_none()
                            && param.function_params.is_none()
                            && named
                                .iter()
                                .find(|(name, _)| name == &param.name.name)
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
            for (parameter, argument) in signature.positional.iter().zip(&args.positional).chain(
                args.named.iter().filter_map(|argument| {
                    signature
                        .named
                        .get(&argument.name.name)
                        .map(|parameter| (parameter, &argument.value))
                }),
            ) {
                if matches!(argument, Expr::FuncExpr { .. }) != closures {
                    continue;
                }
                let expected_argument = context.model.substitute(parameter, &argument_context);
                if closures && has_unresolved_inputs(parameter, argument, &substitutions) {
                    if deferred.capacity() == 0 {
                        deferred.reserve(args.positional.len() + args.named.len());
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

    fn infer_constructor<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        type_args: &[DartType],
        args: &falcon_syntax::ast::ArgList,
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        if let Some(alias) = context.model.identities.type_alias(&identity) {
            if !type_args.is_empty() {
                let arguments: Vec<_> = type_args
                    .iter()
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
                &[],
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
                    .iter()
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
        expr: &Expr,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        type_parameters: &TypeParameterScope,
    ) -> ResolvedType {
        self.infer_with_signatures_using_lookup(expr, model, signatures, type_parameters, &|_| None)
    }

    /// [`Self::infer_with_signatures`], using `lookup` for names without a local binding.
    pub fn infer_with_signatures_using_lookup(
        &self,
        expr: &Expr,
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
    fn receiver_type(
        &self,
        object: &Expr,
        name: &str,
        context: &InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        expected: Option<&ResolvedType>,
    ) -> ResolvedType {
        if matches!(object, Expr::Super { .. }) {
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
    fn implicit_this_method<'a>(
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
    fn static_access_type(
        &self,
        object: &Expr,
        name: &str,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> Option<ResolvedType> {
        let segments = expression_segments(object, false)?;
        let head = segments.first()?;
        if self.is_bound(head) || lookup(head).is_some() {
            return None;
        }
        if let [prefix] = segments.as_slice()
            && let Some(identity) = model.resolve_value(QualifiedName::prefixed(prefix, name))
        {
            return signatures.value_type(&identity).cloned();
        }
        let identity = model
            .resolve_dotted(&segments)
            .or_else(|| model.resolve_extension(QualifiedName::from_segments(&segments)?))?;
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

    fn tearoff_type<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        signature: &ResolvedSignature,
        owner: &HashMap<TypeParameterId, ResolvedType>,
        explicit: &[DartType],
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

    fn constructor_tearoff<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        explicit: &[DartType],
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

    fn callable_tearoff<F: Fn(&str) -> Option<ResolvedType>>(
        &self,
        target: &Expr,
        explicit: &[DartType],
        context: &InferenceContext<'_, '_, '_, F>,
        expected: Option<&ResolvedType>,
        cached_receiver: &mut Option<ResolvedType>,
    ) -> Option<ResolvedType> {
        if self.invocation_unshadowed(target, context)
            && matches!(target, Expr::Field { .. })
            && expression_segments(target, true)
                .is_some_and(|segments| context.model.resolve_dotted(&segments).is_none())
            && let Some((identity, name)) = resolve_call_constructor(target, context.model, true)
            && let Some(ty) = self.constructor_tearoff(identity, name, explicit, context, expected)
        {
            return Some(ty);
        }
        let (signature, owner) =
            self.invocation_declaration(target, context, cached_receiver, None, false)?;
        Some(self.tearoff_type(signature, &owner, explicit, context, expected))
    }

    /// Infers an expression with a downward result context and lexical lookup.
    pub fn infer_with_context(
        &self,
        expr: &Expr,
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

    /// Reuses an immediate receiver already inferred by a shared AST visitor.
    /// A null-shorted selector chain makes the result nullable.
    fn infer_in_context(
        &self,
        expr: &Expr,
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
            self.tearoff_type(signature, &owner, &[], context, expected)
        } else {
            ty
        }
    }

    /// The reached-branch type, before this chain's shorted null is added.
    /// Nullability declared by a member remains part of that branch's type.
    fn infer_unshorted(
        &self,
        expr: &Expr,
        context: InferenceContext<'_, '_, '_, impl Fn(&str) -> Option<ResolvedType>>,
        expected: Option<&ResolvedType>,
        mut receiver: Option<ResolvedType>,
    ) -> ResolvedType {
        if let Expr::Paren { expr, .. } = expr {
            return self.infer_in_context(expr, context, expected, receiver);
        }
        let model = context.model;
        let signatures = context.signatures;
        let type_parameters = context.type_parameters;
        let lookup = context.lookup;
        let infer = |expr: &Expr| {
            self.infer_with_signatures_using_lookup(
                expr,
                model,
                signatures,
                type_parameters,
                lookup,
            )
        };
        if let Expr::Field {
            object,
            field,
            is_null_safe,
            ..
        } = expr
        {
            if let Some(ty) = self.promoted_field(object, &field.name) {
                return ty;
            }
            if let Some(ty) =
                self.static_access_type(object, &field.name, model, signatures, lookup)
            {
                return ty;
            }
            let receiver = receiver.take().unwrap_or_else(|| {
                self.receiver_type(
                    object,
                    &field.name,
                    &context,
                    expected.filter(|_| expression_has_dot_shorthand_base(object)),
                )
            });
            let receiver = if *is_null_safe {
                receiver.non_null()
            } else {
                receiver
            };
            if let Some(ty) = signatures.instantiated_data_read(&receiver, &field.name, model) {
                return ty;
            }
            if matches!(receiver, ResolvedType::Dynamic) {
                return ResolvedType::Dynamic;
            }
            if field.name == "call" && matches!(receiver, ResolvedType::Function { .. }) {
                return receiver.with_nullable(false);
            }
            let explicit = match object.as_ref() {
                Expr::GenericInstantiation { type_args, .. } => type_args.as_slice(),
                _ => &[],
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
        if let Expr::Call {
            callee,
            type_args,
            args,
            ..
        } = expr
        {
            let (target, explicit) = match callee.unparenthesized() {
                Expr::GenericInstantiation {
                    target, type_args, ..
                } => (target.as_ref(), type_args.as_slice()),
                Expr::Field { object, .. } => match object.as_ref() {
                    Expr::GenericInstantiation { type_args, .. } => {
                        (callee.as_ref(), type_args.as_slice())
                    }
                    _ => (callee.as_ref(), type_args.as_slice()),
                },
                target => (target, type_args.as_slice()),
            };
            if let Expr::DotShorthand { name, .. } = target
                && let Some(ResolvedType::Interface { identity, .. }) = expected
            {
                if signatures.constructor(identity, &name.name).is_some() || name.name == "new" {
                    return self.infer_constructor(
                        identity.clone(),
                        &name.name,
                        explicit,
                        args,
                        &context,
                        expected,
                    );
                }
                if signatures
                    .declared_member_facts(identity, &name.name)
                    .iter()
                    .any(|facts| facts.is_static)
                    && let Some(signature) = signatures.member(identity, &name.name)
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
            if let Expr::Field {
                object,
                field,
                is_null_safe: false,
                ..
            } = target
                && field.name == "clamp"
            {
                let reached = receiver.get_or_insert_with(|| {
                    self.receiver_type(
                        object,
                        &field.name,
                        &context,
                        expected.filter(|_| expression_has_dot_shorthand_base(object)),
                    )
                });
                if numeric(reached)
                    && !reached.nullable()
                    && let [lower, upper] = args.positional.as_slice()
                    && args.named.is_empty()
                {
                    let operands = [reached.clone(), infer(lower), infer(upper)];
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
                        invocation.type_args,
                        &context,
                        expected,
                        None,
                    );
                    model.substitute(invocation.inference_signature().2, &substitutions)
                }
                Err(return_type) => return_type,
            };
        }
        match expr {
            Expr::FuncExpr {
                type_params,
                params,
                body,
                async_marker,
                ..
            } => self.infer_function(
                FunctionSyntax {
                    type_params,
                    params,
                    body,
                    async_marker: *async_marker,
                    return_type: None,
                },
                model,
                signatures,
                type_parameters,
                expected,
                lookup,
            ),
            Expr::Is { .. }
            | Expr::Unary {
                op: falcon_syntax::ast::UnaryOp::Bang,
                ..
            } => model.core_type("bool", false),
            Expr::Unary {
                op: falcon_syntax::ast::UnaryOp::Minus,
                operand,
                ..
            } if matches!(operand.as_ref(), Expr::IntLit { .. })
                && expected.is_some_and(|context| context.interface("dart:core", "double")) =>
            {
                model.core_type("double", false)
            }
            Expr::Unary { op, operand, .. } => {
                let ty = infer(operand);
                if numeric(&ty) {
                    ty.with_nullable(false)
                } else {
                    let name = match op {
                        falcon_syntax::ast::UnaryOp::Minus => "unary-",
                        falcon_syntax::ast::UnaryOp::Tilde => "~",
                        falcon_syntax::ast::UnaryOp::PlusPlus => "+",
                        falcon_syntax::ast::UnaryOp::MinusMinus => "-",
                        falcon_syntax::ast::UnaryOp::Bang => unreachable!(),
                    };
                    operator_return_type(&ty, name, signatures, model)
                }
            }
            Expr::PostfixIncDec { operand, .. } => {
                let ty = infer(operand);
                if numeric(&ty) {
                    ty.with_nullable(false)
                } else {
                    ty
                }
            }
            Expr::IntLit { .. }
                if expected.is_some_and(|context| context.interface("dart:core", "double")) =>
            {
                model.core_type("double", false)
            }
            Expr::IntLit { .. } => model.core_type("int", false),
            Expr::DoubleLit { .. } => model.core_type("double", false),
            Expr::StringLit(_) => model.core_type("String", false),
            Expr::BoolLit { .. } => model.core_type("bool", false),
            Expr::NullLit { .. } => ResolvedType::Null,
            Expr::SymbolLit { .. } => model.core_type("Symbol", false),
            Expr::GenericInstantiation {
                target, type_args, ..
            } => self
                .callable_tearoff(target, type_args, &context, expected, &mut None)
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
                }),
            Expr::DotShorthand { name, .. } => {
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
                if signatures.is_enum_value(identity, &name.name) {
                    return receiver.with_nullable(false);
                }
                let owner = signatures.owner_substitutions(
                    signatures.type_parameters(identity).unwrap_or(&[]),
                    arguments,
                    Some(occurrence),
                );
                if let Some((_, ty)) = signatures.field(identity, &name.name)
                    && signatures
                        .declared_member_facts(identity, &name.name)
                        .iter()
                        .any(|facts| facts.is_static)
                {
                    return model.substitute(ty, &owner);
                }
                self.constructor_tearoff(identity.clone(), &name.name, &[], &context, expected)
                    .or_else(|| {
                        signatures.member(identity, &name.name).map(|signature| {
                            self.tearoff_type(signature, &owner, &[], &context, expected)
                        })
                    })
                    .unwrap_or(ResolvedType::Unknown)
            }
            Expr::This { .. } => self.lookup("this"),
            Expr::Ident(identifier) => {
                // Bindings this environment adds (pattern variables of an
                // inferred switch arm) shadow the lexical lookup.
                if self.is_local_function(&identifier.name) {
                    self.callable_tearoff(expr, &[], &context, expected, &mut None)
                        .unwrap_or_else(|| self.lookup(&identifier.name))
                } else if self.is_bound(&identifier.name) {
                    self.lookup(&identifier.name)
                } else if let Some(ty) = self.promoted_this_field(&identifier.name) {
                    ty
                } else if let Some(ty) = lookup(&identifier.name) {
                    ty
                } else if let Some(ty) =
                    self.callable_tearoff(expr, &[], &context, expected, &mut None)
                {
                    ty
                } else if let Some(identity) = model.resolve_value(identifier.name.as_str())
                    && let Some(ty) = signatures.value_type(&identity)
                {
                    ty.clone()
                } else if identifier.name == "Never"
                    || model.resolve_name(identifier.name.as_str()).is_some()
                    || type_parameters.lookup(&identifier.name).is_some()
                {
                    model.core_type("Type", false)
                } else {
                    self.lookup(&identifier.name)
                }
            }
            Expr::As { dart_type, .. } => model.resolve_type_in(dart_type, type_parameters),
            Expr::NullAssert { operand, .. } => {
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
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                if let DartType::Named(named) = dart_type
                    && let Some((identity, name)) =
                        resolve_new_constructor(dart_type, constructor_name.as_ref(), model)
                {
                    self.infer_constructor(
                        identity,
                        name,
                        &named.type_args,
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
            Expr::Cascade { object, .. } => self.infer_with_context(
                object,
                model,
                signatures,
                type_parameters,
                expected,
                lookup,
            ),
            Expr::Await { expr: operand, .. } => {
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
            Expr::Record { fields, .. } => {
                let expected = expected.filter(|ty| matches!(ty, ResolvedType::Record { positional, named, .. }
                    if positional.len() == fields.iter().filter(|field| field.name.is_none()).count()
                        && named.len() == fields.iter().filter(|field| field.name.is_some()).count()
                        && fields.iter().filter_map(|field| field.name.as_ref()).all(|field| named.iter().any(|(name, _)| name == &field.name))));
                let positional_count = fields.iter().filter(|field| field.name.is_none()).count();
                let mut positional = Vec::with_capacity(positional_count);
                let mut named = Vec::with_capacity(fields.len() - positional_count);
                for field in fields {
                    let context = expected.and_then(|ty| match &field.name {
                        Some(name) => ty.record_field(&name.name),
                        None => ty.record_positional(positional.len()),
                    });
                    let ty = self.infer_with_context(
                        &field.value,
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
                    match &field.name {
                        Some(name) => named.push((name.name.clone(), ty)),
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
            Expr::List {
                type_arg, elements, ..
            }
            | Expr::Set {
                type_arg, elements, ..
            } => {
                // Spread-only brace literals are parsed as sets until their
                // downward context or spread operands establish a map shape.
                let map_context = if matches!(expr, Expr::Set { .. }) && type_arg.is_none() {
                    expected.and_then(|ty| {
                        signatures.instantiated_supertype(ty, "dart:core", "Map", model)
                    })
                } else {
                    None
                };
                let context = type_arg
                    .as_ref()
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
                    infer_map_spreads: matches!(expr, Expr::Set { .. })
                        && type_arg.is_none()
                        && context.is_none(),
                };
                for element in elements {
                    visitor.visit_collection_element(element);
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
                        if matches!(expr, Expr::List { .. }) {
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
            Expr::Map {
                type_args,
                elements,
                ..
            } => {
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
                        .get(ordinal)
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
                    visitor.visit_map_element(element);
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
            Expr::Binary {
                op, left, right, ..
            } => match op {
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
                _ => {
                    let left = infer(left);
                    let right_context = operator_parameter_type(&left, *op, signatures, model);
                    let right = self.infer_with_context(
                        right,
                        model,
                        signatures,
                        type_parameters,
                        right_context.as_ref(),
                        lookup,
                    );
                    binary_result(*op, &left, &right, signatures, model)
                }
            },
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                let flow = self.flow(model, signatures, type_parameters);
                let branch = |truth: bool, branch: &Expr| {
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
            Expr::Switch { subject, arms, .. } => {
                let flow = self.flow(model, signatures, type_parameters);
                let matched = infer(subject);
                let arm_types: Vec<_> = arms
                    .iter()
                    .map(|arm| {
                        let mut environment = self.clone();
                        environment.push_scope();
                        flow.assume_pattern(&mut environment, subject, &arm.pattern, true);
                        flow.declare_pattern(&mut environment, &arm.pattern, matched.clone());
                        if let Some(guard) = &arm.guard {
                            flow.assume_condition(&mut environment, guard, true);
                        }
                        environment.infer_with_context(
                            &arm.body,
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
            Expr::Index {
                object,
                is_null_safe,
                ..
            } => {
                let receiver = self.receiver_type(object, "[]", &context, None);
                let receiver = if *is_null_safe {
                    receiver.non_null()
                } else {
                    receiver
                };
                operator_return_type(&receiver, "[]", signatures, model)
            }
            Expr::Assign {
                target, op, value, ..
            } => {
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
            Expr::Throw { .. } => ResolvedType::Never,
            _ => ResolvedType::Unknown,
        }
    }

    /// Infers a function literal or local function. Its body is a lexical child of
    /// `self`, so captured bindings keep their flow state and local-function kind.
    fn infer_function(
        &self,
        function: FunctionSyntax<'_>,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        type_parameters: &TypeParameterScope,
        expected: Option<&ResolvedType>,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> ResolvedType {
        use falcon_syntax::ast::AsyncMarker;
        if function.return_type.is_none()
            && matches!(
                function.async_marker,
                AsyncMarker::SyncStar | AsyncMarker::AsyncStar
            )
        {
            return ResolvedType::Unknown;
        }
        let mut function_parameters = type_parameters.clone();
        function_parameters.push(function.type_params, model);
        let contextual = match expected {
            Some(ResolvedType::Function {
                alias,
                return_type,
                positional,
                named,
                ..
            }) => Some((alias, return_type.as_ref(), positional, named)),
            _ => None,
        };
        let mut environment = self.clone();
        environment.enter_callable();
        environment.push_scope();
        let positional: Vec<_> = function
            .params
            .positional()
            .enumerate()
            .map(|(index, param)| {
                let ty = if param.param_type.is_some() || param.function_params.is_some() {
                    model.resolve_formal_parameter_in(param, &function_parameters)
                } else {
                    contextual
                        .and_then(|(alias, _, positional, _)| {
                            positional
                                .get(index)
                                .map(|ty| ty.clone().projected_from(alias))
                        })
                        .unwrap_or_else(|| signatures.uncontextual_closure_type(expected))
                };
                environment.declare(param.name.name.clone(), ty.clone());
                ty
            })
            .collect();
        let named: Vec<_> = function
            .params
            .named()
            .map(|param| {
                let ty = if param.param_type.is_some() || param.function_params.is_some() {
                    model.resolve_formal_parameter_in(param, &function_parameters)
                } else {
                    contextual
                        .and_then(|(alias, _, _, named)| {
                            named
                                .iter()
                                .find(|(name, _)| name == &param.name.name)
                                .map(|(_, ty)| ty.clone().projected_from(alias))
                        })
                        .unwrap_or_else(|| signatures.uncontextual_closure_type(expected))
                };
                environment.declare(param.name.name.clone(), ty.clone());
                (param.name.name.clone(), ty)
            })
            .collect();
        let return_type = match function.return_type {
            Some(declared) => model.resolve_type_in(declared, &function_parameters),
            None => {
                let body_context = contextual.map(|(alias, return_type, _, _)| {
                    let return_type = return_type.clone().projected_from(alias);
                    if function.async_marker == AsyncMarker::Async
                        && return_type.interface("dart:async", "Future")
                    {
                        ResolvedType::Interface {
                            occurrence: Default::default(),
                            identity: DeclarationIdentity::sdk("dart:async", "FutureOr"),
                            arguments: return_type.type_arguments().collect(),
                            nullable: false,
                            extension_type: false,
                        }
                    } else {
                        return_type.clone()
                    }
                });
                let return_type = match function.body {
                    FunctionBody::Arrow(expression, _) => environment.infer_with_context(
                        expression,
                        model,
                        signatures,
                        &function_parameters,
                        body_context.as_ref(),
                        lookup,
                    ),
                    FunctionBody::Block(block) => {
                        let mut returns = ClosureReturns {
                            environment,
                            conservative_writes: false,
                            flow: super::flow::FlowContext {
                                model,
                                signatures,
                                type_parameters: &function_parameters,
                                field_promotions: self.field_promotions,
                            },
                            context: body_context.as_ref(),
                            lookup,
                            types: Vec::new(),
                            exhaustive_switches: Vec::new(),
                        };
                        for statement in &block.stmts {
                            returns.visit_stmt(statement);
                        }
                        // A reachable end returns `null` implicitly.
                        if super::flow::completes_normally(
                            &block.stmts,
                            &|expression| returns.infer(expression, None) == ResolvedType::Never,
                            &|statement| returns.exhaustive_switches.contains(&statement.span),
                        ) {
                            returns.types.push(ResolvedType::Null);
                        }
                        let inferred = returns
                            .types
                            .into_iter()
                            .map(|ty| {
                                if function.async_marker == AsyncMarker::Async {
                                    flatten_async_return(ty, signatures, model)
                                } else {
                                    ty
                                }
                            })
                            .reduce(|left, right| inference_join(&left, &right, signatures, model))
                            .unwrap_or(ResolvedType::Null);
                        // Inference imputes `void` when the return context is
                        // `void`, or `FutureOr<void>` for an `async` literal.
                        let imputes_void = body_context.as_ref().is_some_and(|context| {
                            matches!(context, ResolvedType::Void)
                                || (function.async_marker == AsyncMarker::Async
                                    && context.interface("dart:async", "FutureOr")
                                    && matches!(
                                        context.arguments().first(),
                                        Some(ResolvedType::Void)
                                    ))
                        });
                        if imputes_void {
                            ResolvedType::Void
                        } else {
                            inferred
                        }
                    }
                    FunctionBody::Native(_, _) => ResolvedType::Unknown,
                };
                if function.async_marker == AsyncMarker::Async {
                    let argument = flatten_async_return(return_type, signatures, model);
                    ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:async", "Future"),
                        arguments: vec![argument].into(),
                        nullable: false,
                        extension_type: false,
                    }
                } else {
                    return_type
                }
            }
        };
        ResolvedType::Function {
            shape: FunctionTypeShape::from_scope(
                function
                    .params
                    .named()
                    .filter(|param| matches!(param.kind, ParamKind::Named { required: true }))
                    .map(|param| param.name.name.clone())
                    .collect(),
                function.type_params,
                &function_parameters,
            ),
            optional_positional: function
                .params
                .positional()
                .filter(|param| param.kind == ParamKind::OptionalPositional)
                .count(),
            alias: FunctionTypeAlias::default(),
            return_type: Arc::new(return_type),
            positional: positional.into(),
            named: named.into(),
            nullable: false,
        }
    }
}

/// The syntax function literals and local function declarations share.
struct FunctionSyntax<'f> {
    type_params: &'f [TypeParam],
    params: &'f FormalParamList,
    body: &'f FunctionBody,
    async_marker: falcon_syntax::ast::AsyncMarker,
    return_type: Option<&'f DartType>,
}

/// `(library, name, members)`; each member is `(name, kind, positional
/// parameter names)`.
type SdkInterfaceMembersEntry = (
    &'static str,
    &'static str,
    &'static [(&'static str, MemberKind, &'static [&'static str])],
);

/// Complete instance-member lists of SDK interfaces (beyond `Object`) whose
/// members [`SignatureIndex::inherited_member_facts`] may report; every listed
/// member is abstract. Any other SDK supertype makes inherited facts unknown.
const SDK_INTERFACE_MEMBERS: &[SdkInterfaceMembersEntry] = &[(
    "dart:core",
    "Type",
    &[
        ("hashCode", MemberKind::Getter, &[]),
        ("==", MemberKind::Operator, &["other"]),
        ("toString", MemberKind::Method, &[]),
    ],
)];

/// `(library, name, arity, supertypes)`; each supertype is `(library, name)`.
type SdkHierarchyEntry = (
    &'static str,
    &'static str,
    usize,
    &'static [(&'static str, &'static str)],
);

/// Curated SDK supertype graph: `(library, name, arity, supertypes)`.
///
/// Each entry lists every public supertype of its declaration other than
/// `Object`, so [`SignatureIndex::implements_sdk`] may prove a negative through
/// it. Generic supertypes with the same arity receive the declaration's own
/// type parameters; others are recorded raw.
const SDK_HIERARCHY: &[SdkHierarchyEntry] = &[
    ("dart:core", "Function", 0, &[]),
    ("dart:core", "Record", 0, &[]),
    ("dart:core", "Type", 0, &[]),
    ("dart:core", "Symbol", 0, &[]),
    ("dart:core", "bool", 0, &[]),
    ("dart:core", "Comparable", 1, &[]),
    ("dart:core", "Pattern", 0, &[]),
    ("dart:core", "num", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "int", 0, &[("dart:core", "num")]),
    ("dart:core", "double", 0, &[("dart:core", "num")]),
    (
        "dart:core",
        "String",
        0,
        &[("dart:core", "Comparable"), ("dart:core", "Pattern")],
    ),
    ("dart:core", "RegExp", 0, &[("dart:core", "Pattern")]),
    ("dart:core", "BigInt", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "Duration", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "DateTime", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "Uri", 0, &[]),
    ("dart:core", "StackTrace", 0, &[]),
    ("dart:core", "Enum", 0, &[]),
    ("dart:core", "Iterable", 1, &[]),
    ("dart:core", "Iterator", 1, &[]),
    ("dart:core", "Map", 2, &[]),
    ("dart:core", "MapEntry", 2, &[]),
    ("dart:core", "Exception", 0, &[]),
    (
        "dart:core",
        "FormatException",
        0,
        &[("dart:core", "Exception")],
    ),
    (
        "dart:core",
        "IntegerDivisionByZeroException",
        0,
        &[
            ("dart:core", "Exception"),
            ("dart:core", "UnsupportedError"),
        ],
    ),
    ("dart:core", "Error", 0, &[]),
    ("dart:core", "AssertionError", 0, &[("dart:core", "Error")]),
    ("dart:core", "TypeError", 0, &[("dart:core", "Error")]),
    ("dart:core", "ArgumentError", 0, &[("dart:core", "Error")]),
    (
        "dart:core",
        "RangeError",
        0,
        &[("dart:core", "ArgumentError")],
    ),
    (
        "dart:core",
        "IndexError",
        0,
        &[("dart:core", "ArgumentError"), ("dart:core", "RangeError")],
    ),
    ("dart:core", "StateError", 0, &[("dart:core", "Error")]),
    (
        "dart:core",
        "UnsupportedError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "UnimplementedError",
        0,
        &[("dart:core", "UnsupportedError")],
    ),
    (
        "dart:core",
        "ConcurrentModificationError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "OutOfMemoryError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "StackOverflowError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "NoSuchMethodError",
        0,
        &[("dart:core", "Error")],
    ),
    ("dart:core", "Sink", 1, &[]),
    ("dart:core", "StringSink", 0, &[]),
    ("dart:async", "Future", 1, &[]),
    ("dart:async", "Completer", 1, &[]),
    ("dart:async", "Timer", 0, &[]),
    ("dart:async", "Stream", 1, &[]),
    ("dart:async", "StreamSubscription", 1, &[]),
    ("dart:async", "EventSink", 1, &[("dart:core", "Sink")]),
    ("dart:async", "StreamConsumer", 1, &[]),
    (
        "dart:async",
        "StreamSink",
        1,
        &[
            ("dart:async", "EventSink"),
            ("dart:async", "StreamConsumer"),
        ],
    ),
    (
        "dart:async",
        "StreamController",
        1,
        &[("dart:async", "StreamSink")],
    ),
    (
        "dart:async",
        "TimeoutException",
        0,
        &[("dart:core", "Exception")],
    ),
    ("dart:async", "AsyncError", 0, &[("dart:core", "Error")]),
    ("dart:io", "FileSystemEntity", 0, &[]),
    ("dart:io", "File", 0, &[("dart:io", "FileSystemEntity")]),
    (
        "dart:io",
        "Directory",
        0,
        &[("dart:io", "FileSystemEntity")],
    ),
    ("dart:io", "Link", 0, &[("dart:io", "FileSystemEntity")]),
    (
        "dart:io",
        "IOSink",
        0,
        &[("dart:async", "StreamSink"), ("dart:core", "StringSink")],
    ),
    (
        "dart:io",
        "Socket",
        0,
        &[("dart:async", "Stream"), ("dart:io", "IOSink")],
    ),
    ("dart:io", "IOException", 0, &[("dart:core", "Exception")]),
    ("dart:io", "OSError", 0, &[("dart:core", "Exception")]),
    (
        "dart:io",
        "FileSystemException",
        0,
        &[("dart:io", "IOException")],
    ),
    ("dart:io", "HttpException", 0, &[("dart:io", "IOException")]),
    (
        "dart:io",
        "SocketException",
        0,
        &[("dart:io", "IOException")],
    ),
];

fn sdk_parameter(owner: &'static str, ordinal: usize) -> TypeParameterId {
    TypeParameterId {
        owner: ParameterOwner::Sdk(owner),
        ordinal,
        name: format!("{owner}#{ordinal}").into(),
    }
}

/// The external (SDK or package) identities of `annotations`.
fn resolve_annotations(
    annotations: &[falcon_syntax::ast::Annotation],
    model: &SemanticModel<'_>,
) -> Vec<DeclarationIdentity> {
    annotations
        .iter()
        .filter_map(|annotation| {
            model.resolve_imported_member(QualifiedName::from_identifiers(&annotation.name)?)
        })
        .collect()
}
/// Dart's async return inference flattens each return before computing their UP.
fn flatten_async_return(
    ty: ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    let flattened =
        if ty.interface("dart:async", "Future") || ty.interface("dart:async", "FutureOr") {
            Some(ty.type_argument(0).unwrap_or(ResolvedType::Dynamic))
        } else {
            signatures
                .instantiated_supertype(&ty, "dart:async", "Future", model)
                .and_then(|future| future.type_argument(0))
        };
    match flattened {
        Some(flattened) if ty.nullable() => flattened.with_nullable(true),
        Some(flattened) => flattened,
        None => ty,
    }
}

/// Collects a block body's return types while binding its locals in a lexical child environment.
struct ClosureReturns<'a, 'model, L> {
    environment: TypeEnvironment,
    /// Unsupported branch/loop joins must invalidate writes without
    /// publishing a promotion from a path that need not execute.
    conservative_writes: bool,
    flow: super::flow::FlowContext<'a, 'model>,
    context: Option<&'a ResolvedType>,
    lookup: &'a L,
    types: Vec<ResolvedType>,
    exhaustive_switches: Vec<falcon_syntax::ast::Span>,
}

impl<L: Fn(&str) -> Option<ResolvedType>> ClosureReturns<'_, '_, L> {
    fn infer(&self, expression: &Expr, expected: Option<&ResolvedType>) -> ResolvedType {
        self.environment.infer_with_context(
            expression,
            self.flow.model,
            self.flow.signatures,
            self.flow.type_parameters,
            expected,
            self.lookup,
        )
    }

    fn scoped(&mut self, visit: impl FnOnce(&mut Self)) {
        self.environment.push_scope();
        visit(self);
        self.environment.pop_scope();
    }

    fn declare_local(&mut self, declaration: &falcon_syntax::ast::LocalVarDecl) {
        for declarator in &declaration.declarators {
            let ty = declaration
                .var_type
                .as_ref()
                .map(|ty| {
                    self.flow
                        .model
                        .resolve_type_in(ty, self.flow.type_parameters)
                })
                .or_else(|| {
                    declarator
                        .initializer
                        .as_ref()
                        .map(|expression| self.infer(expression, None))
                })
                .unwrap_or(ResolvedType::Dynamic);
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
            self.environment.declare(declarator.name.name.clone(), ty);
        }
    }

    fn declare_pattern(&mut self, pattern: &falcon_syntax::ast::Pattern, matched: ResolvedType) {
        self.flow
            .declare_pattern(&mut self.environment, pattern, matched);
    }
}

impl super::flow::FlowContext<'_, '_> {
    /// Proves only spaces represented by the semantic index, never a guessed
    /// closed subtype set. Guards cannot contribute unconditional coverage.
    pub fn switch_is_exhaustive(
        &self,
        statement: &falcon_syntax::ast::SwitchStmt,
        matched: &ResolvedType,
    ) -> bool {
        use falcon_syntax::ast::{ConstPatternValue, LiteralPatternValue, Pattern, SwitchCaseKind};
        let patterns = || {
            statement
                .cases
                .iter()
                .flat_map(|case| &case.cases)
                .filter_map(|case| match case {
                    SwitchCaseKind::Pattern(pattern, None) => Some(pattern.as_ref()),
                    _ => None,
                })
        };
        let covers_type = |matched: &ResolvedType| {
            patterns().any(|pattern| {
                let target = match pattern {
                    Pattern::Wildcard { type_: None, .. }
                    | Pattern::Variable { type_: None, .. } => {
                        return true;
                    }
                    Pattern::Wildcard {
                        type_: Some(type_), ..
                    }
                    | Pattern::Variable {
                        type_: Some(type_), ..
                    } => self.model.resolve_type_in(type_, self.type_parameters),
                    Pattern::Object(object) if object.fields.is_empty() => {
                        self.object_pattern_type(&object.type_, matched)
                    }
                    _ => return false,
                };
                if matches!(target, ResolvedType::Unknown) {
                    return false;
                }
                matched == &target
                    || self.signatures.subtype(matched, &target, self.model) == Truth::Yes
            })
        };
        if covers_type(matched) {
            return true;
        }
        let covers_null = patterns().any(|pattern| {
            matches!(pattern, Pattern::Literal(literal) if matches!(literal.value, LiteralPatternValue::Null))
        });
        if *matched == ResolvedType::Null {
            return covers_null;
        }
        if matched.nullable() && !covers_null {
            return false;
        }
        if matched.nullable() && covers_type(&matched.with_nullable(false)) {
            return true;
        }
        let ResolvedType::Interface { identity, .. } = matched else {
            return false;
        };
        if identity.is_sdk("dart:core", "bool") {
            return [false, true].into_iter().all(|value| {
                patterns().any(|pattern| {
                    matches!(pattern, Pattern::Literal(literal) if matches!(literal.value, LiteralPatternValue::Bool(actual) if actual == value))
                })
            });
        }
        let Some(values) = self.signatures.enum_values.get(identity) else {
            return false;
        };
        !values.is_empty()
            && values.iter().all(|value| {
                patterns().any(|pattern| {
                    let Pattern::Const(constant) = pattern else {
                        return false;
                    };
                    let ConstPatternValue::Name(names) = &constant.value else {
                        return false;
                    };
                    let Some((name, owner)) = names.split_last() else {
                        return false;
                    };
                    name.name == *value
                        && QualifiedName::from_identifiers(owner)
                            .and_then(|owner| self.model.resolve_name(owner))
                            .as_ref()
                            == Some(identity)
                })
            })
    }
}

impl<L: Fn(&str) -> Option<ResolvedType>> Visitor for ClosureReturns<'_, '_, L> {
    fn visit_stmt(&mut self, statement: &falcon_syntax::ast::Stmt) {
        use falcon_syntax::ast::{ForInit, IfCondition, Stmt, SwitchCaseKind};
        let loop_start = match statement {
            Stmt::For(statement) => Some(statement.span.start),
            Stmt::While(statement) => Some(statement.span.start),
            Stmt::DoWhile(statement) => Some(statement.span.start),
            _ => None,
        };
        if let Some(start) = loop_start
            && let Some(writes) = self
                .flow
                .signatures
                .flow_writes
                .get(&self.flow.model.file)
                .and_then(|writes| writes.region_writes.get(&start))
        {
            for name in writes {
                self.environment.demote(name);
            }
        }
        let conservative_before = self.conservative_writes;
        self.conservative_writes |= matches!(
            statement,
            Stmt::While(_) | Stmt::DoWhile(_) | Stmt::For(_) | Stmt::TryCatch(_) | Stmt::Switch(_)
        ) || matches!(statement, Stmt::If(statement) if matches!(statement.condition, IfCondition::Case(..)));
        let optional_entry =
            (self.conservative_writes && !conservative_before).then(|| self.environment.clone());
        match statement {
            Stmt::Return(statement) => {
                if let Some(value) = &statement.value {
                    self.visit_expr(value);
                }
                let ty = statement
                    .value
                    .as_ref()
                    .map_or(ResolvedType::Void, |value| self.infer(value, self.context));
                self.types.push(ty);
            }
            Stmt::LocalVar(declaration) => self.declare_local(declaration),
            Stmt::LocalFunc(function) => {
                let ty = self.environment.infer_function(
                    FunctionSyntax {
                        type_params: &function.type_params,
                        params: &function.params,
                        body: &function.body,
                        async_marker: function.async_marker,
                        return_type: function.return_type.as_ref(),
                    },
                    self.flow.model,
                    self.flow.signatures,
                    self.flow.type_parameters,
                    None,
                    self.lookup,
                );
                self.environment
                    .declare_local_function(function.name.name.clone(), ty);
            }
            Stmt::PatternDecl(declaration) => {
                let matched = self.infer(&declaration.init, None);
                self.declare_pattern(&declaration.pattern, matched);
            }
            Stmt::Block(_) => self.scoped(|returns| walk_stmt(returns, statement)),
            Stmt::If(statement) => match &statement.condition {
                IfCondition::Expr(condition) => {
                    self.visit_expr(condition);
                    let before = self.environment.clone();
                    self.flow
                        .assume_condition(&mut self.environment, condition, true);
                    self.scoped(|returns| returns.visit_stmt(&statement.then_branch));
                    let then_exits = self
                        .flow
                        .statement_exits(&self.environment, &statement.then_branch);
                    let then_environment = self.environment.clone();
                    self.environment = before;
                    self.flow
                        .assume_condition(&mut self.environment, condition, false);
                    let else_exits = if let Some(branch) = &statement.else_branch {
                        self.scoped(|returns| returns.visit_stmt(branch));
                        self.flow.statement_exits(&self.environment, branch)
                    } else {
                        false
                    };
                    if !then_exits {
                        self.environment = if else_exits {
                            then_environment
                        } else {
                            then_environment.join(&self.environment)
                        };
                    }
                    if let Some(entry) = optional_entry {
                        self.environment = entry.join(&self.environment);
                    }
                    self.conservative_writes = conservative_before;
                    return;
                }
                IfCondition::Case(value, pattern, _) => {
                    self.visit_expr(value);
                    let matched = self.infer(value, None);
                    let before = self.environment.clone();
                    self.scoped(|returns| {
                        returns.declare_pattern(pattern, matched);
                        returns.visit_stmt(&statement.then_branch);
                    });
                    let then_environment = self.environment.clone();
                    self.environment = before.join(&then_environment);
                    if let Some(branch) = &statement.else_branch {
                        self.visit_stmt(branch);
                    }
                    self.environment = then_environment.join(&self.environment);
                }
            },
            Stmt::For(statement) => self.scoped(|returns| {
                match &statement.header.init {
                    Some(ForInit::VarDecl(declaration)) => returns.declare_local(declaration),
                    Some(ForInit::ForIn {
                        var_type,
                        name,
                        iterable,
                        declares_variable: true,
                        ..
                    }) => {
                        let ty = match var_type {
                            Some(ty) => returns
                                .flow
                                .model
                                .resolve_type_in(ty, returns.flow.type_parameters),
                            None => returns.flow.iterable_element(
                                &returns.infer(iterable, None),
                                statement.is_await,
                            ),
                        };
                        returns.environment.declare(name.name.clone(), ty);
                    }
                    Some(ForInit::PatternForIn {
                        pattern, iterable, ..
                    }) => {
                        let matched = returns
                            .flow
                            .iterable_element(&returns.infer(iterable, None), statement.is_await);
                        returns.declare_pattern(pattern, matched);
                    }
                    // A bare `for (x in xs)` writes the enclosing `x`.
                    Some(ForInit::Exprs(_) | ForInit::ForIn { .. }) | None => {}
                }
                returns.visit_stmt(&statement.body);
            }),
            Stmt::TryCatch(statement) => {
                let before = self.environment.clone();
                self.scoped(|returns| {
                    for statement in &statement.body.stmts {
                        returns.visit_stmt(statement);
                    }
                });
                let mut exits = self.environment.clone();
                for catch in &statement.catches {
                    self.environment = before.join(&exits);
                    self.scoped(|returns| {
                        // Untyped `catch (e, s)` binds `Object` and `StackTrace`.
                        if let Some(name) = &catch.exception_var {
                            let model = returns.flow.model;
                            let ty = catch.exception_type.as_ref().map_or_else(
                                || model.core_type("Object", false),
                                |ty| model.resolve_type_in(ty, returns.flow.type_parameters),
                            );
                            returns.environment.declare(name.name.clone(), ty);
                        }
                        if let Some(name) = &catch.stack_trace_var {
                            let ty = returns.flow.model.core_type("StackTrace", false);
                            returns.environment.declare(name.name.clone(), ty);
                        }
                        for statement in &catch.body.stmts {
                            returns.visit_stmt(statement);
                        }
                    });
                    exits = exits.join(&self.environment);
                }
                self.environment = before.join(&exits);
                if let Some(finally) = &statement.finally {
                    self.scoped(|returns| {
                        for statement in &finally.stmts {
                            returns.visit_stmt(statement);
                        }
                    });
                }
            }
            Stmt::Switch(statement) => {
                self.visit_expr(&statement.subject);
                let matched = self.infer(&statement.subject, None);
                if self.flow.switch_is_exhaustive(statement, &matched) {
                    self.exhaustive_switches.push(statement.span);
                }
                let before = self.environment.clone();
                let mut exits = before.clone();
                for case in &statement.cases {
                    self.environment = before.join(&exits);
                    self.scoped(|returns| {
                        for kind in &case.cases {
                            if let SwitchCaseKind::Pattern(pattern, _) = kind {
                                returns.declare_pattern(pattern, matched.clone());
                            }
                        }
                        for statement in &case.body {
                            returns.visit_stmt(statement);
                        }
                    });
                    exits = exits.join(&self.environment);
                }
                self.environment = exits;
            }
            _ => walk_stmt(self, statement),
        }
        if let Some(entry) = optional_entry {
            self.environment = entry.join(&self.environment);
        }
        self.conservative_writes = conservative_before;
    }

    fn visit_expr(&mut self, expression: &Expr) {
        let conservative_before = self.conservative_writes;
        self.conservative_writes |= matches!(
            expression,
            Expr::Switch { .. } | Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. }
        );
        match expression {
            // A nested callable is inferred separately and does not execute here.
            Expr::FuncExpr { .. } => {
                self.conservative_writes = conservative_before;
                return;
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.visit_expr(condition);
                let before = self.environment.clone();
                self.flow
                    .assume_condition(&mut self.environment, condition, true);
                self.visit_expr(then_expr);
                let then_environment = self.environment.clone();
                self.environment = before;
                self.flow
                    .assume_condition(&mut self.environment, condition, false);
                self.visit_expr(else_expr);
                self.environment = then_environment.join(&self.environment);
            }
            Expr::Binary {
                op: BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce,
                left,
                right,
                ..
            } => {
                self.visit_expr(left);
                let before = self.environment.clone();
                self.visit_expr(right);
                self.environment = before.join(&self.environment);
            }
            _ => walk_expr(self, expression),
        }
        if self.conservative_writes {
            let target = match expression {
                Expr::Assign { target, .. } => Some(target.as_ref()),
                Expr::Unary {
                    op: UnaryOp::PlusPlus | UnaryOp::MinusMinus,
                    operand,
                    ..
                }
                | Expr::PostfixIncDec { operand, .. } => Some(operand.as_ref()),
                _ => None,
            };
            if let Some(Expr::Ident(name)) = target {
                self.environment.demote(&name.name);
            }
        } else {
            let assigned = match expression {
                Expr::Assign {
                    target, op, value, ..
                } => {
                    let expected = match target.as_ref() {
                        Expr::Ident(name)
                            if self.environment.is_bound(&name.name)
                                && matches!(op.binary(), None | Some(BinaryOp::NullCoalesce)) =>
                        {
                            Some(self.environment.declared_type(&name.name))
                        }
                        _ => None,
                    };
                    Some(self.infer(value, expected.as_ref()))
                }
                _ => None,
            };
            self.flow
                .finish_expression(&mut self.environment, expression, assigned);
        }
        self.conservative_writes = conservative_before;
    }
}

struct CollectionTypes<'a, 'model, L> {
    lexical: ClosureReturns<'a, 'model, L>,
    contexts: [Option<&'a ResolvedType>; 2],
    components: [Vec<ResolvedType>; 2],
    /// A brace literal without a distinguishing downward schema can acquire
    /// its map shape from a spread operand rather than an explicit entry.
    infer_map_spreads: bool,
}

impl<L: Fn(&str) -> Option<ResolvedType>> CollectionTypes<'_, '_, L> {
    fn branch(
        &mut self,
        condition: &falcon_syntax::ast::IfCondition,
        truth: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        use falcon_syntax::ast::IfCondition;
        let saved = self.lexical.environment.clone();
        self.lexical.environment.push_scope();
        match condition {
            IfCondition::Expr(expression) => {
                self.lexical
                    .flow
                    .assume_condition(&mut self.lexical.environment, expression, truth)
            }
            IfCondition::Case(expression, pattern, guard) if truth => {
                let matched = self.lexical.infer(expression, None);
                self.lexical.declare_pattern(pattern, matched);
                self.lexical.flow.assume_pattern(
                    &mut self.lexical.environment,
                    expression,
                    pattern,
                    true,
                );
                if let Some(guard) = guard {
                    self.lexical
                        .flow
                        .assume_condition(&mut self.lexical.environment, guard, true);
                }
            }
            IfCondition::Case(expression, pattern, None) => self.lexical.flow.assume_pattern(
                &mut self.lexical.environment,
                expression,
                pattern,
                false,
            ),
            _ => {}
        }
        visit(self);
        self.lexical.environment = saved;
    }

    fn loop_body(
        &mut self,
        header: &falcon_syntax::ast::ForHeader,
        is_await: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        use falcon_syntax::ast::ForInit;
        let saved = header
            .condition
            .as_ref()
            .map(|_| self.lexical.environment.clone());
        self.lexical.environment.push_scope();
        match &header.init {
            Some(ForInit::VarDecl(declaration)) => self.lexical.declare_local(declaration),
            Some(ForInit::ForIn {
                var_type,
                name,
                iterable,
                declares_variable: true,
                ..
            }) => {
                let ty = var_type
                    .as_ref()
                    .map(|ty| {
                        self.lexical
                            .flow
                            .model
                            .resolve_type_in(ty, self.lexical.flow.type_parameters)
                    })
                    .unwrap_or_else(|| {
                        self.lexical
                            .flow
                            .iterable_element(&self.lexical.infer(iterable, None), is_await)
                    });
                self.lexical.environment.declare(name.name.clone(), ty);
            }
            Some(ForInit::PatternForIn {
                pattern, iterable, ..
            }) => {
                let matched = self
                    .lexical
                    .flow
                    .iterable_element(&self.lexical.infer(iterable, None), is_await);
                self.lexical.declare_pattern(pattern, matched);
            }
            _ => {}
        }
        if let Some(condition) = &header.condition {
            self.lexical
                .flow
                .assume_condition(&mut self.lexical.environment, condition, true);
        }
        visit(self);
        self.lexical.environment.pop_scope();
        if let Some(saved) = saved {
            self.lexical.environment = saved;
        }
    }

    fn spread(&mut self, expression: &Expr, map: bool, null_aware: bool) {
        let context = self.contexts[0].map(|first| ResolvedType::Interface {
            occurrence: Default::default(),
            identity: DeclarationIdentity::sdk("dart:core", if map { "Map" } else { "Iterable" }),
            arguments: if map {
                vec![
                    first.clone(),
                    self.contexts[1].cloned().unwrap_or(ResolvedType::Unknown),
                ]
                .into()
            } else {
                vec![first.clone()].into()
            },
            nullable: null_aware,
            extension_type: false,
        });
        let ty = self.lexical.infer(expression, context.as_ref());
        let map_type = (map || self.infer_map_spreads)
            .then(|| {
                self.lexical.flow.signatures.instantiated_supertype(
                    &ty.non_null(),
                    "dart:core",
                    "Map",
                    self.lexical.flow.model,
                )
            })
            .flatten();
        let map = map || map_type.is_some();
        let ty = if map {
            map_type
        } else {
            self.lexical.flow.signatures.instantiated_supertype(
                &ty.non_null(),
                "dart:core",
                "Iterable",
                self.lexical.flow.model,
            )
        };
        for (ordinal, component) in
            self.components
                .iter_mut()
                .enumerate()
                .take(if map { 2 } else { 1 })
        {
            component.push(
                ty.as_ref()
                    .and_then(|ty| ty.type_argument(ordinal))
                    .unwrap_or(ResolvedType::Unknown),
            );
        }
    }
}

impl<L: Fn(&str) -> Option<ResolvedType>> Visitor for CollectionTypes<'_, '_, L> {
    fn visit_collection_element(&mut self, element: &falcon_syntax::ast::CollectionElement) {
        use falcon_syntax::ast::CollectionElement;
        match element {
            CollectionElement::Expr(expression) => {
                self.components[0].push(self.lexical.infer(expression, self.contexts[0]))
            }
            CollectionElement::NullAware { expr, .. } => {
                self.components[0].push(self.lexical.infer(expr, self.contexts[0]).non_null())
            }
            CollectionElement::Spread {
                expr,
                is_null_aware,
                ..
            } => self.spread(expr, self.contexts[1].is_some(), *is_null_aware),
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                self.branch(condition, true, |visitor| {
                    visitor.visit_collection_element(then_elem)
                });
                if let Some(element) = else_elem {
                    self.branch(condition, false, |visitor| {
                        visitor.visit_collection_element(element)
                    });
                }
            }
            CollectionElement::For {
                header,
                is_await,
                element,
                ..
            } => self.loop_body(header, *is_await, |visitor| {
                visitor.visit_collection_element(element)
            }),
        }
    }

    fn visit_map_element(&mut self, element: &falcon_syntax::ast::MapElement) {
        use falcon_syntax::ast::MapElement;
        match element {
            MapElement::Entry(entry) => {
                let key = self.lexical.infer(&entry.key, self.contexts[0]);
                let value = self.lexical.infer(&entry.value, self.contexts[1]);
                self.components[0].push(if entry.key_null_aware {
                    key.non_null()
                } else {
                    key
                });
                self.components[1].push(if entry.value_null_aware {
                    value.non_null()
                } else {
                    value
                });
            }
            MapElement::Spread {
                expr,
                is_null_aware,
                ..
            } => self.spread(expr, true, *is_null_aware),
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => {
                self.branch(condition, true, |visitor| {
                    visitor.visit_map_element(then_entry)
                });
                if let Some(entry) = else_entry {
                    self.branch(condition, false, |visitor| visitor.visit_map_element(entry));
                }
            }
            MapElement::For {
                header,
                is_await,
                entry,
                ..
            } => self.loop_body(header, *is_await, |visitor| {
                visitor.visit_map_element(entry)
            }),
        }
    }

    fn visit_expr(&mut self, _expression: &Expr) {}
}

struct TypeConstraintContext<'a, 'model> {
    parameters: &'a [TypeParameterId],
    inferred: &'a mut HashMap<TypeParameterId, ResolvedType>,
    upper: &'a mut HashMap<TypeParameterId, ResolvedType>,
    signatures: &'a SignatureIndex,
    model: &'a SemanticModel<'model>,
    /// Unknown argument evidence survives joins; Unknown downward schema slots
    /// instead mean that no constraint has been supplied yet.
    poison_unknown: bool,
    /// Partial contextual schemas are fallbacks, not upward constraints.
    partial: Option<&'a mut HashMap<TypeParameterId, ResolvedType>>,
}

fn infer_type_constraints(
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

fn inference_candidate(
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

/// The position of a top type in Dart's `MORETOP` order, where `void` is above
/// `dynamic`, which is above `Object?`; `None` for any other type.
fn top_rank(ty: &ResolvedType) -> Option<u8> {
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
struct InferenceJoinMemo {
    shapes: FxHashMap<(u64, u64), Vec<InferenceJoinEntry>>,
    hashes: FxHashMap<*const ResolvedType, u64>,
    provenance: FxHashMap<AliasPresenceKey, bool>,
}

impl InferenceJoinMemo {
    fn hash(&mut self, ty: &ResolvedType) -> u64 {
        // Roots may be projected stack temporaries. Retain only hashes of their
        // immutable Arc children, which are kept alive by the cached operands.
        self.hashes.remove(&(ty as *const ResolvedType));
        let hash = ty.structural_hash(&mut self.hashes);
        self.hashes.remove(&(ty as *const ResolvedType));
        hash
    }

    fn project(&mut self, ty: &ResolvedType, owner: &FunctionTypeAlias) -> ResolvedType {
        ty.clone()
            .projected_from_memoized(owner, &mut self.provenance)
    }
}

fn inference_join_memoized(
    left: &ResolvedType,
    right: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
    memo: &mut InferenceJoinMemo,
) -> ResolvedType {
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
        return result.clone();
    }
    let result = inference_join_impl(left, right, signatures, model, memo);
    if let Some(key) = key {
        memo.shapes
            .entry(key)
            .or_default()
            .push((left.clone(), right.clone(), result.clone()));
    }
    result
}

fn inference_join(
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
                        signatures.subtype(left, &right, model),
                        signatures.subtype(&right, left, model),
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
            let left_to_right = signatures.subtype(left, right, model);
            if left_to_right == Truth::Yes {
                return Some(left.clone());
            }
            match signatures.subtype(right, left, model) {
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
        if signatures.subtype(&right_base, &left_base, model) == Truth::Yes {
            return left.with_nullable(nullable);
        }
        if signatures.subtype(&left_base, &right_base, model) == Truth::Yes {
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
    if signatures.subtype(&right_base, &left_base, model) == Truth::Yes {
        return left.with_nullable(nullable);
    }
    if signatures.subtype(&left_base, &right_base, model) == Truth::Yes {
        return right.with_nullable(nullable);
    }
    ResolvedType::Unknown
}

/// The return type of the operator method `name` invoked on `receiver`.
fn operator_return_type(
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
fn operator_parameter_type(
    receiver: &ResolvedType,
    op: BinaryOp,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> Option<ResolvedType> {
    let (signature, substitutions) =
        signatures.resolved_member(receiver, op.method_name()?, model)?;
    signature
        .positional
        .first()
        .map(|ty| model.substitute(ty, &substitutions))
}

/// The static type of the binary operator `op`: the special typing of the
/// built-in numeric and `String +` operators, else the return type of the
/// receiver's operator method.
fn binary_result(
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
    op.method_name().map_or(ResolvedType::Unknown, |name| {
        operator_return_type(left, name, signatures, model)
    })
}

/// The static type of a conditional, `??` or switch expression with branch
/// types `types`: their least upper bound `T`, unless `T` is not a subtype of
/// the context type `K` while every branch is, which makes it `K`.
fn contextual_join(
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
        && types
            .iter()
            .all(|ty| signatures.subtype(ty, context, model) == Truth::Yes)
    {
        return context.clone();
    }
    match signatures.subtype(&join, context, model) {
        Truth::Yes => join,
        Truth::No
            if types
                .iter()
                .all(|ty| signatures.subtype(ty, context, model) == Truth::Yes) =>
        {
            context.clone()
        }
        Truth::No => join,
        Truth::Unknown => ResolvedType::Unknown,
    }
}

/// The static type of a built-in arithmetic, bitwise or `String +` operator, else `Unknown`.
pub(super) fn operator_result(
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

fn numeric(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk { library, name },
            ..
        } if library.as_str() == "dart:core" && matches!(name.as_str(), "int" | "double" | "num")
    )
}

fn fixnum_and_int(fixnum: &ResolvedType, int: &ResolvedType) -> bool {
    matches!(
        fixnum,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Package { package, name, .. },
            ..
        } if package.as_str() == "fixnum" && matches!(name.as_str(), "Int32" | "Int64")
    ) && int.interface("dart:core", "int")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use falcon_dart_parser::parse;

    use super::*;
    use crate::resolve::{IdentitySource, LibrarySource};

    #[test]
    fn sdk_identities_remain_canonical_across_plain_and_embedded_contexts() {
        let directory = tempfile::tempdir().unwrap();
        let root = &directory.path().canonicalize().unwrap();
        let sdk = root.join("flutter/bin/cache/dart-sdk/lib");
        let embedded = root.join("embedder/lib");
        std::fs::create_dir_all(&sdk).unwrap();
        std::fs::create_dir_all(&embedded).unwrap();
        std::fs::write(
            sdk.join("libraries.json"),
            r#"{"vm":{"libraries":{"core":{"uri":"core.dart"},"async":{"uri":"async.dart"}}}}"#,
        )
        .unwrap();
        std::fs::write(
            embedded.join("_embedder.yaml"),
            "embedded_libs:\n  dart:core: core.dart\n  dart:async: async.dart\n",
        )
        .unwrap();
        for (lib, parameter) in [(&sdk, "int"), (&embedded, "String")] {
            let operator = if parameter == "int" {
                "bool operator ==(Object other) => true;"
            } else {
                "bool operator ==(Object other);"
            };
            std::fs::write(
                lib.join("core.dart"),
                format!("class Object {{}} class num {{}} class int extends num {{}} \
                 class String {{}} class Type {{}} class Error {{}} class Exception {{}} \
                 class ArgumentError extends Error {{ ArgumentError(String message); }} \
                 class StateError extends Error {{ StateError(String message); }} \
                 class Box<T> {{ Box(T value); }} \
                 abstract class Canonical extends Box<{parameter}> {{ Canonical(super.value); {operator} }}"),
            )
            .unwrap();
            std::fs::write(
                lib.join("async.dart"),
                "class Future<T> {} class Completer<T> { Completer(); }",
            )
            .unwrap();
        }
        let paths = [
            root.join("plain/lib/main.dart"),
            root.join("embedded/lib/main.dart"),
        ];
        let source = "import 'dart:async'; import 'package:dependency/dependency.dart'; final error = ArgumentError('x'); \
                      final state = StateError('x'); int number = 0; String text = ''; \
                      final done = Completer<void>(); class LocalError extends Error {} \
                      class NotAnError {}";
        for (index, path) in paths.iter().enumerate() {
            let app = path.parent().unwrap().parent().unwrap();
            std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::create_dir_all(app.join("dependency/lib")).unwrap();
            let parameter = if index == 0 { "int" } else { "String" };
            std::fs::write(app.join("dependency/lib/dependency.dart"), format!(
                "class PackageBox<T> {{ PackageBox(T value); }} \
                 class PackageCanonical extends PackageBox<{parameter}> {{ PackageCanonical(super.value); }}"
            )).unwrap();
            let packages = if index == 0 {
                serde_json::json!([{"name":"dependency","rootUri":"../dependency","packageUri":"lib/"}])
            } else {
                serde_json::json!([
                    {"name":"sky_engine","rootUri":"../../embedder","packageUri":"lib/"},
                    {"name":"dependency","rootUri":"../dependency","packageUri":"lib/"}
                ])
            };
            std::fs::write(
                app.join(".dart_tool/package_config.json"),
                serde_json::json!({
                    "configVersion": 2,
                    "flutterRoot": format!("file://{}", root.join("flutter").display()),
                    "packages": packages
                })
                .to_string(),
            )
            .unwrap();
            std::fs::write(path, source).unwrap();
        }
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let environment = crate::AnalysisEnvironment::discover_with_sdk_locator(
            paths.iter().map(PathBuf::as_path),
            |config| {
                crate::environment::DartSdk::load(
                    &config?.flutter_root.as_ref()?.join("bin/cache/dart-sdk"),
                )
            },
        );
        let project: Vec<_> = paths
            .iter()
            .map(|path| (path.as_path(), &program, source))
            .collect();
        let external = ExternalLibraries::load(&environment, &project, |path| {
            paths.iter().any(|project| project == path)
        });
        assert_eq!(
            external.libraries().iter().filter(|library| {
                matches!(&library.origin, crate::environment::ExternalOrigin::Sdk(uri) if uri.as_str() == "dart:core")
            }).count(),
            2,
        );
        let files: Vec<_> = paths.iter().map(|path| (path.clone(), &program)).collect();
        let sources: Vec<_> = paths
            .iter()
            .map(|path| IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            })
            .collect();
        let external_sources = crate::ExternalSources {
            libraries: &external,
            environment: &environment,
        };
        let monolithic =
            IdentityIndex::build(&sources, &[source, source], &[], Some(external_sources));
        let base = Arc::new(IdentityIndex::build_external(external_sources));
        let overlay = IdentityIndex::with_external(
            &sources,
            &[source, source],
            &[],
            external_sources,
            base.clone(),
        );
        for (identities, external_identities) in
            [(&monolithic, None), (&overlay, Some(base.as_ref()))]
        {
            let mut types = TypeIndex::from_resolved_files(
                paths.iter().map(|path| {
                    (
                        path.as_path(),
                        LibrarySource {
                            program: &program,
                            has_parse_errors: false,
                            has_unresolved_parts: false,
                        },
                    )
                }),
                identities,
            );
            types.add_external(&external, identities);
            let signatures = if let Some(external_identities) = external_identities {
                let external_signatures = Arc::new(SignatureIndex::build(
                    &[],
                    Some(&external),
                    external_identities,
                    &types,
                ));
                SignatureIndex::with_external(&files, identities, &types, external_signatures)
            } else {
                SignatureIndex::build(&files, Some(&external), identities, &types)
            };
            for path in &paths {
                let model = SemanticModel::new(path, identities, Some(&types));
                for name in ["Canonical", "PackageCanonical"] {
                    let identity = model.resolve_name(name).unwrap();
                    let library = identities.declaration_library(&identity).unwrap();
                    let file = identities.declaration_file(library, name).unwrap();
                    let source = identities.source_path(file).unwrap();
                    let parameter = if source.starts_with(&embedded)
                        || source.starts_with(root.join("embedded"))
                    {
                        "String"
                    } else {
                        "int"
                    };
                    assert_eq!(
                        signatures.constructor(&identity, "new").unwrap().positional,
                        [model.core_type(parameter, false)],
                        "{identity:?} from {source:?}",
                    );
                    if name == "Canonical" {
                        let facts = signatures.declared_member_facts(&identity, "==");
                        assert_eq!(facts.len(), 1);
                        assert_eq!(facts[0].is_abstract, parameter == "String");
                    }
                }
                for (name, library) in [
                    ("int", "dart:core"),
                    ("String", "dart:core"),
                    ("Type", "dart:core"),
                    ("Completer", "dart:async"),
                ] {
                    assert_eq!(
                        model.resolve_name(name),
                        Some(DeclarationIdentity::sdk(library, name))
                    );
                }
                for name in ["error", "state"] {
                    let ty = signatures
                        .value_type(&model.resolve_value(name).unwrap())
                        .unwrap();
                    assert_eq!(
                        signatures.interface_check_sdk(ty, "dart:core", "Error", &model),
                        Truth::Yes
                    );
                }
                for name in ["number", "text", "done", "NotAnError"] {
                    let ty = model
                        .resolve_value(name)
                        .and_then(|id| signatures.value_type(&id).cloned())
                        .unwrap_or_else(|| model.declared_type(&model.resolve_name(name).unwrap()));
                    assert_eq!(
                        signatures.interface_check_sdk(&ty, "dart:core", "Error", &model),
                        Truth::No
                    );
                }
                let number = signatures
                    .value_type(&model.resolve_value("number").unwrap())
                    .unwrap();
                let text = signatures
                    .value_type(&model.resolve_value("text").unwrap())
                    .unwrap();
                assert_eq!(
                    signatures.types_are_unrelated(number, &model.core_type("int", false), &model),
                    Truth::No
                );
                assert_eq!(
                    signatures.types_are_unrelated(text, &model.core_type("String", false), &model),
                    Truth::No
                );
                assert_eq!(
                    signatures.types_are_unrelated(number, text, &model),
                    Truth::Yes
                );
                let done = signatures
                    .value_type(&model.resolve_value("done").unwrap())
                    .unwrap();
                assert_eq!(
                    signatures.interface_check_sdk(done, "dart:async", "Completer", &model),
                    Truth::Yes
                );
                let local = model.declared_type(&model.resolve_name("LocalError").unwrap());
                assert_eq!(
                    signatures.interface_check_sdk(&local, "dart:core", "Error", &model),
                    Truth::Yes
                );
            }
        }
    }

    #[test]
    fn single_file_and_project_inference_share_converged_headers_and_values() {
        with_model(
            r#"
class Root { final int value; Root(this.value); }
class Middle extends Root { Middle(super.value); }
class Leaf extends Middle { Leaf(super.value); }
class Base<T> { T get value => throw 0; T method(T input) => input; }
class Child extends Base<int> {
  get value => 1;
  method(input) => input;
}
final first = second;
final second = third;
final third = Child().value;
final partial = [first, unresolved];
"#,
            |program, model| {
                let single = SignatureIndex::from_program(program, model);
                let project = SignatureIndex::from_project_files(
                    &[(PathBuf::from("/project/lib/main.dart"), program)],
                    model.identities,
                    model.types.unwrap(),
                );
                for name in ["first", "second", "third", "partial"] {
                    let identity = model.resolve_value(name).unwrap();
                    let actual = single.value_type(&identity).unwrap();
                    assert_eq!(Some(actual), project.value_type(&identity));
                    if name == "partial" {
                        assert!(actual.interface("dart:core", "List"));
                        assert!(has_inference_holes(actual));
                    } else {
                        assert!(actual.interface("dart:core", "int"));
                    }
                }
                for name in ["Root", "Middle", "Leaf"] {
                    let identity = model.resolve_name(name).unwrap();
                    let actual = single.constructor(&identity, "new").unwrap();
                    assert_eq!(
                        actual.function_type(),
                        project
                            .constructor(&identity, "new")
                            .unwrap()
                            .function_type(),
                    );
                    assert!(actual.positional[0].interface("dart:core", "int"));
                }
            },
        );
    }

    #[test]
    fn extension_setter_lookup_borrows_indexed_spelling_through_overlays() {
        let mut setters = FxHashMap::default();
        setters.insert(ExtensionMemberName("value=".to_owned()), vec![0]);
        let mut members = ExtensionMembers::default();
        members[1] = setters;
        let base = SignatureIndex {
            extension_members: Some(members),
            ..SignatureIndex::default()
        };
        let overlay = SignatureIndex {
            extension_members: Some(ExtensionMembers::default()),
            external: Some(Arc::new(base)),
            ..SignatureIndex::default()
        };
        let spelling = overlay.extension_access_name("value", true).unwrap();
        assert_eq!(spelling, "value=");
        let stored = &overlay
            .external
            .as_ref()
            .unwrap()
            .extension_members
            .as_ref()
            .unwrap()[1]
            .get_key_value("value")
            .unwrap()
            .0
            .0;
        assert_eq!(spelling.as_ptr(), stored.as_ptr());
        assert_eq!(overlay.extension_access_name("value", false), Some("value"));
        assert_eq!(overlay.extension_access_name("missing", true), None);
    }

    #[test]
    fn extension_setter_lookup_borrows_unindexed_stored_spelling() {
        with_model(
            "class C {} extension E on C { set value(int next) {} }",
            |program, model| {
                let mut signatures = SignatureIndex::from_program(program, model);
                signatures.extension_members = None;
                let spelling = signatures.extension_access_name("value", true).unwrap();
                let stored = signatures.extensions[0]
                    .instance_members
                    .get("value=")
                    .unwrap();
                assert_eq!(spelling, "value=");
                assert_eq!(spelling.as_ptr(), stored.as_ptr());
                assert_eq!(
                    signatures.extension_access_name("value", false),
                    Some("value")
                );
                assert_eq!(signatures.extension_access_name("missing", true), None);
            },
        );
    }

    #[test]
    fn external_overlays_share_facts_and_replace_project_snapshots() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("app/.dart_tool")).unwrap();
        std::fs::create_dir_all(root.path().join("app/lib")).unwrap();
        std::fs::create_dir_all(root.path().join("dep/lib")).unwrap();
        std::fs::write(
            root.path().join("app/.dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[
                {"name":"app","rootUri":"../","packageUri":"lib/"},
                {"name":"dep","rootUri":"../../dep","packageUri":"lib/"}
            ]}"#,
        )
        .unwrap();
        let path = root.path().join("app/lib/main.dart");
        let external_path = root.path().join("dep/lib/external.dart");
        std::fs::write(
            &external_path,
            "class Base<T> { T field; Base(this.field); T method() => field; } \
             int get answer => 1; int imported(int x) => x; \
             extension Tools on Base<int> { int tool() => 1; int implicitTool() => 1; } \
             extension Other on Base<int> { String tool() => ''; } \
             extension GenericTools<T> on Base<T> { T chosen() => field; R choose<R>(R value) => value; } \
             enum Choice { first, second }",
        )
        .unwrap();
        let external_path = std::fs::canonicalize(&external_path).unwrap();
        let old_source = "import 'package:dep/external.dart'; \
            class Child extends Base<int> { Child(): super(1); } \
            final oldValue = answer; final result = Child().method(); \
            final viaExtension = Base<int>(1).implicitTool(); \
            final viaExplicit = Tools(Base<int>(1)).tool(); \
            final viaGeneric = GenericTools<int>(Base<int>(1)).chosen(); \
            final viaGenericMethod = GenericTools(Base<int>(1)).choose<String>('value'); \
            Base<int>? maybeBase; \
            final viaNullable = GenericTools(maybeBase)?.chosen(); \
            final viaNonNull = GenericTools(Base<int>(1))?.chosen(); \
            final viaInferredMethod = GenericTools(Base<int>(1)).choose('value');";
        let new_source = "import 'package:dep/external.dart'; \
            class Child extends Base<String> { Child(): super('x'); } \
            final freshValue = answer; final result = Child().method(); \
            final viaExplicit = Tools(Base<int>(1)).tool(); \
            final viaGeneric = GenericTools<int>(Base<int>(1)).chosen(); \
            final viaGenericMethod = GenericTools(Base<int>(1)).choose<String>('value'); \
            Base<int>? maybeBase; \
            final viaNullable = GenericTools(maybeBase)?.chosen(); \
            final viaNonNull = GenericTools(Base<int>(1))?.chosen(); \
            final viaInferredMethod = GenericTools(Base<int>(1)).choose('value');";
        let falcon_dart_parser::ParseResult {
            program: old_program,
            errors,
            ..
        } = parse(old_source);
        assert!(errors.is_empty(), "{errors:?}");
        let falcon_dart_parser::ParseResult {
            program: new_program,
            errors,
            ..
        } = parse(new_source);
        assert!(errors.is_empty(), "{errors:?}");
        std::fs::write(&path, old_source).unwrap();
        let environment =
            crate::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
        let external = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), &old_program, old_source)],
            // Only this package file belongs to the fixture's external graph.
            |candidate| candidate != external_path,
        );
        assert_eq!(external.libraries().len(), 1);
        let external_sources = || crate::ExternalSources {
            libraries: &external,
            environment: &environment,
        };
        let base_ids = Arc::new(IdentityIndex::build_external(external_sources()));
        let mut base_types = TypeIndex::from_resolved_files([], &base_ids);
        base_types.add_external(&external, &base_ids);
        let base_types = Arc::new(base_types);
        let base_signatures = Arc::new(SignatureIndex::build(
            &[],
            Some(&external),
            &base_ids,
            &base_types,
        ));
        let mut old_identity = None;
        for (source, program, expected) in [
            (old_source, &old_program, "int"),
            (new_source, &new_program, "String"),
        ] {
            let identities = IdentityIndex::with_external(
                &[IdentitySource {
                    path: &path,
                    program,
                    has_parse_errors: false,
                }],
                &[source],
                &[],
                external_sources(),
                Arc::clone(&base_ids),
            );
            let types = TypeIndex::with_external(
                [(
                    path.as_path(),
                    LibrarySource {
                        program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )],
                &identities,
                Arc::clone(&base_types),
            );
            let signatures = SignatureIndex::with_external(
                &[(path.clone(), program)],
                &identities,
                &types,
                Arc::clone(&base_signatures),
            );
            let model = SemanticModel::new(&path, &identities, Some(&types));
            let base = model.resolve_name("Base").unwrap();
            let child = model.resolve_name("Child").unwrap();
            assert_eq!(types.type_kind_identity(&base), Some(TypeKind::Class));
            assert_eq!(types.is_subtype_identity(&child, &base), Truth::Yes);
            assert_eq!(
                types.member_lookup_identity(&child, "method"),
                crate::resolve::MemberResult::Found(MemberKind::Method),
            );
            let imported = model.resolve_value("imported").unwrap();
            assert!(std::ptr::eq(
                signatures.function(&imported).unwrap(),
                base_signatures.function(&imported).unwrap(),
            ));
            assert!(std::ptr::eq(
                signatures.constructor(&base, "new").unwrap(),
                base_signatures.constructor(&base, "new").unwrap(),
            ));
            assert!(signatures.is_value_getter(&model.resolve_value("answer").unwrap()));
            assert!(
                signatures
                    .value_type(&model.resolve_value("result").unwrap())
                    .unwrap()
                    .interface("dart:core", expected)
            );
            let choice = model.resolve_name("Choice").unwrap();
            assert!(signatures.is_enum_value(&choice, "first"));
            let tools = model.resolve_extension("Tools").unwrap();
            assert!(signatures.extension_on_type(&tools).is_some());
            assert!(
                signatures
                    .extension_override_method(&tools, "tool")
                    .is_some()
            );
            let (resolved, _) = signatures
                .resolved_extension_override_method(
                    &tools,
                    signatures.extension_on_type(&tools).unwrap(),
                    &[],
                    "tool",
                    &model,
                )
                .unwrap();
            assert!(resolved.return_type.interface("dart:core", "int"));
            assert!(
                signatures
                    .value_type(&model.resolve_value("viaExplicit").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
            assert!(
                signatures
                    .value_type(&model.resolve_value("viaGeneric").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
            assert!(
                signatures
                    .value_type(&model.resolve_value("viaGenericMethod").unwrap())
                    .unwrap()
                    .interface("dart:core", "String")
            );
            for name in ["viaNullable", "viaNonNull"] {
                let ty = signatures
                    .value_type(&model.resolve_value(name).unwrap())
                    .unwrap();
                assert!(ty.interface("dart:core", "int"), "{name}: {ty:?}");
                assert!(ty.nullable(), "{name}: {ty:?}");
            }
            let inferred = signatures
                .value_type(&model.resolve_value("viaInferredMethod").unwrap())
                .unwrap();
            assert!(inferred.interface("dart:core", "String"));
            assert!(!inferred.nullable());
            if let Some(identity) = &old_identity {
                assert!(signatures.value_type(identity).is_none());
                assert!(
                    signatures
                        .value_type(&model.resolve_value("freshValue").unwrap())
                        .unwrap()
                        .interface("dart:core", "int")
                );
            } else {
                old_identity = Some(model.resolve_value("oldValue").unwrap());
                assert!(
                    signatures
                        .value_type(&model.resolve_value("viaExtension").unwrap())
                        .unwrap()
                        .interface("dart:core", "int")
                );
            }
            let empty_types = TypeIndex::with_external([], &identities, Arc::clone(&base_types));
            assert_eq!(empty_types.type_kind_identity(&base), Some(TypeKind::Class));
            assert!(Arc::ptr_eq(
                signatures.external.as_ref().unwrap(),
                &base_signatures
            ));
        }
    }

    #[test]
    fn signature_map_iteration_preserves_base_and_project_shadowing() {
        let shared = DeclarationIdentity::sdk("dart:core", "shared");
        let external_only = DeclarationIdentity::sdk("dart:core", "externalOnly");
        let project_only = DeclarationIdentity::sdk("dart:core", "projectOnly");
        let mut external = SignatureIndex::default();
        external
            .value_types
            .insert(shared.clone(), ResolvedType::Dynamic);
        external
            .value_types
            .insert(external_only.clone(), ResolvedType::Void);
        let external = Arc::new(external);
        let mut overlay = SignatureMap {
            external: Some((Arc::clone(&external), |base| &base.value_types)),
            ..SignatureMap::default()
        };
        overlay.insert(shared.clone(), ResolvedType::Never);
        overlay.insert(project_only.clone(), ResolvedType::Null);
        assert_eq!(overlay.get(&shared), Some(&ResolvedType::Never));
        assert_eq!(overlay.get(&external_only), Some(&ResolvedType::Void));
        assert!(overlay.contains_key(&external_only));
        assert_eq!(overlay.get(&project_only), Some(&ResolvedType::Null));
        assert_eq!(overlay.local.len(), 2);
        let entries: FxHashMap<_, _> = overlay
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries.get(&shared), Some(&ResolvedType::Never));
        assert_eq!(entries.get(&external_only), Some(&ResolvedType::Void));
        assert_eq!(entries.get(&project_only), Some(&ResolvedType::Null));
        assert_eq!(external.value_type(&shared), Some(&ResolvedType::Dynamic));
    }

    #[test]
    fn typedef_provenance_preserves_resolved_type_size() {
        // The semantic type layout before function typedef metadata was added.
        #[allow(dead_code)]
        enum BeforeTypedefProvenance {
            Unknown,
            Dynamic,
            Void,
            Never,
            Null,
            Interface {
                identity: DeclarationIdentity,
                arguments: Vec<BeforeTypedefProvenance>,
                nullable: bool,
                extension_type: bool,
            },
            TypeParameter {
                id: TypeParameterId,
                bound: Box<BeforeTypedefProvenance>,
                nullable: bool,
            },
            NonNullTypeParameter {
                id: TypeParameterId,
                bound: Box<BeforeTypedefProvenance>,
            },
            Function {
                return_type: Box<BeforeTypedefProvenance>,
                positional: Vec<BeforeTypedefProvenance>,
                named: Vec<(String, BeforeTypedefProvenance)>,
                nullable: bool,
            },
            Record {
                positional: Vec<BeforeTypedefProvenance>,
                named: Vec<(String, BeforeTypedefProvenance)>,
                nullable: bool,
            },
        }

        assert_eq!(
            std::mem::size_of::<FunctionTypeAlias>(),
            std::mem::size_of::<usize>(),
        );
        assert!(
            std::mem::size_of::<ResolvedType>() <= std::mem::size_of::<BeforeTypedefProvenance>(),
        );
    }

    #[test]
    fn no_op_flow_mutations_preserve_shared_scopes() {
        let mut environment = TypeEnvironment::new();
        environment.declare("this", ResolvedType::Dynamic);
        environment.declare("plain", ResolvedType::Dynamic);
        environment.declare("value", ResolvedType::Dynamic);
        environment.declare_unpromotable("blocked", ResolvedType::Dynamic);
        environment.promote("value", ResolvedType::Null);
        environment.promote_this_field("synthetic", ResolvedType::Null);
        environment.enter_callable();
        environment.record_write("blocked");
        environment.mark_captured("blocked");
        let snapshot = environment.clone();

        environment.promote("plain", ResolvedType::Unknown);
        environment.promote("value", ResolvedType::Null);
        environment.promote("blocked", ResolvedType::Null);
        environment.promote("missing", ResolvedType::Null);
        environment.forget("plain");
        environment.forget("missing");
        environment.promote_this_field("blocked", ResolvedType::Null);
        environment.promote_this_field("value", ResolvedType::Null);
        environment.promote_this_field("synthetic", ResolvedType::Null);
        environment.record_write("blocked");
        environment.mark_captured("blocked");
        environment.attach_finally(&snapshot, &snapshot);
        assert!(Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
        assert_eq!(environment.lookup("value"), ResolvedType::Null);
        assert_eq!(environment.lookup("plain"), ResolvedType::Dynamic);
    }

    #[test]
    fn flow_snapshots_copy_only_written_scopes() {
        let mut environment = TypeEnvironment::new();
        environment.declare("outer", ResolvedType::Dynamic);
        environment.push_scope();
        environment.declare("inner", ResolvedType::Dynamic);
        let snapshot = environment.clone();
        assert!(Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
        assert!(Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));

        environment.promote("outer", ResolvedType::Null);
        assert_eq!(snapshot.lookup("outer"), ResolvedType::Dynamic);
        assert_eq!(environment.lookup("outer"), ResolvedType::Null);
        assert!(!Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
        assert!(Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));

        environment.declare("inner", ResolvedType::Null);
        assert_eq!(snapshot.lookup("inner"), ResolvedType::Dynamic);
        assert!(!Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));
        let joined = environment.join(&snapshot);
        assert_eq!(joined.lookup("outer"), ResolvedType::Dynamic);
        assert_eq!(environment.lookup("outer"), ResolvedType::Null);
    }

    #[test]
    fn shared_flow_joins_preserve_capture_demotion() {
        let mut environment = TypeEnvironment::new();
        environment.declare("value", ResolvedType::Dynamic);
        let unchanged = environment.join(&environment);
        assert!(Arc::ptr_eq(&environment.scopes[0], &unchanged.scopes[0]));

        environment.enter_callable();
        environment.assign("value", Vec::new());
        assert!(environment.binding("value").unwrap().captured);
        assert!(environment.binding("value").unwrap().promotable);
        let joined = environment.join(&environment);
        assert!(!joined.binding("value").unwrap().promotable);
        assert!(environment.binding("value").unwrap().promotable);
    }

    #[test]
    fn unknown_generic_arguments_poison_constraints_in_either_order_and_nested() {
        with_model(
            "T pick<T>(T a, T b) => a; T nested<T>(List<T> a, T b) => b; \
             dynamic first(dynamic missing) => pick(missing, 1); \
             dynamic last(dynamic missing) => pick(1, missing); \
             dynamic inside(List<dynamic> missing) => nested(missing, 1); \
             T consume<T>(void Function(T) a, void Function(T) b) => throw 0; \
             void integer(int value) {} \
             dynamic contraFirst(void Function(dynamic) missing) => consume(missing, integer); \
             dynamic contraLast(void Function(dynamic) missing) => consume(integer, missing);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    if !matches!(
                        function.name.name.as_str(),
                        "first" | "last" | "inside" | "contraFirst" | "contraLast"
                    ) {
                        continue;
                    }
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let hole = if function.name.name == "inside" {
                        ResolvedType::Interface {
                            occurrence: Default::default(),
                            identity: DeclarationIdentity::sdk("dart:core", "List"),
                            arguments: vec![ResolvedType::Unknown].into(),
                            nullable: false,
                            extension_type: false,
                        }
                    } else if function.name.name.starts_with("contra") {
                        ResolvedType::Function {
                            shape: None,
                            alias: FunctionTypeAlias::default(),
                            optional_positional: 0,
                            return_type: Arc::new(ResolvedType::Void),
                            positional: vec![ResolvedType::Unknown].into(),
                            named: Arc::from([]),
                            nullable: false,
                        }
                    } else {
                        ResolvedType::Unknown
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.declare("missing", hole);
                    assert_eq!(
                        environment.infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &TypeParameterScope::default()
                        ),
                        ResolvedType::Unknown,
                        "{}",
                        function.name.name,
                    );
                }
                assert_eq!(
                    inference_join(
                        &ResolvedType::Dynamic,
                        &ResolvedType::Void,
                        &signatures,
                        model
                    ),
                    ResolvedType::Void
                );
                assert_eq!(
                    inference_join(
                        &ResolvedType::Void,
                        &ResolvedType::Dynamic,
                        &signatures,
                        model
                    ),
                    ResolvedType::Void
                );
            },
        );
    }

    #[test]
    fn unknown_downward_schema_does_not_poison_argument_inference() {
        with_model(
            "T pick<T>(T a, T b) => a; int use() => pick(1, 2);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = &program.declarations[1] else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    panic!()
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                        Some(&ResolvedType::Unknown),
                        &|_| None,
                    ),
                    model.core_type("int", false),
                );
            },
        );
    }

    #[test]
    fn partial_downward_schemas_leave_whole_variables_for_upward_inference() {
        with_model(
            "import 'dart:async'; \
             T identity<T>(T value) => value; \
             T boundedIdentity<T extends num>(T value) => value; \
             T empty<T>() => throw 0; \
             class StateProvider<T> {} \
             class Ref { T watch<T>(StateProvider<T> provider) => throw 0; } \
             class FutureProvider<T> { FutureProvider(FutureOr<T> Function(Ref) create); } \
             class Pair<A, B> { Pair(A first, B second); } \
             int partial() => identity(1); \
             num complete() => identity(1); \
             int bounded() => boundedIdentity(1); \
             dynamic defaulted() => empty(); \
             Pair<num, int> nested() => Pair(1, 2); \
             FutureProvider<int> provider(StateProvider<int> dep) => \
                 FutureProvider((ref) async => ref.watch(dep));",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let partial = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:async", "FutureOr"),
                    arguments: vec![ResolvedType::Unknown].into(),
                    nullable: false,
                    extension_type: false,
                };
                let partial_pair = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Pair").unwrap(),
                    arguments: vec![model.core_type("num", false), ResolvedType::Unknown].into(),
                    nullable: false,
                    extension_type: false,
                };
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    if !function.type_params.is_empty() {
                        continue;
                    }
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let context = match function.name.name.as_str() {
                        "partial" | "bounded" => Some(&partial),
                        "complete" => Some(&expected),
                        "nested" => Some(&partial_pair),
                        _ => None,
                    };
                    let scope = TypeParameterScope::default();
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert_eq!(
                        environment.infer_with_context(
                            expression,
                            model,
                            &signatures,
                            &scope,
                            context,
                            &|_| None,
                        ),
                        expected,
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn partial_contexts_remain_fallbacks_for_calls_and_tearoffs_without_evidence() {
        with_model(
            "T empty<T>() => throw 0; T identity<T>(T value) => value; \
             dynamic call() => empty(); dynamic tearoff() => identity;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let partial = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:core", "List"),
                    arguments: vec![ResolvedType::Unknown].into(),
                    nullable: false,
                    extension_type: false,
                };
                let mut signature = ResolvedSignature::returning(partial.clone());
                signature.positional.push(partial.clone());
                let function_context = signature.function_type();
                for declaration in &program.declarations[2..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let is_call = function.name.name == "call";
                    let inferred = TypeEnvironment::new().infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                        Some(if is_call { &partial } else { &function_context }),
                        &|_| None,
                    );
                    if is_call {
                        assert_eq!(inferred, partial);
                    } else {
                        let ResolvedType::Function { positional, .. } = &inferred else {
                            panic!("expected an instantiated tear-off")
                        };
                        assert_eq!(inferred.function_return_type(), Some(partial.clone()));
                        assert_eq!(positional.as_ref(), std::slice::from_ref(&partial));
                    }
                }
            },
        );
    }

    #[test]
    fn downward_future_or_contexts_constrain_returned_interface_supertypes() {
        with_model(
            "import 'dart:async'; \
             class Sequence<T> {} class Items<T> extends Sequence<T> {} \
             Items<T> create<T>(T value) => throw 0; \
             Future<Items<T>> pending<T>(T value) => throw 0; \
             FutureOr<Sequence<num>> direct() => create(1); \
             FutureOr<Sequence<num>> future() => pending(1); \
             Future<Sequence<num>> Function() closure() => () async => create(1);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations[4..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let inferred = TypeEnvironment::new().infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                        Some(&expected),
                        &|_| None,
                    );
                    let value = match &inferred {
                        ResolvedType::Function { return_type, .. } => return_type.as_ref(),
                        other => other,
                    };
                    let value = if value.interface("dart:async", "Future") {
                        &value.arguments()[0]
                    } else {
                        value
                    };
                    assert!(
                        value.arguments()[0].interface("dart:core", "num"),
                        "{}: {inferred:?}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn downward_constraints_follow_formal_supertypes() {
        with_model(
            "class Sequence<T> {} class Items<T> extends Sequence<T> {} \
             Items<T> create<T>(T value) => throw 0; \
             Sequence<num> probe() => create(1);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = program.declarations.last().unwrap() else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    panic!()
                };
                let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                let inferred = TypeEnvironment::new().infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                    Some(&expected),
                    &|_| None,
                );
                assert!(inferred.arguments()[0].interface("dart:core", "num"));
            },
        );
    }

    #[test]
    fn async_closure_flattens_returns_before_joining() {
        with_model(
            "import 'dart:async'; \
             class Pending implements Future<int> {} \
             dynamic mixed(bool choose, Future<int> future) => () async { if (choose) return 1; return future; }; \
             dynamic subclass(bool choose, Pending future) => () async { if (choose) return 1; return future; }; \
             dynamic arrow(Pending future) => () async => future;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let scope = TypeParameterScope::default();
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let ResolvedType::Function { return_type, .. } =
                        environment.infer_with_signatures(expression, model, &signatures, &scope)
                    else {
                        panic!()
                    };
                    assert!(return_type.interface("dart:async", "Future"));
                    let argument = &return_type.arguments()[0];
                    assert!(argument.interface("dart:core", "int"));
                }
            },
        );
    }

    #[test]
    fn closure_loop_writes_are_cached_per_file() {
        let paths = [
            PathBuf::from("/project/lib/first.dart"),
            PathBuf::from("/project/lib/second.dart"),
        ];
        let sources = [
            "dynamic f() => () { int? x = 1; while (true) { x = null; } return x; };",
            "dynamic f() => () { int? y = 1; while (true) { y = null; } return y; };",
        ];
        let parsed = sources.map(|source| {
            let parsed = parse(source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            parsed
        });
        let programs = parsed.each_ref().map(|parsed| &parsed.program);
        let identities = IdentityIndex::from_project_files(
            &[
                IdentitySource {
                    path: &paths[0],
                    program: programs[0],
                    has_parse_errors: false,
                },
                IdentitySource {
                    path: &paths[1],
                    program: programs[1],
                    has_parse_errors: false,
                },
            ],
            &[],
        );
        let types = TypeIndex::from_program(programs[0]);
        let files = vec![
            (paths[0].clone(), programs[0]),
            (paths[1].clone(), programs[1]),
        ];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let config = falcon_config::FalconConfig::default();
        for (index, name) in ["x", "y"].into_iter().enumerate() {
            let model = SemanticModel::new(&paths[index], &identities, Some(&types));
            let start = sources[index].find("while").unwrap();
            assert_eq!(
                signatures.flow_writes[&model.file].region_writes[&start],
                HashSet::from([name.to_owned()]),
            );
            let mut context = crate::AnalyzeContext::new(
                &paths[index],
                sources[index],
                &config,
                &parsed[index].syntax,
            );
            context.identities = Some(&identities);
            context.signatures = Some(&signatures);
            assert!(std::ptr::eq(
                context.flow_writes(programs[index]),
                signatures
                    .flow_writes(model.file.unwrap())
                    .unwrap()
                    .as_ref(),
            ));
        }
    }

    #[test]
    fn nullable_receiver_selects_unconstrained_generic_extension() {
        with_model(
            "extension Wide on Object? { int get value => 0; } \
             extension Narrow<T> on T { String get value => ''; }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let (ty, substitutions) = signatures
                    .resolved_extension_getter(&model.core_type("int", true), "value", model)
                    .unwrap();
                let ty = model.substitute(&ty, &substitutions);
                assert!(ty.interface("dart:core", "String"));
            },
        );
    }

    #[test]
    fn nullable_extensions_shadow_members_of_the_non_null_branch() {
        with_model(
            "class String { bool get isEmpty => true; bool check() => true; } \
             extension Nullable on String? { int get isEmpty => 0; int check() => 0; }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let receiver =
                    ResolvedType::interface_identity(model.resolve_name("String").unwrap());
                let nullable = receiver.with_nullable(true);
                let (field, substitutions) = signatures
                    .resolved_extension_getter(&nullable, "isEmpty", model)
                    .unwrap();
                assert!(
                    model
                        .substitute(&field, &substitutions)
                        .interface("dart:core", "int")
                );
                assert!(
                    signatures
                        .resolved_field(&nullable, "isEmpty", model)
                        .is_none()
                );
                let (method, substitutions) = signatures
                    .resolved_member(&nullable, "check", model)
                    .unwrap();
                assert!(
                    model
                        .substitute(&method.return_type, &substitutions)
                        .interface("dart:core", "int")
                );
                assert_eq!(
                    signatures.resolved_extension_owner(&nullable, "isEmpty", model),
                    model.resolve_extension("Nullable"),
                );
                let (field, substitutions) = signatures
                    .resolved_field(&receiver, "isEmpty", model)
                    .unwrap();
                assert!(
                    model
                        .substitute(&field, &substitutions)
                        .interface("dart:core", "bool")
                );
                let (method, substitutions) = signatures
                    .resolved_member(&receiver, "check", model)
                    .unwrap();
                assert!(
                    model
                        .substitute(&method.return_type, &substitutions)
                        .interface("dart:core", "bool")
                );
                assert!(
                    signatures
                        .resolved_extension_owner(&receiver, "isEmpty", model)
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn null_shorting_preserves_declared_nullable_member_receivers() {
        with_model(
            r#"
class Value {}
class Items { Value? operator [](int index) => throw 0; }
class Holder {
  Value? get maybe => throw 0;
  Value get present => throw 0;
  Value? maybeCall() => throw 0;
  Value presentCall() => throw 0;
  Items get items => throw 0;
}
extension Nullable on Value? {
  int get size => 0;
  int measure() => 0;
}
extension NonNullable on Value {
  String get size => '';
  String measure() => '';
}
Holder? holder = throw 0;
final nullableField = holder?.maybe.size;
final nonNullableField = holder?.present.size;
final ownShorting = holder?.maybe?.size;
final parenthesizedField = (holder?.present).size;
final nullableCall = holder?.maybeCall().measure();
final parenthesizedCall = (holder?.presentCall()).measure();
final nullableIndex = holder?.items[0].size;
"#,
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (name, expected, nullable) in [
                    ("nullableField", "int", true),
                    ("nonNullableField", "String", true),
                    ("ownShorting", "String", true),
                    ("parenthesizedField", "int", false),
                    ("nullableCall", "int", true),
                    ("parenthesizedCall", "int", false),
                    ("nullableIndex", "int", true),
                ] {
                    let identity = model.resolve_value(name).unwrap();
                    let ty = signatures.value_type(&identity).unwrap();
                    assert!(ty.interface("dart:core", expected), "{name}: {ty:?}");
                    assert_eq!(ty.nullable(), nullable, "{name}: {ty:?}");
                }
            },
        );
    }

    #[test]
    fn inherited_interface_owner_follows_nested_type_parameter_bounds() {
        with_model(
            "class I { void m() {} } abstract class A implements I {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let a = ResolvedType::interface_identity(model.resolve_name("A").unwrap());
                let s = ResolvedType::TypeParameter {
                    id: sdk_parameter("bounds", 0),
                    bound: Arc::new(a),
                    nullable: false,
                };
                let t = ResolvedType::TypeParameter {
                    id: sdk_parameter("bounds", 1),
                    bound: Arc::new(s.clone()),
                    nullable: false,
                };
                for receiver in [s, t] {
                    assert_eq!(
                        signatures.resolved_member_owner(&receiver, "m", model),
                        model.resolve_name("I"),
                    );
                }
            },
        );
    }

    #[test]
    fn finally_and_capture_mutations_leave_snapshots_unchanged() {
        let mut entry = TypeEnvironment::new();
        entry.declare("value", ResolvedType::Dynamic);
        let mut end = entry.clone();
        end.promote("value", ResolvedType::Null);
        let mut target = entry.clone();
        target.attach_finally(&entry, &end);
        assert_eq!(target.lookup("value"), ResolvedType::Null);
        assert_eq!(entry.lookup("value"), ResolvedType::Dynamic);

        let snapshot = target.clone();
        target.mark_captured("value");
        assert_eq!(target.lookup("value"), ResolvedType::Dynamic);
        assert_eq!(snapshot.lookup("value"), ResolvedType::Null);
        assert!(!snapshot.binding("value").unwrap().captured);
    }

    #[test]
    fn inferred_intersections_demote_before_later_writes() {
        let parameter = ResolvedType::TypeParameter {
            id: sdk_parameter("test", 0),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let intersection = parameter.non_null();
        let mut environment = TypeEnvironment::new();
        environment.declare("value", intersection.clone());
        assert_eq!(environment.declared_type("value"), parameter);
        assert_eq!(environment.lookup("value"), intersection);
        environment.forget("value");
        assert_eq!(environment.lookup("value"), parameter);
    }

    #[test]
    fn unresolved_property_chains_infer_in_linear_work() {
        let source = format!("final value = unknown{};", ".x".repeat(40));
        with_model(&source, |program, model| {
            let TopLevelDecl::Variable(variable) = &program.declarations[0] else {
                panic!("expected variable");
            };
            let expression = variable.declarators[0].initializer.as_ref().unwrap();
            let signatures = SignatureIndex::from_program(program, model);
            let lookups = std::cell::Cell::new(0);
            let lookup = |_: &str| {
                lookups.set(lookups.get() + 1);
                None
            };
            let result = TypeEnvironment::new().infer_with_signatures_using_lookup(
                expression,
                model,
                &signatures,
                &TypeParameterScope::default(),
                &lookup,
            );
            assert_eq!(result, ResolvedType::Unknown);
            assert!(
                lookups.get() <= 200,
                "{} lookups for 40 fields",
                lookups.get()
            );
        });
    }

    #[test]
    fn member_parameter_covariance_includes_overridden_interfaces() {
        with_model(
            r#"
class Leaf extends Middle {
  void run(Object renamed, [Object? extra]) {}
}
class Middle implements Contract {
  void run(Object different, [Object? extra]) {}
}
abstract class Contract {
  void run(covariant Object value, [Object? extra]);
  void named({covariant Object? selected, Object? untouched});
}
class Named implements Contract {
  void run(Object value, [Object? extra]) {}
  void named({Object? untouched, Object? selected, Object? added}) {}
}
mixin Mixed {
  void apply(covariant Object value) {}
}
class Applied with Mixed {
  void apply(Object renamed) {}
}
class Generic<T> {
  void accept(T value) {}
}
class GenericChild extends Generic<int> {
  void accept(int value) {}
}
class StaticBase {
  static void accept(covariant Object value) {}
}
class StaticChild extends StaticBase {
  void accept(Object value) {}
}
class Explicit {
  void accept(covariant Object value) {}
}
"#,
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (owner, name, expected) in [
                    ("Leaf", "run", vec![true, false]),
                    ("Middle", "run", vec![true, false]),
                    ("Named", "named", vec![false, true, false]),
                    ("Applied", "apply", vec![true]),
                    ("GenericChild", "accept", vec![false]),
                    ("StaticChild", "accept", vec![false]),
                    ("Explicit", "accept", vec![true]),
                ] {
                    let identity = model.resolve_name(owner).unwrap();
                    let receiver = ResolvedType::interface_identity(identity);
                    let contracts = signatures
                        .effective_parameter_contracts(&receiver, name, false, model)
                        .unwrap();
                    assert_eq!(
                        contracts
                            .iter()
                            .map(|parameter| parameter.is_covariant)
                            .collect::<Vec<_>>(),
                        expected,
                        "{owner}.{name}",
                    );
                    if owner == "Leaf" {
                        let inherited = signatures
                            .super_parameter_contracts(&receiver, name, false, model)
                            .unwrap();
                        assert_eq!(
                            inherited
                                .iter()
                                .map(|parameter| parameter.is_covariant)
                                .collect::<Vec<_>>(),
                            expected,
                        );
                    }
                }
            },
        );
    }

    #[test]
    fn setter_parameter_covariance_stays_accessor_specific() {
        with_model(
            r#"
abstract class Setters {
  set value(covariant Object input);
  covariant Object field = 0;
}
class Override implements Setters {
  Object get value => 0;
  set value(Object renamed) {}
  set field(Object renamed) {}
}
class FieldOverride implements Setters {
  Object value = 0;
  Object field = 0;
}
class Plain {
  set value(Object input) {}
}
class Explicit {
  set value(covariant Object input) {}
}
class OtherKind {
  void value(covariant Object input) {}
}
class DifferentKind extends OtherKind {
  set value(Object input) {}
}
"#,
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (owner, name, expected) in [
                    ("Override", "value", true),
                    ("Override", "field", true),
                    ("FieldOverride", "value", true),
                    ("FieldOverride", "field", true),
                    ("Plain", "value", false),
                    ("Explicit", "value", true),
                    ("DifferentKind", "value", false),
                ] {
                    let identity = model.resolve_name(owner).unwrap();
                    let facts = signatures.declared_member_facts(&identity, name);
                    let receiver = ResolvedType::interface_identity(identity);
                    let contracts = signatures
                        .effective_parameter_contracts(&receiver, name, true, model)
                        .unwrap();
                    assert_eq!(contracts[0].is_covariant, expected, "{owner}.{name}",);
                    for getter in facts
                        .iter()
                        .filter(|facts| facts.kind == MemberKind::Getter)
                    {
                        assert!(getter.parameter_contracts.as_ref().unwrap().is_empty());
                    }
                }
            },
        );
    }

    #[test]
    fn covariance_distinguishes_interface_signatures_from_contextual_implementations() {
        with_model(
            r#"
class CovariantBase {
  void run(covariant num value) {}
}
mixin PlainMixin {
  void run(num value) {}
}
class Shadowed extends CovariantBase with PlainMixin {
  void run(num value) {}
}
abstract class CovariantInterface {
  void run(covariant num value);
  set value(covariant num input);
}
abstract class PlainInterface {
  void run(num value);
}
abstract class PlainFirst implements PlainInterface, CovariantInterface {}
abstract class CovariantFirst implements CovariantInterface, PlainInterface {}
class PlainChild extends PlainFirst {
  void run(num value) {}
}
class CovariantChild extends CovariantFirst {
  void run(num value) {}
}
class Concrete {
  void run(num value) {}
  set value(num input) {}
}
class Inherited extends Concrete implements CovariantInterface {}
class Deeper extends Inherited {}
mixin ConstraintMixin on Inherited {
  void run(num value) => super.run(value);
}
class PairBase {
  void pair(num left, num right) {}
}
abstract class LeftCovariance {
  void pair(covariant num left, num right);
}
abstract class RightCovariance {
  void pair(num left, covariant num right);
}
class FirstPair extends PairBase implements LeftCovariance {}
class ThroughPair extends FirstPair {}
class ChangedPair extends ThroughPair implements RightCovariance {}
class ContractBase {
  void defaults([int value = 2]) {}
  void covariance(num value) {}
}
abstract class AbstractContract extends ContractBase {
  void defaults([int value = 1]);
  void covariance(covariant num value);
}
class ConcreteContract extends AbstractContract {
  void defaults([int value = 1]) => super.defaults(value);
  void covariance(covariant num value) => super.covariance(value);
}
class ContextWithUnknown extends Concrete implements PlainInterface, CovariantInterface {}
class UnknownOnly extends Concrete implements PlainInterface {}
mixin InheritedMixin implements CovariantInterface {}
class InheritedMixinChild extends Concrete with InheritedMixin {
  void run(num value) {}
}
class Redeclared extends Inherited {
  void run(num value) {}
  set value(num input) {}
}
class Conflict implements CovariantInterface {
  num get run => 0;
}
class ConflictChild extends Conflict {
  void run(num value) {}
}
"#,
            |program, model| {
                let mut signatures = SignatureIndex::from_program(program, model);
                for (owner, expected) in [
                    ("Shadowed", false),
                    ("PlainChild", false),
                    ("CovariantChild", true),
                    ("Concrete", false),
                    ("Redeclared", false),
                    ("InheritedMixinChild", false),
                    ("ConflictChild", false),
                ] {
                    let identity = model.resolve_name(owner).unwrap();
                    let receiver = ResolvedType::interface_identity(identity);
                    let contracts = signatures
                        .effective_parameter_contracts(&receiver, "run", false, model)
                        .unwrap();
                    assert_eq!(contracts[0].is_covariant, expected, "{owner}.run");
                }
                for (owner, name, write, expected) in [
                    ("Concrete", "run", false, false),
                    ("Inherited", "run", false, true),
                    ("Deeper", "run", false, true),
                    ("Concrete", "value", true, false),
                    ("Inherited", "value", true, true),
                    ("Deeper", "value", true, true),
                    ("Redeclared", "value", true, false),
                ] {
                    let receiver =
                        ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                    let contracts = signatures
                        .effective_parameter_contracts(&receiver, name, write, model)
                        .unwrap();
                    assert_eq!(contracts[0].is_covariant, expected, "{owner}.{name}");
                }
                for (owner, expected) in [("Redeclared", true), ("ConstraintMixin", false)] {
                    let receiver =
                        ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                    let contracts = signatures
                        .super_parameter_contracts(&receiver, "run", false, model)
                        .unwrap();
                    assert_eq!(contracts[0].is_covariant, expected, "{owner}.super.run");
                }
                for (owner, expected) in [
                    ("FirstPair", [true, false]),
                    ("ThroughPair", [true, false]),
                    ("ChangedPair", [false, true]),
                ] {
                    let receiver =
                        ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                    let contracts = signatures
                        .effective_parameter_contracts(&receiver, "pair", false, model)
                        .unwrap();
                    assert_eq!(
                        [contracts[0].is_covariant, contracts[1].is_covariant],
                        expected,
                        "{owner}.pair",
                    );
                }
                let receiver = ResolvedType::interface_identity(
                    model.resolve_name("ConcreteContract").unwrap(),
                );
                for name in ["defaults", "covariance"] {
                    let own = signatures
                        .effective_parameter_contracts(&receiver, name, false, model)
                        .unwrap();
                    let inherited = signatures
                        .super_parameter_contracts(&receiver, name, false, model)
                        .unwrap();
                    let base = signatures
                        .declared_member_facts(&model.resolve_name("ContractBase").unwrap(), name)
                        [0]
                    .parameter_contracts
                    .as_ref()
                    .unwrap();
                    assert_eq!(&inherited, base, "{name} concrete super");
                    assert_ne!(own, inherited, "{name} abstract redeclaration");
                }
                let plain = model.resolve_name("PlainInterface").unwrap();
                signatures
                    .member_entries
                    .get_mut(&plain)
                    .unwrap()
                    .get_mut("run")
                    .unwrap()
                    .facts[0]
                    .parameter_contracts = None;
                for (owner, expected) in [("ContextWithUnknown", Some(true)), ("UnknownOnly", None)]
                {
                    let receiver =
                        ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                    let contracts =
                        signatures.effective_parameter_contracts(&receiver, "run", false, model);
                    assert_eq!(
                        contracts
                            .as_ref()
                            .map(|parameters| parameters[0].is_covariant),
                        expected,
                        "{owner}.run",
                    );
                }
            },
        );
    }

    #[test]
    fn super_contracts_skip_abstract_mixin_declarations() {
        with_model(
            r#"
class ConstraintBase { void run([int value = 1]) {} }
class ActualBase extends ConstraintBase { void run([int value = 2]) {} }
mixin AbstractConstraint on ConstraintBase { void run([int value = 1]); }
class ConstraintApplication extends ActualBase with AbstractConstraint {}
class ConstraintChild extends ConstraintApplication {
  void run([int value = 1]) => super.run(value);
}
abstract class Interface { void run([int value = 2]); }
mixin AbstractInterface implements Interface { void run([int value = 2]); }
class InterfaceApplication extends ActualBase with AbstractInterface {}
class InterfaceChild extends InterfaceApplication {
  void run([int value = 2]) => super.run(value);
}
class SetterBase { set value(num input) {} }
mixin AbstractSetter { set value(num input); }
class SetterApplication extends SetterBase with AbstractSetter {}
class SetterChild extends SetterApplication {
  set value(num input) => super.value = input;
}
"#,
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (owner, base, name, write, equal) in [
                    ("ConstraintChild", "ActualBase", "run", false, false),
                    ("InterfaceChild", "ActualBase", "run", false, true),
                    ("SetterChild", "SetterBase", "value", true, true),
                ] {
                    let receiver =
                        ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                    let inherited = signatures
                        .super_parameter_contracts(&receiver, name, write, model)
                        .unwrap();
                    let base = signatures
                        .declared_member_facts(&model.resolve_name(base).unwrap(), name)[0]
                        .parameter_contracts
                        .as_ref()
                        .unwrap();
                    assert_eq!(&inherited, base, "{owner} concrete super");
                    let own = signatures
                        .effective_parameter_contracts(&receiver, name, write, model)
                        .unwrap();
                    assert_eq!(own == inherited, equal, "{owner} forwarding contract");
                }
            },
        );
    }

    fn with_model(source: &str, test: impl FnOnce(&Program, &SemanticModel<'_>)) {
        let path = PathBuf::from("/project/lib/main.dart");
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let sources = [IdentitySource {
            path: &path,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_program(&program);
        let model = SemanticModel::new(&path, &identities, Some(&types));
        test(&program, &model);
    }

    #[test]
    fn substitution_removes_instantiated_function_binders_and_maps_retained_bounds() {
        with_model(
            "T f<T, U extends T>(T input, {required int value}) => throw 0;",
            |program, model| {
                let TopLevelDecl::Function(function) = &program.declarations[0] else {
                    panic!("expected function")
                };
                let mut scope = TypeParameterScope::default();
                scope.push(&function.type_params[..1], model);
                assert!(
                    FunctionTypeShape::from_scope(Vec::new(), &function.type_params, &scope,)
                        .is_none()
                );
                scope.push(&function.type_params, model);
                let shape = FunctionTypeShape::from_scope(
                    vec!["value".to_owned()],
                    &function.type_params,
                    &scope,
                )
                .unwrap();
                assert_eq!(shape.type_parameters.len(), 2);
                let ResolvedType::TypeParameter { id, .. } = &shape.type_parameters[0] else {
                    panic!("expected binder")
                };
                let substitutions = HashMap::from([(id.clone(), ResolvedType::Void)]);
                let signatures = SignatureIndex::from_program(program, model);
                let source = signatures
                    .function(&model.resolve_value("f").unwrap())
                    .unwrap()
                    .function_type();
                let result = model.substitute(&source, &substitutions);
                let ResolvedType::Function {
                    shape: Some(shape),
                    return_type,
                    positional,
                    ..
                } = &result
                else {
                    panic!("expected exact function shape")
                };
                assert_eq!(**return_type, ResolvedType::Void);
                assert_eq!(positional.as_ref(), &[ResolvedType::Void]);
                assert_eq!(shape.required_named.as_ref(), &["value".to_owned()]);
                assert_eq!(shape.type_parameters.len(), 1);
                let ResolvedType::TypeParameter { id, bound, .. } = &shape.type_parameters[0]
                else {
                    panic!("expected retained binder")
                };
                assert_eq!(id.name.as_ref(), "U");
                assert_eq!(**bound, ResolvedType::Void);
                let substitutions = HashMap::from([(id.clone(), ResolvedType::Dynamic)]);
                // An unused binder still changes the generic arity when supplied.
                let ResolvedType::Function {
                    shape: Some(shape), ..
                } = model.substitute(&result, &substitutions)
                else {
                    panic!("expected instantiated function")
                };
                assert!(shape.type_parameters.is_empty());
            },
        );
    }

    #[test]
    fn extension_erasure_distinguishes_nested_arguments_from_representation_cycles() {
        with_model(
            "extension type Id<T>(T value) {} \
             extension type Cycle<T>(Cycle<T> value) {} \
             Id<Id<int>> nested() => throw 0; \
             Cycle<int> cyclic() => throw 0;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations[2..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!("expected function")
                    };
                    let ty = model.resolve_type(function.return_type.as_ref().unwrap());
                    let erased = signatures.extension_type_erasure(&ty, model);
                    if function.name.name == "nested" {
                        assert_eq!(erased, Some(model.core_type("int", false)));
                    } else {
                        assert_eq!(erased, None);
                    }
                }
            },
        );
    }

    #[test]
    fn nested_alias_projections_retain_occurrence_provenance_without_copying_types() {
        let source = r#"
typedef G<T> = Future<T> Function();
typedef F<T> = G<T> Function();
class Items<T> { T get first => throw 0; }
typedef R<T> = (G<T>,) Function();
typedef I<T> = Items<G<T>> Function();
typedef L<T> = Items<G<T>>;
typedef P<T> = (G<T>,);
typedef H<T> = G<T> Function() Function();
class Box<T> {
  final (F<T>, F<T>) pair;
  final (G<T>, G<T>) callbacks;
  final Items<G<T>> items;
  final F<T> f;
  final (R<T>, R<T>) records;
  final (I<T>, I<T>) lists;
  final (L<T>, L<T>) aliases;
  final (P<T>, P<T>) recordAliases;
  final (H<T>, H<T>) nested;
  Box(this.pair, this.callbacks, this.items, this.f, this.records, this.lists, this.aliases, this.recordAliases, this.nested);
}
Box<int> box = throw 0;
bool condition = throw 0;
F<int> f = throw 0;
R<int> r = throw 0;
I<int> i = throw 0;
(G<int>, G<int>) directPair = throw 0;
Items<G<int>> directItems = throw 0;
final pair = box.pair;
final left = pair.$1();
final right = pair.$2();
final separateReturns = condition ? left : right;
final sharedReturn = condition ? left : left;
final separateCalls = condition ? pair.$1() : pair.$2();
final separateMemberCalls = condition ? box.pair.$1() : box.pair.$2();
final namedCallbacks = (first: pair.$1, second: pair.$2);
final namedCalls = condition ? namedCallbacks.first() : namedCallbacks.second();
final sharedNamedCalls = condition ? namedCallbacks.first() : namedCallbacks.first();
final callbacks = box.callbacks;
final sharedProjection = condition ? callbacks.$1 : callbacks.$1;
final siblingProjection = condition ? callbacks.$1 : callbacks.$2;
final repeatedMemberProjection = condition ? box.callbacks.$1 : box.callbacks.$1;
final sharedDirectProjection = condition ? directPair.$1 : directPair.$1;
final separateDirectProjection = condition ? directPair.$1 : directPair.$2;
final repeatedMemberArgument = condition ? box.items.first : box.items.first;
final sharedDirectArgument = condition ? directItems.first : directItems.first;
final sharedDirectCalls = condition ? f() : f();
final separateMemberFunctionCalls = condition ? box.f() : box.f();
final records = box.records;
final a = records.$1();
final b = records.$2();
final siblingRecordReturns = condition ? a.$1 : b.$1;
final sharedRecordReturn = condition ? a.$1 : a.$1;
final siblingRecordCalls = condition ? records.$1().$1 : records.$2().$1;
final sharedDirectRecordCalls = condition ? r().$1 : r().$1;
final joinedRecordReturn = (condition ? a : b).$1;
final sharedJoinedRecordReturn = (condition ? a : a).$1;
final lists = box.lists;
final siblingInterfaceCalls = condition ? lists.$1().first : lists.$2().first;
final sharedDirectInterfaceCalls = condition ? i().first : i().first;
final joinedCallbacks = (condition ? pair.$1 : pair.$2)();
final joinedRecordCallbacks = (condition ? records.$1 : records.$2)().$1;
final sharedJoinedCallbacks = (condition ? pair.$1 : pair.$1)();
final sharedJoinedRecordCallbacks = (condition ? records.$1 : records.$1)().$1;
final aliases = box.aliases;
final siblingAliases = condition ? aliases.$1.first : aliases.$2.first;
final sharedAliases = condition ? aliases.$1.first : aliases.$1.first;
final recordAliases = box.recordAliases;
final siblingRecordAliases = condition ? recordAliases.$1.$1 : recordAliases.$2.$1;
final sharedRecordAliases = condition ? recordAliases.$1.$1 : recordAliases.$1.$1;
final nested = box.nested;
final siblingNested = condition ? nested.$1()() : nested.$2()();
final sharedNested = condition ? nested.$1()() : nested.$1()();
final joinedNested = (condition ? nested.$1() : nested.$2())();
final sharedJoinedNested = (condition ? nested.$1() : nested.$1())();
final intermediate = nested.$1();
"#;
        with_model(source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let g = model.resolve_name("G").unwrap();
            for (name, retains_alias) in [
                ("separateReturns", false),
                ("sharedReturn", true),
                ("separateCalls", false),
                ("separateMemberCalls", false),
                ("namedCalls", false),
                ("sharedNamedCalls", true),
                ("sharedProjection", true),
                ("siblingProjection", false),
                ("repeatedMemberProjection", false),
                ("sharedDirectProjection", true),
                ("separateDirectProjection", false),
                ("repeatedMemberArgument", false),
                ("sharedDirectArgument", true),
                ("sharedDirectCalls", true),
                ("separateMemberFunctionCalls", false),
                ("siblingRecordReturns", false),
                ("sharedRecordReturn", true),
                ("siblingRecordCalls", false),
                ("sharedDirectRecordCalls", true),
                ("joinedRecordReturn", false),
                ("sharedJoinedRecordReturn", true),
                ("siblingInterfaceCalls", false),
                ("sharedDirectInterfaceCalls", true),
                ("joinedCallbacks", false),
                ("joinedRecordCallbacks", false),
                ("sharedJoinedCallbacks", true),
                ("sharedJoinedRecordCallbacks", true),
                ("siblingAliases", false),
                ("sharedAliases", true),
                ("siblingRecordAliases", false),
                ("sharedRecordAliases", true),
                ("siblingNested", false),
                ("sharedNested", true),
                ("joinedNested", false),
                ("sharedJoinedNested", true),
            ] {
                let identity = model.resolve_value(name).unwrap();
                let ResolvedType::Function { alias, .. } =
                    signatures.value_type(&identity).unwrap()
                else {
                    panic!("lost function type for {name}")
                };
                assert_eq!(alias.identity(), retains_alias.then_some(&g), "{name}");
            }
            let ResolvedType::Function { alias, .. } = signatures
                .value_type(&model.resolve_value("intermediate").unwrap())
                .unwrap()
            else {
                panic!("lost unaliased intermediate function")
            };
            assert_eq!(alias.identity(), None);
        });
    }

    #[test]
    fn callable_record_fields_precede_extension_methods() {
        with_model(
            r#"
class Token {}
class Wrong {}
Token callback() => Token();
extension PositionalCollision on (Token Function(),) {
  Wrong $1() => Wrong();
  Token extra() => Token();
}
extension NamedCollision on ({Token Function() action}) {
  Wrong action() => Wrong();
  Token extra() => Token();
}
final positionalRecord = (callback,);
final namedRecord = (action: callback);
final positionalResult = positionalRecord.$1();
final namedResult = namedRecord.action();
final positionalExtension = positionalRecord.extra();
final namedExtension = namedRecord.extra();
"#,
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let token = model.resolve_name("Token").unwrap();
                for name in [
                    "positionalResult",
                    "namedResult",
                    "positionalExtension",
                    "namedExtension",
                ] {
                    let identity = model.resolve_value(name).unwrap();
                    let Some(ResolvedType::Interface { identity, .. }) =
                        signatures.value_type(&identity)
                    else {
                        panic!("lost interface type for {name}")
                    };
                    assert_eq!(identity, &token, "{name}");
                }
            },
        );
    }

    #[test]
    fn sdk_iterable_getters_preserve_projected_alias_origins() {
        with_parsed_core(
            r#"
class Result<T> {}
typedef G<T> = Result<T> Function();
typedef I<T> = List<G<T>> Function();
typedef L<T> = List<G<T>>;
class Box<T> {
  List<G<T>> get list => throw 0;
  (I<T>, I<T>) get functions => throw 0;
  (L<T>, L<T>) get aliases => throw 0;
}
Box<int> box = throw 0;
bool condition = throw 0;
List<G<int>> direct = throw 0;
List<int> integers = throw 0;
final functions = box.functions;
final aliases = box.aliases;
final repeatedMember = condition ? box.list.first : box.list.first;
final returnedSiblings = condition ? functions.$1().first : functions.$2().first;
final aliasedSiblings = condition ? aliases.$1.first : aliases.$2.first;
final sharedAlias = condition ? aliases.$1.first : aliases.$1.first;
final sharedDirect = condition ? direct.first : direct.first;
final integer = integers.first;
"#,
            "library dart.core; class Object {} abstract class int {} \
             abstract class bool {} abstract class Iterable<E> { E get first; } \
             abstract class List<E> implements Iterable<E> {}",
            |_program, model, signatures| {
                let g = model.resolve_name("G").unwrap();
                for (name, retains_alias) in [
                    ("repeatedMember", false),
                    ("returnedSiblings", false),
                    ("aliasedSiblings", false),
                    ("sharedAlias", true),
                    ("sharedDirect", true),
                ] {
                    let identity = model.resolve_value(name).unwrap();
                    let Some(ResolvedType::Function { alias, .. }) =
                        signatures.value_type(&identity)
                    else {
                        panic!("lost SDK iterable function type for {name}")
                    };
                    assert_eq!(alias.identity(), retains_alias.then_some(&g), "{name}");
                }
                let integer = model.resolve_value("integer").unwrap();
                assert!(
                    signatures
                        .value_type(&integer)
                        .unwrap()
                        .interface("dart:core", "int")
                );
            },
        );
    }

    fn with_parsed_core(
        source: &str,
        core: &str,
        test: impl FnOnce(&Program, &SemanticModel<'_>, &SignatureIndex),
    ) {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let sdk = root.join("flutter/bin/cache/dart-sdk/lib");
        std::fs::create_dir_all(sdk.join("core")).unwrap();
        std::fs::write(
            sdk.join("libraries.json"),
            r#"{"vm":{"libraries":{"core":{"uri":"core/core.dart"}}}}"#,
        )
        .unwrap();
        std::fs::write(sdk.join("core/core.dart"), core).unwrap();
        std::fs::create_dir_all(root.join("app/.dart_tool")).unwrap();
        std::fs::create_dir_all(root.join("app/lib")).unwrap();
        std::fs::write(
            root.join("app/.dart_tool/package_config.json"),
            serde_json::json!({
                "configVersion": 2,
                "flutterRoot": format!("file://{}", root.join("flutter").display()),
                "packages": [{"name": "app", "rootUri": "../", "packageUri": "lib/"}]
            })
            .to_string(),
        )
        .unwrap();
        let path = root.join("app/lib/main.dart");
        std::fs::write(&path, source).unwrap();
        let path = crate::canonical_or_lexical(&path);
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        let environment =
            crate::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |config| {
                let root = config?.flutter_root.as_ref()?;
                crate::environment::DartSdk::load(&root.join("bin/cache/dart-sdk"))
            });
        let external = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), &program, source)],
            |candidate| candidate == path.as_path(),
        );
        assert!(!external.is_empty(), "parsed core fixture was not loaded");
        let identities = IdentityIndex::build(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[source],
            &[],
            Some(crate::ExternalSources {
                libraries: &external,
                environment: &environment,
            }),
        );
        let mut types = TypeIndex::from_resolved_files(
            [(
                path.as_path(),
                LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        types.add_external(&external, &identities);
        let signatures = SignatureIndex::build(
            &[(path.clone(), &program)],
            Some(&external),
            &identities,
            &types,
        );
        test(
            &program,
            &SemanticModel::new(&path, &identities, Some(&types)),
            &signatures,
        );
    }

    #[test]
    fn concrete_property_chain_readiness_reuses_receiver_results() {
        const DEPTH: usize = 40;
        crate::with_rules_stack(|| {
            for chain in [".next".repeat(DEPTH), ".next.step()".repeat(DEPTH)] {
                let source = format!(
                    "class Link {{ Link get next => throw 0; Link step() => throw 0; }} \
                 final value = root{chain};",
                );
                with_model(&source, |program, model| {
                    let signatures = SignatureIndex::from_program(program, model);
                    let TopLevelDecl::Variable(variable) = &program.declarations[1] else {
                        panic!("expected variable");
                    };
                    let expression = variable.declarators[0].initializer.as_ref().unwrap();
                    let receiver = ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name("Link").unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    };
                    let lookups = std::cell::Cell::new(0);
                    assert!(signatures.declaration_dependencies_known_using_lookup(
                        expression,
                        model,
                        None,
                        &TypeEnvironment::new(),
                        &TypeParameterScope::default(),
                        &|name| {
                            lookups.set(lookups.get() + 1);
                            (name == "root").then(|| receiver.clone())
                        },
                    ));
                    assert_eq!(lookups.get(), 1, "readiness re-inferred a receiver prefix");
                });
            }
        })
        .unwrap();
    }

    #[test]
    fn root_dependencies_preserve_partial_progress_and_dynamic_roots() {
        for source in [
            "final rows = legacy == null ? [header] : [header]; final header = 'h'; var legacy;",
            "var legacy; final header = 'h'; final rows = legacy == null ? [header] : [header];",
            "final rows = cycle == null ? [header] : [header]; final header = 'h'; final cycle = cycle;",
            "final cycle = cycle; final header = 'h'; final rows = cycle == null ? [header] : [header];",
        ] {
            with_model(source, |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                assert_eq!(
                    signatures.value_type(&model.resolve_value("rows").unwrap()),
                    Some(&ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "List"),
                        arguments: vec![model.core_type("String", false)].into(),
                        nullable: false,
                        extension_type: false,
                    }),
                );
                if let Some(identity) = model.resolve_value("legacy") {
                    assert_eq!(
                        signatures.value_type(&identity),
                        Some(&ResolvedType::Dynamic)
                    );
                }
            });
        }
    }

    #[test]
    fn root_dependency_scopes_do_not_create_shadowing_cycles() {
        for initializer in [
            "[for (final value in <int>[1]) value * 2]",
            "(() { int f(int value) => value; return <int>[f(1)]; })()",
            "(() { for (var value = 0; value < 1; value++) {} return <int>[1]; })()",
            "(() { for (final value in <int>[1]) { print(value); } return <int>[1]; })()",
            "[for (final (value,) in <(int,)>[(1,)]) value]",
            "(() { try {} catch (value) { print(value); } return <int>[1]; })()",
            "(() { final (value,) = (1,); print(value); return <int>[1]; })()",
            "(() { if ((1,) case (var value,)) { print(value); } return <int>[1]; })()",
            "(() { switch (1) { case var value: print(value); } return <int>[1]; })()",
            "[if ((1,) case (var value,)) value]",
            "[switch (1) { var value => value }]",
        ] {
            with_model(
                &format!("final items = {initializer}; final value = items.first;"),
                |program, model| {
                    let signatures = SignatureIndex::from_program(program, model);
                    assert!(signatures.cyclic_values.is_empty(), "{initializer}");
                    assert_eq!(
                        signatures.value_type(&model.resolve_value("items").unwrap()),
                        Some(&ResolvedType::Interface {
                            occurrence: Default::default(),
                            identity: DeclarationIdentity::sdk("dart:core", "List"),
                            arguments: vec![model.core_type("int", false)].into(),
                            nullable: false,
                            extension_type: false,
                        }),
                        "{initializer}",
                    );
                },
            );
        }
    }

    #[test]
    fn field_readiness_respects_closure_parameters() {
        with_parsed_core(
            "class C { static final value = [1].map((value) => value + 1).toList(); }",
            "library dart.core; class Object {} abstract class num {} \
             abstract class int extends num { int operator +(int other); } \
             abstract class Iterable<E> { Iterable<T> map<T>(T toElement(E value)); List<E> toList(); } \
             abstract class List<E> implements Iterable<E> {}",
            |_, model, signatures| {
                assert_eq!(
                    signatures
                        .field(&model.resolve_name("C").unwrap(), "value")
                        .unwrap()
                        .1,
                    ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "List"),
                        arguments: vec![model.core_type("int", false)].into(),
                        nullable: false,
                        extension_type: false,
                    },
                );
            },
        );
    }

    #[test]
    fn inherited_getter_dependencies_wait_for_declaration_inference() {
        with_model(
            "final result = Child().value; final dynamicResult = Explicit().value; \
             final plainResult = Plain().value; final receiver = Child(); \
             final indirect = receiver.value; \
             class Child extends Middle { get value => throw 0; } \
             class Middle extends Base { get value => throw 0; } \
             class Base { final value = later; } \
             final later = Packet(); class Packet {} \
             class Explicit extends Base { dynamic get value => throw 0; } \
             class Plain { get value => throw 0; }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let packet = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Packet").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                for name in ["result", "indirect"] {
                    assert_eq!(
                        signatures.value_type(&model.resolve_value(name).unwrap()),
                        Some(&packet),
                    );
                }
                for name in ["Child", "Middle", "Base"] {
                    assert_eq!(
                        signatures
                            .field(&model.resolve_name(name).unwrap(), "value")
                            .unwrap()
                            .1,
                        packet,
                    );
                }
                for name in ["dynamicResult", "plainResult"] {
                    assert_eq!(
                        signatures.value_type(&model.resolve_value(name).unwrap()),
                        Some(&ResolvedType::Dynamic),
                    );
                }
            },
        );
    }

    #[test]
    fn interface_diamonds_preserve_substitution_in_bounded_lookup() {
        let mut source = String::from(
            "class Packet {} class Root<T> {} \
             class A0<T> implements Root<T> {} class B0<T> implements Root<T> {} \
             class Tail<T> { T get value => throw 0; }",
        );
        for level in 1..24 {
            let previous = level - 1;
            source.push_str(&format!(
                "class A{level}<T> implements A{previous}<T>, B{previous}<T> {{}} \
                 class B{level}<T> implements A{previous}<T>, B{previous}<T> {{}}",
            ));
        }
        source.push_str("class Child extends A23<Packet> implements Tail<Packet> {}");
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            assert!(signatures.cyclic_supertypes.is_some());
            let child = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: model.resolve_name("Child").unwrap(),
                arguments: Vec::new().into(),
                nullable: false,
                extension_type: false,
            };
            assert!(
                signatures
                    .resolved_field(&child, "missing", model)
                    .is_none()
            );
            let (ty, substitutions) = signatures.resolved_field(&child, "value", model).unwrap();
            assert_eq!(
                model.substitute(&ty, &substitutions),
                ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Packet").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            );
        });
    }

    #[test]
    fn missing_declaration_types_use_completed_signatures() {
        with_model(
            "final forward = later; final later = Box(1); get top => forward; \
             final cycle = Box(other); final other = cycle; \
             class Box<T> { Box(T value); T get value => throw 0; } \
             class Values<T> { \
               static final first = second; static final second = Box('text'); \
               static get result => first; final number = 1; get inferred => number; \
               T get typed => throw 0; get generic => typed; \
             }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let box_identity = model.resolve_name("Box").unwrap();
                let box_of = |argument| ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: box_identity.clone(),
                    arguments: vec![argument].into(),
                    nullable: false,
                    extension_type: false,
                };
                for name in ["forward", "later"] {
                    assert_eq!(
                        signatures.value_type(&model.resolve_value(name).unwrap()),
                        Some(&box_of(model.core_type("int", false)))
                    );
                }
                assert_eq!(
                    signatures.value_type(&model.resolve_value("top").unwrap()),
                    Some(&ResolvedType::Dynamic)
                );
                for name in ["cycle", "other"] {
                    assert_eq!(
                        signatures.value_type(&model.resolve_value(name).unwrap()),
                        Some(&ResolvedType::Unknown)
                    );
                }
                let values = model.resolve_name("Values").unwrap();
                for name in ["first", "second"] {
                    assert_eq!(
                        signatures.field(&values, name).unwrap().1,
                        box_of(model.core_type("String", false))
                    );
                }
                assert_eq!(
                    signatures.field(&values, "number").unwrap().1,
                    model.core_type("int", false)
                );
                for name in ["result", "inferred", "generic"] {
                    assert_eq!(
                        signatures.field(&values, name).unwrap().1,
                        ResolvedType::Dynamic
                    );
                }
                assert!(matches!(
                    signatures.field(&values, "typed").unwrap().1,
                    ResolvedType::TypeParameter { .. }
                ));
            },
        );
    }

    #[test]
    fn untyped_overrides_inherit_instantiated_declared_types() {
        with_model(
            "abstract class Base { num get value; num get empty; } \
             class FieldValue extends Base { final value = 1; var empty; } \
             class GetterValue extends Base { get value => 1.5; get empty => 1; } \
             abstract class Generic<T> { T get value; } \
             class GenericValue implements Generic<int> { get value => 1.5; } \
             class GenericField implements Generic<int> { final value = 1.5; } \
             class EarlyValue extends MiddleValue { final value = 1; var empty; } \
             class MiddleValue extends Base { get value => 1.5; get empty => 1; } \
             class StaticValue extends Base { static final value = 1; static get empty => 1.5; } \
             abstract class Other { String get value; } \
             class Conflict extends Base implements Other { final value = 1; get empty => 1; } \
             class GetterConflict extends Base implements Other { get value => 1.5; }",
            |program, model| {
                let mut signatures = SignatureIndex::default();
                signatures.add_program(program, model);
                for owner in ["FieldValue", "GetterValue", "EarlyValue", "MiddleValue"] {
                    let identity = model.resolve_name(owner).unwrap();
                    for name in ["value", "empty"] {
                        assert_eq!(
                            signatures.field(&identity, name).unwrap().1,
                            model.core_type("num", false)
                        );
                    }
                }
                for owner in ["GenericValue", "GenericField"] {
                    let generic = model.resolve_name(owner).unwrap();
                    assert_eq!(
                        signatures.field(&generic, "value").unwrap().1,
                        model.core_type("int", false)
                    );
                }
                let static_value = model.resolve_name("StaticValue").unwrap();
                assert_eq!(
                    signatures.field(&static_value, "value").unwrap().1,
                    model.core_type("int", false)
                );
                assert_eq!(
                    signatures.field(&static_value, "empty").unwrap().1,
                    ResolvedType::Dynamic
                );
                for owner in ["Conflict", "GetterConflict"] {
                    let identity = model.resolve_name(owner).unwrap();
                    assert_eq!(
                        signatures.field(&identity, "value").unwrap().1,
                        ResolvedType::Unknown
                    );
                }
            },
        );
    }

    #[test]
    fn null_global_initializers_infer_dynamic_except_explicit_null() {
        with_model(
            "var mutable = null; final immutable = null; const constant = null; Null explicit = null;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for name in ["mutable", "immutable", "constant"] {
                    let identity = model.resolve_value(name).unwrap();
                    assert_eq!(
                        signatures.value_type(&identity),
                        Some(&ResolvedType::Dynamic)
                    );
                }
                let identity = model.resolve_value("explicit").unwrap();
                assert_eq!(signatures.value_type(&identity), Some(&ResolvedType::Null));
            },
        );
    }

    #[test]
    fn global_partial_inference_converges_without_expanding_recursive_containers() {
        with_parsed_core(
            "class Partial<T> { Partial(T value); int get length => 0; } \
             final nested = [items]; final items = [later]; final later = 1; \
             final self = [self]; final left = [right]; final right = [left]; \
             final missingCount = Partial(missing).length; \
             final selfCount = self.length; final partialSelfCount = Partial(self).length; \
             final missing = [unresolved]; final consumer = [self]; \
             List<int> annotated = [annotated]; final barrier = [annotated]; \
             final shadow = (shadow) => shadow;",
            "library dart.core; class Object {} abstract class int {} \
             abstract class List<E> { int get length; }",
            |_, model, signatures| {
                let value = |name| {
                    signatures
                        .value_type(&model.resolve_value(name).unwrap())
                        .unwrap()
                };
                let nested = value("nested");
                assert!(nested.interface("dart:core", "List"));
                assert!(nested.arguments()[0].interface("dart:core", "List"));
                assert!(nested.arguments()[0].arguments()[0].interface("dart:core", "int"));
                // Cyclic values stay unknown, as does member access on them.
                for name in ["self", "left", "right", "selfCount"] {
                    assert_eq!(value(name), &ResolvedType::Unknown, "{name}");
                }
                let missing = value("missing");
                assert!(missing.interface("dart:core", "List"));
                assert_eq!(missing.arguments(), &[ResolvedType::Unknown]);
                assert!(value("missingCount").interface("dart:core", "int"));
                assert!(value("partialSelfCount").interface("dart:core", "int"));
                let consumer = value("consumer");
                assert!(consumer.interface("dart:core", "List"));
                assert_eq!(consumer.arguments(), &[ResolvedType::Unknown]);
                let barrier = value("barrier");
                assert!(barrier.arguments()[0].interface("dart:core", "List"));
                assert!(barrier.arguments()[0].arguments()[0].interface("dart:core", "int"));
                assert!(matches!(value("shadow"), ResolvedType::Function { .. }));
            },
        );
    }

    #[test]
    fn partial_dependency_retry_is_independent_of_declaration_order() {
        for declarations in [
            "final result = total; final total = count + 1; \
             final nested = [collected]; final collected = [count]; \
             final count = secondCopy.length; final secondCopy = firstCopy; \
             final firstCopy = partial; final partial = Partial([unresolved]);",
            "final partial = Partial([unresolved]); final firstCopy = partial; \
             final secondCopy = firstCopy; final count = secondCopy.length; \
             final collected = [count]; final nested = [collected]; \
             final total = count + 1; final result = total;",
        ] {
            let source = format!(
                "class Partial<T> {{ Partial(T value); int get length => 0; }} {declarations}"
            );
            with_model(&source, |program, model| {
                let files = [(PathBuf::from("/project/lib/main.dart"), program)];
                for signatures in [
                    SignatureIndex::from_program(program, model),
                    SignatureIndex::from_project_files(
                        &files,
                        model.identities,
                        model.types.unwrap(),
                    ),
                ] {
                    for name in ["count", "total", "result"] {
                        let ty = signatures.value_type(&model.resolve_value(name).unwrap());
                        assert!(
                            ty.is_some_and(|ty| ty.interface("dart:core", "int")),
                            "{name}: {ty:?}; {declarations}",
                        );
                    }
                    let collected = signatures
                        .value_type(&model.resolve_value("collected").unwrap())
                        .unwrap();
                    assert!(collected.interface("dart:core", "List"));
                    assert!(collected.arguments()[0].interface("dart:core", "int"));
                    let nested = signatures
                        .value_type(&model.resolve_value("nested").unwrap())
                        .unwrap();
                    assert!(nested.interface("dart:core", "List"));
                    assert_eq!(nested.arguments(), std::slice::from_ref(collected));
                    let partial = signatures
                        .value_type(&model.resolve_value("partial").unwrap())
                        .unwrap();
                    assert!(has_inference_holes(partial), "{partial:?}");
                    assert!(partial.arguments()[0].interface("dart:core", "List"));
                    assert_eq!(partial.arguments()[0].arguments(), &[ResolvedType::Unknown],);
                    for name in ["firstCopy", "secondCopy"] {
                        assert_eq!(
                            signatures.value_type(&model.resolve_value(name).unwrap()),
                            Some(partial),
                            "{name}; {declarations}",
                        );
                    }
                }
            });
        }
    }

    #[test]
    fn imported_partial_global_inference_is_independent_of_file_order() {
        let main_path = PathBuf::from("/project/lib/main.dart");
        let origin_path = PathBuf::from("/project/lib/origin.dart");
        let falcon_dart_parser::ParseResult {
            program: main,
            errors,
            ..
        } = parse(
            "import 'origin.dart' as origin; final items = [origin.later]; \
             dynamic readItems() => items; dynamic readNull() => origin.empty; \
             final cycle = [origin.other];",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let falcon_dart_parser::ParseResult {
            program: origin,
            errors,
            ..
        } = parse(
            "import 'main.dart' as root; final later = 1; var empty = null; \
             final other = [root.cycle];",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let sources = [
            IdentitySource {
                path: &main_path,
                program: &main,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &origin_path,
                program: &origin,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        for files in [
            [(main_path.clone(), &main), (origin_path.clone(), &origin)],
            [(origin_path.clone(), &origin), (main_path.clone(), &main)],
        ] {
            let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
            let model = SemanticModel::new(&main_path, &identities, Some(&types));
            let items = signatures
                .value_type(&model.resolve_value("items").unwrap())
                .unwrap();
            assert!(items.arguments()[0].interface("dart:core", "int"));
            let origin_model = SemanticModel::new(&origin_path, &identities, Some(&types));
            for identity in [
                model.resolve_value("cycle").unwrap(),
                origin_model.resolve_value("other").unwrap(),
            ] {
                assert_eq!(
                    signatures.value_type(&identity),
                    Some(&ResolvedType::Unknown)
                );
            }
            for (declaration, expected) in main.declarations[1..]
                .iter()
                .zip([items.clone(), ResolvedType::Dynamic])
            {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                    panic!()
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        body,
                        &model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    expected,
                );
            }
        }
    }

    #[test]
    fn bound_static_access_heads_use_instance_types_and_lexical_lookup() {
        with_model(
            "class Root { static int value = 1; } \
             class Other { Other get child => this; String get value => ''; } \
             dynamic direct(Other Root) => Root.value; \
             dynamic nested(Other Root) => Root.child.value;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                let other = ResolvedType::interface_identity(model.resolve_name("Other").unwrap());
                for declaration in &program.declarations[2..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                        panic!()
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert!(
                        environment
                            .infer_with_signatures(body, model, &signatures, &scope)
                            .interface("dart:core", "String")
                    );
                    assert!(
                        TypeEnvironment::new()
                            .infer_with_signatures_using_lookup(
                                body,
                                model,
                                &signatures,
                                &scope,
                                &|name| (name == "Root").then(|| other.clone()),
                            )
                            .interface("dart:core", "String")
                    );
                }
            },
        );
    }

    #[test]
    fn named_mixin_applications_preserve_inherited_collection_arguments() {
        with_model(
            "abstract class Base<A, B> implements List<B> { B get first; } \
             mixin ReadOnly<E> {} \
             abstract class Alias<K, V> = Base<K, V> with ReadOnly<V>; \
             abstract class View<T> extends Alias<String, T> {} \
             View<int> view;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let receiver = signatures
                    .value_type(&model.resolve_value("view").unwrap())
                    .unwrap();
                let iterable = signatures
                    .instantiated_supertype(receiver, "dart:core", "Iterable", model)
                    .unwrap();
                assert!(iterable.arguments()[0].interface("dart:core", "int"));
                let (field, substitutions) =
                    signatures.resolved_field(receiver, "first", model).unwrap();
                assert!(
                    model
                        .substitute(&field, &substitutions)
                        .interface("dart:core", "int")
                );
            },
        );
    }

    #[test]
    fn executable_const_bodies_exclude_external_constructors() {
        with_model(
            "class Value { const Value(); external const factory Value.environment(String name); }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let identity = model.resolve_name("Value").unwrap();
                assert!(signatures.const_constructor(&identity, "new").is_some());
                assert!(
                    signatures
                        .const_constructor(&identity, "environment")
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn extension_method_resolution_preserves_receiver_uncertainty_and_nullability() {
        with_model(
            "extension OnObject on Object { int strict() => 0; } extension OnNullable on Object? { int nullable() => 0; }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for receiver in [ResolvedType::Dynamic, ResolvedType::Unknown] {
                    assert!(
                        signatures
                            .resolved_member(&receiver, "strict", model)
                            .is_none()
                    );
                    assert!(
                        signatures
                            .resolved_member(&receiver, "nullable", model)
                            .is_none()
                    );
                }
                let receiver = model.core_type("String", true);
                assert!(
                    signatures
                        .resolved_member(&receiver, "strict", model)
                        .is_none()
                );
                assert!(
                    signatures
                        .resolved_member(&receiver, "nullable", model)
                        .is_some()
                );
                assert!(
                    signatures
                        .resolved_member(&receiver.with_nullable(false), "strict", model)
                        .is_some()
                );
            },
        );
    }

    #[test]
    fn collection_scopes_and_record_contexts_infer_without_sdk_sources() {
        with_model(
            "abstract class Source<A, B> implements Iterable<B> {} \
             List<T> empty<T>() => []; \
             dynamic list(Source<String, int> source) => [for (var item in source) item]; \
             dynamic map(Source<String, int> source) => {for (var item in source) item: item}; \
             dynamic counted() => [for (var index = 0; index < 2; index++) index]; \
             dynamic promoted(Object? value) => [if (value is int) value]; \
             dynamic record() => (1, name: 'text'); \
             dynamic positional() => (1, name: 'text').$1; \
             dynamic named() => (1, name: 'text').name; \
             (List<int>, {List<String> names}) contextual() => (empty(), names: empty());",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations[2..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                        panic!()
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let expected = function
                        .return_type
                        .as_ref()
                        .map(|ty| model.resolve_type(ty));
                    let actual = environment.infer_with_context(
                        body,
                        model,
                        &signatures,
                        &scope,
                        expected.as_ref(),
                        &|_| None,
                    );
                    let int = model.core_type("int", false);
                    let string = model.core_type("String", false);
                    match function.name.name.as_str() {
                        "list" | "counted" | "promoted" => assert_eq!(actual.arguments(), &[int]),
                        "map" => assert_eq!(actual.arguments(), &[int.clone(), int]),
                        "positional" => assert_eq!(actual, int),
                        "named" => assert_eq!(actual, string),
                        "record" => assert_eq!(
                            actual,
                            ResolvedType::Record {
                                occurrence: Default::default(),
                                positional: vec![int].into(),
                                named: vec![("name".into(), string)].into(),
                                nullable: false
                            }
                        ),
                        "contextual" => assert_eq!(Some(actual), expected),
                        _ => panic!(),
                    }
                }
            },
        );
    }

    #[test]
    fn contextual_generic_spreads_and_record_fields_have_exact_types() {
        with_model(
            "Map<K, V> emptyEntries<K, V>() => {}; List<T> emptyElements<T>() => []; \
             Map<int, String> map() => {...emptyEntries()}; \
             Map<int, String> conditional(bool flag) => {if (flag) ...emptyEntries()}; \
             Map<int, String> loop() => {for (var i = 0; i < 2; i++) ...emptyEntries()}; \
             Map<int, String> nullable() => {...?emptyEntries()}; \
             List<int> list() => [...emptyElements()]; \
             (List<int>, {List<String> names}) record() => (emptyElements(), names: emptyElements());",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations[2..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                        panic!()
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let actual = environment.infer_with_context(
                        body,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    );
                    assert_eq!(actual, expected, "{}", function.name.name);
                }
            },
        );
    }

    #[test]
    fn spread_only_braces_infer_map_shape_from_operands_without_context() {
        with_parsed_core(
            "abstract class Source extends Map<String, String> {} \
             Map<String, String> direct(Map<String, String> input) => {...input}; \
             Map<String, String> nullable(Map<String, String>? input) => {...?input}; \
             Map<String, String> inherited(Source input) => {...input}; \
             Map<String, String> conditional(bool flag, Map<String, String> input) => {if (flag) ...input}; \
             Map<String, String> loop(List<Map<String, String>> inputs) => {for (final input in inputs) ...input}; \
             String value(Map<String, String> input) => {...input}.values.first; \
             Set<String> set(Set<String> input) => {...input}; \
             List<String> list(List<String> input) => [...input];",
            "library dart.core; class Object {} abstract class String {} abstract class bool {} \
             abstract class Iterable<E> { E get first; } \
             abstract class Map<K, V> { Iterable<V> get values; } \
             abstract class Set<E> implements Iterable<E> {} \
             abstract class List<E> implements Iterable<E> {}",
            |program, model, signatures| {
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let scope = TypeParameterScope::default();
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, signatures, &scope),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn collection_literal_element_inference() {
        let cases: &[(&str, &str, &[Option<&str>])] = &[
            ("['a', 'b']", "List", &[Some("String")]),
            ("{1, 2}", "Set", &[Some("int")]),
            ("[1, 2.5]", "List", &[Some("num")]),
            ("{1.5, 2}", "Set", &[Some("num")]),
            ("[i, ni]", "List", &[Some("int?")]),
            ("[ni, i]", "List", &[Some("int?")]),
            ("[ni, ni]", "List", &[Some("int?")]),
            ("[ni, d]", "List", &[Some("num?")]),
            ("[null, 'a']", "List", &[Some("String?")]),
            ("['a', null]", "List", &[Some("String?")]),
            ("[null, null]", "List", &[Some("Null")]),
            ("[true, 1]", "List", &[Some("Object")]),
            ("[null, true, 1]", "List", &[Some("Object?")]),
            ("[a, a]", "List", &[Some("A")]),
            ("[a, b]", "List", &[Some("Object")]),
            ("[missing, 1]", "List", &[None]),
            ("[x, 1]", "List", &[Some("dynamic")]),
            ("[]", "List", &[Some("dynamic")]),
            ("{}", "Map", &[Some("dynamic"), Some("dynamic")]),
            ("{'a': 1, 'b': 2.5}", "Map", &[Some("String"), Some("num")]),
            (
                "{null: 'a', 1: null}",
                "Map",
                &[Some("int?"), Some("String?")],
            ),
            ("{'a': a, 'b': b}", "Map", &[Some("String"), Some("Object")]),
            ("<num>[1, 2]", "List", &[Some("num")]),
            ("<String>{}", "Set", &[Some("String")]),
            (
                "<String, num>{'a': 1}",
                "Map",
                &[Some("String"), Some("num")],
            ),
            ("[...[1]]", "List", &[Some("int")]),
            ("[if (true) 1]", "List", &[Some("int")]),
            ("[for (final n in [1]) n]", "List", &[Some("int")]),
            ("{...{1}}", "Set", &[Some("int")]),
            (
                "{'known': 1, ...{'a': 1}}",
                "Map",
                &[Some("String"), Some("int")],
            ),
            (
                "{'known': 1, if (true) 'a': 1}",
                "Map",
                &[Some("String"), Some("int")],
            ),
            (
                "{'known': 1, for (final n in [1]) n: n}",
                "Map",
                &[Some("Object"), Some("int")],
            ),
            ("<int>[...[1]]", "List", &[Some("int")]),
            (
                "<String, int>{...{'a': 1}}",
                "Map",
                &[Some("String"), Some("int")],
            ),
        ];
        let mut source = String::from("class A {} class B {}");
        for (index, (expression, _, _)) in cases.iter().enumerate() {
            use std::fmt::Write;
            write!(
                source,
                "dynamic f{index}(int i, int? ni, double d, dynamic x, A a, B b) => {expression};"
            )
            .unwrap();
        }
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for ((expression, name, arguments), declaration) in
                cases.iter().zip(&program.declarations[2..])
            {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                    panic!()
                };
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let actual = environment.infer_with_signatures(body, model, &signatures, &scope);
                assert!(
                    actual.interface("dart:core", name),
                    "{expression}: {actual:?}"
                );
                assert_eq!(actual.arguments().len(), arguments.len(), "{expression}");
                for (actual, expected) in actual.arguments().iter().zip(*arguments) {
                    let expected = expected.map_or(ResolvedType::Unknown, |ty| {
                        let falcon_dart_parser::ParseResult {
                            program, errors, ..
                        } = parse(&format!("{ty} expected;"));
                        assert!(errors.is_empty(), "{errors:?}");
                        let TopLevelDecl::Variable(variable) = &program.declarations[0] else {
                            panic!()
                        };
                        model.resolve_type(variable.var_type.as_ref().unwrap())
                    });
                    assert_eq!(*actual, expected, "{expression}");
                }
            }
        });
    }

    #[test]
    fn raw_owner_parameters_use_declared_bounds() {
        with_model(
            "class Box<T> { T? get value => null; } class Bounded<T extends num> { T? get value => null; } dynamic raw(Box box) => box.value; num? bounded(Bounded box) => box.value;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, &signatures, &scope),
                        model.resolve_type(function.return_type.as_ref().unwrap())
                    );
                }
            },
        );
    }

    #[test]
    fn generic_closure_inputs_complete_before_upward_bound_inference() {
        with_model(
            "T bounded<T extends Object>(bool Function(T) callback) => throw 0; \
             T unbounded<T>(bool Function(T) callback) => throw 0; \
             T typed<T>(void Function(T) first, void Function(T) second) => throw 0; \
             void consumeNum(num value) {} \
             Object useBounded() => bounded((value) => true); \
             Object? useUnbounded() => unbounded((value) => true); \
             num useTyped() => typed((num value) {}, (value) {}); \
             num useReverseTyped() => typed((value) {}, (num value) {}); \
             num useOrdinary() => typed(consumeNum, (value) {}); \
             num useReverseOrdinary() => typed((value) {}, consumeNum);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations[4..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let Expr::Call {
                        callee,
                        type_args,
                        args,
                        ..
                    } = expression.as_ref()
                    else {
                        panic!()
                    };
                    let environment = TypeEnvironment::new();
                    let context = InferenceContext {
                        model,
                        signatures: &signatures,
                        type_parameters: &scope,
                        lookup: &|_| None,
                        argument_fallback: None,
                    };
                    let invocation = environment
                        .resolved_invocation(callee, type_args, &context)
                        .unwrap();
                    let mut argument_contexts = HashMap::new();
                    let substitutions = environment.infer_signature_substitutions_with_result(
                        invocation.inference_signature(),
                        args,
                        type_args,
                        &context,
                        None,
                        Some(&mut argument_contexts),
                    );
                    assert_eq!(
                        model.substitute(&invocation.signature.return_type, &substitutions),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                    );
                    assert_eq!(argument_contexts.len(), 1);
                    let ResolvedType::Function { positional, .. } =
                        argument_contexts.values().next().unwrap()
                    else {
                        panic!()
                    };
                    assert_eq!(positional.as_ref(), &[model.core_type("Object", true)]);
                }
            },
        );
    }

    #[test]
    fn sibling_closure_results_contextualize_positional_and_named_inputs() {
        with_model(
            "class Target {} \
             class Owner<A> { B method<B>(void Function(A) callback) => throw 0; } \
             T positional<T>(T Function() create, void Function(T) dispose) => create(); \
             T named<T>({required T Function() create, required void Function(T) dispose}) => create(); \
             T reverse<T>(void Function(T) dispose, T Function() create) => create(); \
             T pick<T>(T Function() first, T Function() second) => first(); \
             Target usePositional() => positional(() => Target(), (value) {}); \
             Target useNamed() => named(create: () => Target(), dispose: (value) {}); \
             Target useReverse() => reverse((value) {}, () => Target()); \
             Target useReverseNamed() => named(dispose: (value) {}, create: () => Target()); \
             num independentNumbers() => pick(() => 1.5, () => 2); \
             List<dynamic> independentLists() => pick(() => <int>[], () => []); \
             dynamic ownerHole(Owner<Missing> owner) => owner.method((value) {});",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations[6..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let Expr::Call {
                        callee,
                        type_args,
                        args,
                        ..
                    } = expression.as_ref()
                    else {
                        panic!()
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let context = InferenceContext {
                        model,
                        signatures: &signatures,
                        type_parameters: &scope,
                        lookup: &|_| None,
                        argument_fallback: None,
                    };
                    let invocation = environment
                        .resolved_invocation(callee, type_args, &context)
                        .unwrap();
                    let mut argument_contexts = HashMap::new();
                    let substitutions = environment.infer_signature_substitutions_with_result(
                        invocation.inference_signature(),
                        args,
                        type_args,
                        &context,
                        None,
                        Some(&mut argument_contexts),
                    );
                    assert_eq!(
                        model.substitute(&invocation.signature.return_type, &substitutions),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                    );
                    assert!(argument_contexts.is_empty(), "{}", function.name.name);
                }
            },
        );
    }

    #[test]
    fn conditional_callable_and_raw_interface_joins_preserve_inference() {
        with_model(
            "class S<T> { final T data; S(this.data); } class E {} \
             String encode(String value, {int encoding = 0}) => value; \
             String Function(String) callback(bool flag) => flag ? encode : (e) => e; \
             S<dynamic> choose(dynamic value) => value is S ? value : S(E());",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    if function.name.name == "encode" {
                        continue;
                    }
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert_eq!(
                        environment.infer_with_context(
                            expression,
                            model,
                            &signatures,
                            &scope,
                            Some(&expected),
                            &|_| None,
                        ),
                        expected,
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn conditional_generic_tearoffs_preserve_alpha_equivalent_binders() {
        with_model(
            "T first<T extends num>(T value) => value; \
             U second<U extends num>(U value) => value; \
             int joined(bool flag) => (flag ? first : second)(1);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = &program.declarations[2] else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    panic!()
                };
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        None,
                        &|_| None,
                    ),
                    model.core_type("int", false),
                );
            },
        );
    }

    #[test]
    fn primary_private_fields_expose_public_named_signature_labels() {
        with_model(
            "class C({required final double _progress, final int _count = 0});",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let identity = model.resolve_name("C").unwrap();
                let signature = signatures.constructor(&identity, "new").unwrap();
                assert_eq!(
                    signature.named.get("progress"),
                    Some(&model.core_type("double", false))
                );
                assert_eq!(
                    signature.named.get("count"),
                    Some(&model.core_type("int", false))
                );
                assert!(!signature.named.contains_key("_progress"));
                assert!(signature.defaults.contains_key("count"));
                assert_eq!(
                    signature.shape.as_ref().unwrap().required_named.as_ref(),
                    &["progress"]
                );
            },
        );
    }

    #[test]
    fn generic_calls_infer_from_inherited_method_tearoffs() {
        with_model(
            "abstract class Random { double nextDouble(); } \
             class Mock {} class MockRandom extends Mock implements Random {} \
             class When<T> { T thenReturn(T value) => value; } \
             When<T> when<T>(T Function() callback) => throw 0; \
             double probe(MockRandom random) => when(random.nextDouble).thenReturn(1.0);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = &program.declarations[5] else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    panic!()
                };
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_signatures(expression, model, &signatures, &scope),
                    model.core_type("double", false),
                );
            },
        );
    }

    #[test]
    fn generic_calls_infer_from_arguments_and_result_context() {
        with_model(
            concat!(
                "import 'dart:async'; T identity<T>(T value) => value; T empty<T>() => throw 0; T merge<T>(T first, T second) => first; T namedIdentity<T>({required T value}) => value; T futureValue<T>(FutureOr<T> value) => throw 0; ",
                "T transform<T>(T Function(T) callback, T value) => callback(value); T inputType<T>(void Function(T) callback) => throw 0; ",
                "class Box<T> { final T value; Box(this.value); Box.named(this.value); T choose<U>(U input, T value) => value; U convert<U>(U value) => value; } class Fields { int Function() callback; Fields(this.callback); } ",
                "int arg() => identity(1); String named() => identity<String>('x'); int namedArgument() => namedIdentity(value: 1); Box<int> create() => Box(1); Box<int> explicitNew() => new Box(1); Box<int> explicitNamed() => Box<int>.named(1); Box<int> cascade() => Box(1)..convert(2); ",
                "int method(Box<String> box) => box.convert(1); String owner(Box<String> box) => box.choose(1, 'x'); int variable(int Function() callback) => callback(); int field(Fields fields) => fields.callback(); ",
                "int variance(int Function(Object) callback) => transform(callback, 1); int upperOnly(void Function(int) callback) => inputType(callback); ",
                "typedef NumBox = Box<num>; NumBox aliasNew() => new NumBox(1); num nestedSibling() => merge(identity(1), 2.0); int? nullableJoin(int? value) => merge(1, value); int? nullableCall(Fields? fields) => fields?.callback(); ",
                "int lexicalClosure() => transform((int x) { final value = x * 2; return value; }, 1); int lexicalArrow() => transform((int x) => x, 1); int tearoffTarget() => 1; int Function() tearoff() => tearoffTarget; num contextNumber() => identity(1.0); ",
                "Box<num> contextBoxNumber() => Box(1); Box<double> contextBoxDouble() => Box(1.0); double contextDouble() => identity(1.0); int? futureNullable(Future<int>? future) => futureValue(future); ",
                "num common(bool flag) => identity(flag ? 1 : 2.0); num lowerBounds() => merge(1, 2.0); int futureScalar() => futureValue(1); int futureWrapped(Future<int> future) => futureValue(future); int context() => empty(); int contextNested() => identity(empty());",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    if !function.type_params.is_empty() {
                        continue;
                    }
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert_eq!(
                        environment.infer_with_context(
                            expression,
                            model,
                            &signatures,
                            &scope,
                            Some(&expected),
                            &|_| None
                        ),
                        expected,
                        "{}",
                        function.name.name,
                    );
                    if !function.name.name.starts_with("context") {
                        assert_eq!(
                            environment.infer_with_signatures(
                                expression,
                                model,
                                &signatures,
                                &scope
                            ),
                            expected
                        );
                    } else if function.name.name == "contextNumber"
                        || function.name.name == "contextDouble"
                    {
                        assert_eq!(
                            environment.infer_with_signatures(
                                expression,
                                model,
                                &signatures,
                                &scope
                            ),
                            model.core_type("double", false)
                        );
                    } else if function.name.name == "contextBoxNumber" {
                        let ResolvedType::Interface { arguments, .. } = environment
                            .infer_with_signatures(expression, model, &signatures, &scope)
                        else {
                            panic!()
                        };
                        assert_eq!(arguments.as_ref(), &[model.core_type("int", false)]);
                    }
                }
            },
        );
    }

    #[test]
    fn omitted_generic_arguments_use_defaults_without_erasing_unknown_evidence() {
        with_model(
            "class Box<T> { Box([T? value]); } \
             class Bounded<T extends num> { Bounded(); } \
             typedef Alias<T> = Box<T>; \
             Box<dynamic> omitted() => Box(); \
             Box<dynamic> alias() => Alias(); \
             Bounded<num> bounded() => Bounded(); \
             Box<int> explicit() => Box<int>(); \
             Box<int> supplied() => Box(1); \
             Box<dynamic> unresolved() => Box(missing);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let actual = TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope,
                    );
                    if function.name.name == "unresolved" {
                        assert_eq!(actual.arguments(), &[ResolvedType::Unknown]);
                    } else {
                        assert_eq!(
                            actual,
                            model.resolve_type(function.return_type.as_ref().unwrap()),
                            "{}",
                            function.name.name,
                        );
                    }
                }
            },
        );
    }

    #[test]
    fn function_context_coerces_callable_values_but_not_unrelated_values() {
        with_model(
            "class Callable { void call() {} } \
             void Function() callback(Callable value) => value; \
             Callable ordinary(Callable value) => value; \
             void Function() unrelated(Object value) => value;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let actual = environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    );
                    let parameter = model
                        .resolve_formal_parameter_in(&function.params().unwrap().params[0], &scope);
                    assert_eq!(
                        actual,
                        if function.name.name == "unrelated" {
                            parameter
                        } else {
                            expected
                        },
                        "{}",
                        function.name.name,
                    );
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, &signatures, &scope),
                        model.resolve_formal_parameter_in(
                            &function.params().unwrap().params[0],
                            &scope
                        ),
                    );
                }
            },
        );
    }

    #[test]
    fn record_schema_downcasts_dynamic_fields_without_downcasting_object_fields() {
        with_model(
            "(String, int) dynamicField(dynamic value) => (value, 1); \
             (String, int) objectField(Object value) => (value, 1); \
             String standalone(dynamic value) => value;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let actual = environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    );
                    match function.name.name.as_str() {
                        "dynamicField" => assert_eq!(actual, expected),
                        "objectField" => {
                            let ResolvedType::Record { positional, .. } = actual else {
                                panic!()
                            };
                            assert_eq!(positional[0], model.core_type("Object", false));
                        }
                        "standalone" => assert_eq!(actual, ResolvedType::Dynamic),
                        _ => unreachable!(),
                    }
                }
            },
        );
    }

    #[test]
    fn mixin_application_constructors_retain_names_types_and_defaults() {
        with_model(
            concat!(
                "class Base<T> { Base(T value); Base.named(T value, [int count = 7]); ",
                "factory Base.factory(T value) => Base(value); } ",
                "mixin Extra {} class Alias<U> = Base<List<U>> with Extra; ",
                "class Chained<V> = Alias<V> with Extra; ",
                "class Child extends Chained<String> { Child(super.value, [super.count]) : super.named(); }",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let base = model.resolve_name("Base").unwrap();
                for name in ["Alias", "Chained"] {
                    let owner = model.resolve_name(name).unwrap();
                    assert!(signatures.constructor(&owner, "new").is_some());
                    assert!(signatures.constructor(&owner, "factory").is_none());
                    let named = signatures.constructor(&owner, "named").unwrap();
                    assert_eq!(
                        named.owner_parameters,
                        signatures.type_parameters(&owner).unwrap()
                    );
                    assert!(matches!(
                        &named.defaults["count"],
                        ParameterDefault::Inherited { owner, .. } if *owner == base
                    ));
                    let facts = signatures.declaration(&owner).unwrap();
                    assert_eq!(facts.constructors.len(), 2);
                    let facts = facts
                        .constructors
                        .iter()
                        .find(|facts| facts.name == "named")
                        .unwrap();
                    assert_eq!(facts.parameters[0].ty, named.positional[0]);
                }
                let child = model.resolve_name("Child").unwrap();
                let signature = signatures.constructor(&child, "new").unwrap();
                assert!(
                    matches!(
                        &signature.positional[0],
                        ResolvedType::Interface { identity, arguments, .. }
                            if identity.is_sdk("dart:core", "List")
                                && arguments[0].interface("dart:core", "String")
                    ),
                    "forwarded parameter: {:?}",
                    signature.positional[0],
                );
                assert!(matches!(
                    &signature.defaults["count"],
                    ParameterDefault::Inherited { owner, .. } if *owner == base
                ));
            },
        );
    }

    #[test]
    fn mixin_application_constness_requires_known_fieldless_mixins() {
        with_model(
            concat!(
                "class Base { const Base(); Base.named(); } ",
                "mixin Stateless {} mixin StaticOnly { static int value = 1; } ",
                "mixin Mutable { int value = 1; } mixin FinalField { final int value = 1; } ",
                "class EmptyAlias = Base with Stateless; ",
                "class StaticAlias = Base with StaticOnly; ",
                "class MutableAlias = Base with Mutable; ",
                "class FinalAlias = Base with FinalField; ",
                "class UnknownAlias = Base with Unresolved; ",
                "class MixedAlias = Base with Stateless, FinalField; ",
                "class Chained = MutableAlias with Stateless;",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (name, expected) in [
                    ("EmptyAlias", true),
                    ("StaticAlias", true),
                    ("MutableAlias", false),
                    ("FinalAlias", false),
                    ("UnknownAlias", false),
                    ("MixedAlias", false),
                    ("Chained", false),
                ] {
                    let owner = model.resolve_name(name).unwrap();
                    assert_eq!(
                        signatures.is_const_constructor(&owner, "new"),
                        expected,
                        "{name}",
                    );
                    assert!(signatures.constructor(&owner, "new").is_some(), "{name}");
                    assert!(!signatures.is_const_constructor(&owner, "named"), "{name}");
                }
            },
        );
    }

    #[test]
    fn generated_mixin_application_constructors_drop_superclass_annotations() {
        with_model(
            concat!(
                "import 'package:meta/meta.dart'; ",
                "class Base { @awaitNotRequired const Base(); } ",
                "mixin Extra {} class Alias = Base with Extra;",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let base = model.resolve_name("Base").unwrap();
                assert_eq!(
                    signatures.declaration(&base).unwrap().constructors[0].annotations,
                    [DeclarationIdentity::package("meta", "awaitNotRequired")],
                );
                let alias = model.resolve_name("Alias").unwrap();
                let facts = &signatures.declaration(&alias).unwrap().constructors[0];
                assert!(facts.annotations.is_empty());
                assert!(facts.is_const);
            },
        );
    }

    #[test]
    fn mixin_application_private_constructors_are_library_scoped() {
        let base_path = PathBuf::from("/project/lib/base.dart");
        let alias_path = PathBuf::from("/project/lib/alias.dart");
        let falcon_dart_parser::ParseResult {
            program: base,
            errors: base_errors,
            ..
        } = parse(concat!(
            "class Base { Base(); Base._private(); } mixin Extra {} ",
            "class LocalAlias = Base with Extra;",
        ));
        let falcon_dart_parser::ParseResult {
            program: alias,
            errors: alias_errors,
            ..
        } = parse("import 'base.dart'; class ImportedAlias = Base with Extra;");
        assert!(base_errors.is_empty());
        assert!(alias_errors.is_empty());
        let sources = [
            IdentitySource {
                path: &alias_path,
                program: &alias,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &base_path,
                program: &base,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files = [(alias_path.clone(), &alias), (base_path.clone(), &base)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        for (path, name, private_visible) in [
            (&base_path, "LocalAlias", true),
            (&alias_path, "ImportedAlias", false),
        ] {
            let model = SemanticModel::new(path, &identities, Some(&types));
            let owner = model.resolve_name(name).unwrap();
            assert!(signatures.constructor(&owner, "new").is_some());
            assert_eq!(
                signatures.constructor(&owner, "_private").is_some(),
                private_visible,
            );
            assert_eq!(
                signatures
                    .declaration(&owner)
                    .unwrap()
                    .constructors
                    .iter()
                    .any(|facts| facts.name == "_private"),
                private_visible,
            );
        }
    }

    #[test]
    fn mixin_application_constructor_facts_support_documented_selectors() {
        with_model(
            "class Base { Base(); Base.named(); } mixin Extra {} class Alias = Base with Extra;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let alias = model.resolve_name("Alias").unwrap();
                for name in ["new", "named"] {
                    assert!(signatures.constructor(&alias, name).is_some());
                    assert!(
                        signatures
                            .declaration(&alias)
                            .unwrap()
                            .constructors
                            .iter()
                            .any(|facts| facts.name == name)
                    );
                }
            },
        );
    }

    #[test]
    fn generic_alias_constructors_infer_alias_parameters_upward() {
        with_model(
            concat!(
                "class Box<T> { final T value; Box(this.value); Box.named(this.value); } class Pair<K, V> { Pair(K key, V value); } ",
                "typedef Alias<T> = Box<T>; typedef Same<T> = Pair<T, T>; typedef Flipped<A, B> = Pair<B, A>; ",
                "Box<int> call() => Alias(1); Box<int> explicitNew() => new Alias(1); Box<String> named() => Alias.named('x'); ",
                "Box<num> explicitArguments() => Alias<num>(1); Pair<num, num> same() => Same(1, 2.0); Pair<int, String> flipped() => Flipped(1, 'x');",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &scope
                        ),
                        expected,
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn downward_generic_candidates_obey_bounds_and_raw_owner_contexts() {
        with_model(
            "T minimum<T extends num>(T first, T second) => first; \
             class Sequence<T extends Object> { Sequence.single(T value); } \
             class Items<T extends Object> extends Sequence<T> { Items.single(T value); } \
             int? bounded(int first, int second) => minimum(first, second); \
             Sequence rawContext() => Sequence.single((Object value) {}); \
             Sequence rawInheritedContext() => Items.single((Object value) {});",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    if !matches!(
                        function.name.name.as_str(),
                        "bounded" | "rawContext" | "rawInheritedContext"
                    ) {
                        continue;
                    }
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let inferred = environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    );
                    if function.name.name == "bounded" {
                        assert_eq!(inferred, expected.non_null());
                    } else {
                        assert!(inferred.arguments()[0].interface("dart:core", "Object"));
                    }
                }
            },
        );
    }

    #[test]
    fn declared_bounds_meet_top_contexts_and_instantiate_unconstrained_tearoffs() {
        with_model(
            concat!(
                "class Route<T> {} class Routes { static Route<T>? of<T extends Object?>(int context) => null; } ",
                "Route<T>? find<T extends Object?>() => null; Route<T>? any<T>() => null; ",
                "class Open<T extends Object?> { Open(); } class Narrow<T extends num> { Narrow(); } class Raw<T> { Raw(); } ",
                "Route<dynamic>? staticCall(int context) => Routes.of(context); ",
                "Route<dynamic>? functionCall() => find(); ",
                "Route<dynamic>? omittedBound() => any(); ",
                "Open<dynamic> openConstructor() => Open(); ",
                "Object Function() boundedTearoff() => Narrow.new; ",
                "Object Function() rawTearoff() => Raw.new; ",
                "typedef Alias<X> = Raw<X>; typedef BoundedAlias<X extends Object?> = Raw<X>; ",
                "Object Function() aliasTearoff() => Alias.new; ",
                "Raw<dynamic> aliasConstructor() => Alias(); ",
                "Raw<dynamic> boundedAliasConstructor() => BoundedAlias(); ",
                "Raw<dynamic> Function() boundedAliasTearoff() => BoundedAlias.new;",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                let dynamic = ResolvedType::Dynamic;
                let object = model.core_type("Object", true);
                let num = model.core_type("num", false);
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    if !function.type_params.is_empty() {
                        continue;
                    }
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let inferred = environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    );
                    let instance = match &inferred {
                        ResolvedType::Function { return_type, .. } => return_type.as_ref(),
                        other => other,
                    };
                    let argument = match function.name.name.as_str() {
                        "omittedBound" | "rawTearoff" | "aliasTearoff" | "aliasConstructor" => {
                            &dynamic
                        }
                        "boundedTearoff" => &num,
                        _ => &object,
                    };
                    assert_eq!(
                        instance.arguments(),
                        std::slice::from_ref(argument),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn raw_interfaces_match_generic_formals_at_their_bounds() {
        with_model(
            concat!(
                "class Box<T> {} class Bounded<T extends num> {} T unbox<T>(Box<T> box) => throw 0; T unbound<T>(Bounded<T> box) => throw 0; ",
                "T first<T>(Iterable<T> values) => throw 0; ",
                "dynamic raw(Box box) => unbox(box); num bounded(Bounded box) => unbound(box); dynamic rawList(List list) => first(list);",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    if !function.type_params.is_empty() {
                        continue;
                    }
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, &signatures, &scope),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn generic_function_values_infer_invocation_binders() {
        with_model(
            "class Stub<T> { T value() => throw 0; } \
             Stub<T> Function<T>(T Function()) get when => throw 0; \
             class Factory { final Stub<T> Function<T>(T Function()) create; Factory(this.create); } \
             double nextDouble() => 0; \
             Stub<double> getterInferred() => when(nextDouble); \
             Stub<int> getterExplicit() => when<int>(() => 1); \
             Stub<double> fieldInferred(Factory factory) => factory.create(nextDouble); \
             Stub<int> fieldExplicit(Factory factory) => factory.create<int>(() => 1); \
             Stub<int> fieldCall(Factory factory) => factory.create.call<int>(() => 1); \
             double chained() => when(nextDouble).value();",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    if function.name.name == "nextDouble" {
                        continue;
                    }
                    let Some(params) = function.params() else {
                        continue;
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(params, model, &scope);
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, &signatures, &scope),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn partial_function_contexts_preserve_tearoffs_and_coalescing_elements() {
        with_model(
            "T identity<T>(T value) => value; \
             R apply<T, R>(T value, R Function(T) callback) => callback(value); \
             class Box<T> { Box(T value); } \
             class Mapper { T convert<T>(T value) => value; } \
             class Chain<E> { Chain(E value); Chain<R> map<R>(R Function(E) callback) => throw 0; } \
             class Holder { final List<int>? items; Holder(this.items); } \
             dynamic functionTearoff() => apply(1, identity); \
             Box<dynamic> constructorTearoff() => apply(1, Box.new); \
             dynamic methodTearoff() => apply(1, Mapper().convert); \
             Chain<int> receiverFunctionTearoff() => Chain(1).map(identity); \
             Chain<int> receiverMethodTearoff() => Chain(1).map(Mapper().convert); \
             List<int> coalescing() => apply(Holder(null), (value) => value.items ?? []); \
             extension Twice on int { get twice => this * 2; } \
             dynamic implicitGetter() => 1.twice;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    if !function.type_params.is_empty() {
                        continue;
                    }
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &scope,
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn inferred_field_formals_refresh_reversed_super_chains() {
        with_model(
            "class Leaf<T> extends Middle<T> { Leaf(super.items); } \
             class Middle<T> extends Box<T> { Middle(super.items); } \
             class Box<T> { var items = <T>[]; Box(this.items); } \
             Box<int> direct() => Box([1]); \
             Middle<int> middle() => Middle([1]); \
             Leaf<int> leaf() => Leaf([1]);",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &scope,
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
                for name in ["Box", "Middle", "Leaf"] {
                    let identity = model.resolve_name(name).unwrap();
                    let signature = signatures.constructor(&identity, "new").unwrap();
                    let facts = signatures.declarations.get(&identity).unwrap();
                    assert_eq!(
                        facts.constructors[0].parameters[0].ty,
                        signature.positional[0]
                    );
                    assert!(!has_inference_holes(&signature.positional[0]));
                }
            },
        );
    }

    #[test]
    fn block_closures_preserve_private_field_promotion_mode() {
        with_model(
            "class Promoted { final int? _x; Promoted(this._x); \
             int Function() closure() => () { if (_x != null) return _x; return 0; }; }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Class(class) = &program.declarations[0] else {
                    panic!("expected class");
                };
                let method = class
                    .members
                    .iter()
                    .find_map(|member| match member {
                        ClassMember::Method(method) => Some(method),
                        _ => None,
                    })
                    .unwrap();
                let Some(FunctionBody::Arrow(expression, _)) = &method.body else {
                    panic!("expected arrow body");
                };
                let receiver = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Promoted").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                let mut environment = TypeEnvironment::with_field_promotions(true);
                environment.declare("this", receiver.clone());
                let lookup = |name: &str| {
                    signatures
                        .resolved_field(&receiver, name, model)
                        .map(|(ty, substitutions)| model.substitute(&ty, &substitutions))
                };
                assert_eq!(
                    environment.infer_with_signatures_using_lookup(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                        &lookup,
                    ),
                    model.resolve_type(method.return_type.as_ref().unwrap()),
                );
            },
        );
    }

    #[test]
    fn block_closures_demote_guarded_locals_after_writes() {
        with_model(
            "int? Function(int?, int?) guarded() => (int? a, int? b) { \
             int? value = a; if (value == null) return 0; value = b; return value; }; \
             int? Function(int?, int?, bool) nested() => (int? a, int? b, bool flag) { \
             int? value = a; if (value == null) return 0; \
             flag ? (value = b) : 0; return value; };",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &TypeParameterScope::default(),
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn block_closures_demote_branch_locals_after_writes() {
        with_parsed_core(
            "int? Function(List<int?>) branch() => (List<int?> values) { \
             int? value = values.first; if (value != null) { \
             value = values.last; return value; } return 0; };",
            "class Object {} class num extends Object {} class int extends num {} \
             class List<E> { E get first => throw 0; E get last => throw 0; }",
            |program, model, signatures| {
                let TopLevelDecl::Function(function) = &program.declarations[0] else {
                    panic!("expected function");
                };
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    panic!("expected arrow body");
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type.as_ref().unwrap()),
                );
            },
        );
    }

    #[test]
    fn block_closures_keep_optional_loop_writes_nullable() {
        with_model(
            "int? Function(bool) loop() => (bool condition) { \
             int? value; while (condition) { value = 1; condition = false; } return value; }; \
             int? Function(bool) collection() => (bool condition) { \
             int? value; [if (condition) value = 1]; return value; };",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &TypeParameterScope::default(),
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn block_closures_keep_optional_switch_writes_nullable() {
        with_model(
            "int? Function(int) switched() => (int key) { int? value; \
             switch (key) { case 0: value = 1; default: break; } return value; }; \
             int? Function(int) expression() => (int key) { int? value; \
             (switch (key) { 0 => value = 1, _ => 0 }); return value; };",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &TypeParameterScope::default(),
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn block_closures_keep_exceptional_writes_nullable() {
        with_model(
            "int? Function() caught() => () { int? value; \
             try { value = parse(); } catch (_) {} return value; }; int parse() => 1;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = &program.declarations[0] else {
                    panic!("expected function");
                };
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    panic!("expected arrow body");
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type.as_ref().unwrap()),
                );
            },
        );
    }

    #[test]
    fn block_closures_preserve_locals_and_guards_inside_optional_paths() {
        with_parsed_core(
            "int Function(List<int>) locals() => (List<int> values) { \
             for (final value in values) { final doubled = value * 2; return doubled; } return 0; }; \
             int Function(List<int?>) guards() => (List<int?> values) { \
             for (final value in values) { if (value == null) continue; return value; } return 0; }; \
             int Function() caught() => () { \
             try { final result = parse(); return result; } catch (_) { return 0; } }; \
             int parse() => 1;",
            "class Object {} class num extends Object {} class int extends num {} \
             class Iterable<E> {} class List<E> extends Iterable<E> {}",
            |program, model, signatures| {
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            signatures,
                            &TypeParameterScope::default(),
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn block_closures_isolate_alternatives_and_loop_back_edges() {
        with_parsed_core(
            "int? Function() caught() => () { int? value = maybe(); \
             try { if (value == null) return 0; invoke(); } \
             catch (_) { return value; } return 1; }; \
             int? Function(int, int?) switched() => (int key, int? value) { \
             switch (key) { case 0: if (value == null) return 0; break; \
             case 1: return value; } return 2; }; \
             int? Function(Object, int?) patterned() => (Object key, int? value) { \
             if (key case int _) { if (value == null) return 0; } \
             else { return value; } return 1; }; \
             int? Function(int?) loop() => (int? value) { \
             if (value == null) return 0; \
             for (var i = 0; i < 3; i++) { \
             if (i == 2) return value; value = maybe(); } return 0; }; \
             int? maybe() => null; void invoke() {}",
            "class Object {} class num extends Object {} class int extends num {}",
            |program, model, signatures| {
                for declaration in program.declarations.iter().take(4) {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!("expected function");
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!("expected arrow body");
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            signatures,
                            &TypeParameterScope::default(),
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn deep_generic_invocation_chains_infer_in_linear_time() {
        const DEPTH: usize = 48;
        let calls = format!("{}1{}", "identity(".repeat(DEPTH), ")".repeat(DEPTH));
        let methods = format!("Box(1){}", ".map((value) => value)".repeat(DEPTH));
        let named = format!("Box(1){}", ".named(f: (value) => value)".repeat(DEPTH));
        let blocks = format!("Box(1){}", ".map((value) { return value; })".repeat(DEPTH));
        let source = format!(
            "T identity<T>(T value) => value; class Box<T> {{ Box(T value); Box<U> map<U>(U Function(T) f) => throw 0; Box<U> named<U>({{required U Function(T) f}}) => throw 0; }} int calls() => {calls}; Box<int> methods() => {methods}; Box<int> named() => {named}; Box<int> blocks() => {blocks};",
        );
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in &program.declarations {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if !function.type_params.is_empty() {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    continue;
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope
                    ),
                    model.resolve_type(function.return_type.as_ref().unwrap()),
                    "{}",
                    function.name.name,
                );
            }
        });
    }

    #[test]
    fn block_closures_bind_lexical_locals_and_join_reachable_ends() {
        with_model(
            concat!(
                "int? Function(bool) partial() => (bool flag) { if (flag) return 1; }; ",
                "int Function(bool) total() => (bool flag) { if (flag) return 1; throw 0; }; ",
                "int Function() looped() => () { while (true) { return 1; } }; ",
                "int Function(List<int>) iterated() => (List<int> values) { for (final value in values) { return value; } throw 0; }; ",
                "int Function() caught() => () { try { return 1; } on int catch (error) { return error; } }; ",
                "int Function((int, String)) destructured() => ((int, String) pair) { final (number, _) = pair; return number; }; ",
                "String Function(Object) cased() => (Object value) { if (value case String text) { return text; } throw 0; }; ",
                "int Function(Object) switched() => (Object value) { switch (value) { case int number: return number; default: throw 0; } }; ",
                "int Function() captured() => () { T identity<T>(T value) => value; final call = () => identity(1); return call(); };",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &scope
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn closure_switches_join_null_only_for_reachable_ends() {
        with_model(
            concat!(
                "enum Choice { first, second } ",
                "class Other { static const first = 0; } ",
                "sealed class Closed {} class Child extends Closed {} ",
                "int Function(Choice) enumeration() => (Choice value) { switch (value) { case Choice.first: return 1; case Choice.second: return 2; } }; ",
                "int Function(bool) boolean() => (bool value) { switch (value) { case true: return 1; case false: return 2; } }; ",
                "int Function(bool?) nullableBoolean() => (bool? value) { switch (value) { case true: return 1; case false: return 2; case null: return 3; } }; ",
                "int Function(Choice?) nullableEnum() => (Choice? value) { switch (value) { case Choice.first: return 1; case Choice.second: return 2; case null: return 3; } }; ",
                "int Function(Object) wildcard() => (Object value) { switch (value) { case Object _: return 1; } }; ",
                "int Function(Object?) untypedWildcard() => (Object? value) { switch (value) { case _: return 1; } }; ",
                "int Function(Object?) wildcardAndNull() => (Object? value) { switch (value) { case Object _: return 1; case null: return 2; } }; ",
                "int Function(Closed) sealedCatchAll() => (Closed value) { switch (value) { case Closed(): return 1; } }; ",
                "int Function(List<int>) genericCatchAll() => (List<int> value) { switch (value) { case List<num> _: return 1; } }; ",
                "int Function(bool) lexical() => (bool value) { final local = value; { switch (local) { case true: return 1; case false: return 2; } } }; ",
                "int? Function(Choice) incompleteEnum() => (Choice value) { switch (value) { case Choice.first: return 1; } }; ",
                "int? Function(bool) incompleteBool() => (bool value) { switch (value) { case true: return 1; } }; ",
                "int? Function(bool?) missingNull() => (bool? value) { switch (value) { case true: return 1; case false: return 2; } }; ",
                "int? Function(bool) guarded() => (bool value) { switch (value) { case true when value: return 1; case false: return 2; } }; ",
                "int? Function(Object?) nullableWildcard() => (Object? value) { switch (value) { case Object _: return 1; } }; ",
                "int? Function(bool) broken() => (bool value) { switch (value) { case true: return 1; case false: break; } }; ",
                "int? Function(Choice) wrongOwner() => (Choice value) { switch (value) { case Other.first: return 1; case Choice.second: return 2; } }; ",
                "int? Function(List<num>) narrowGeneric() => (List<num> value) { switch (value) { case List<int> _: return 1; } }; ",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &scope,
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn interface_joins_take_the_deepest_unique_shared_superinterface() {
        with_model(
            concat!(
                "class Base<T> { T get value => throw 0; } ",
                "class A extends Base<A> {} class B extends Base<B> {} ",
                "class Box<T> {} class C extends Box<Box<C>> {} class D extends Box<Box<D>> {} ",
                "class Shape {} class Square extends Shape {} class Circle extends Shape {} ",
                "class I1 {} class I2 {} class X implements I1, I2 {} class Y implements I1, I2 {} ",
                "class S {} mixin M {} class J0 {} class J extends J0 {} class K extends S with M {} ",
                "class K1 extends K implements J {} class K2 extends K implements J {} ",
                "T pick<T>(T first, T second) { return first; } ",
                "Object generic() => pick(A(), B()); ",
                "Object Function(bool) conditional() => (bool flag) => flag ? A() : B(); ",
                "Object? Function(bool, B?) nullable() => (bool flag, B? other) => flag ? A() : other; ",
                "Object Function(bool) closure() => (bool flag) { if (flag) return A(); return B(); }; ",
                "Object Function(bool) nested() => (bool flag) => flag ? C() : Box<D>(); ",
                "Base<A> Function(bool) lifted() => (bool flag) => flag ? A() : Base<A>(); ",
                "Shape Function(bool) shared() => (bool flag) => flag ? Square() : Circle(); ",
                "Object Function(bool) ambiguous() => (bool flag) => flag ? X() : Y(); ",
                "K Function(bool) mixed() => (bool flag) => flag ? K1() : K2(); ",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        continue;
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            expression,
                            model,
                            &signatures,
                            &scope,
                        ),
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
            },
        );
    }

    #[test]
    fn expression_inference_regression_table() {
        let cases = [
            ("Box.named()", Some("Box")),
            ("box?.field", Some("int?")),
            ("box?.method()", Some("int?")),
            ("box?.fail()", Some("Null")),
            ("callback()", Some("int")),
            ("x.member", Some("dynamic")),
            ("x.member()", Some("dynamic")),
            ("x.member.nested", Some("dynamic")),
            ("x as int?", Some("int?")),
            ("x as int", Some("int")),
            ("x is int", Some("bool")),
            ("x is! int", Some("bool")),
            ("!flag", Some("bool")),
            ("-n", Some("num")),
            ("++n", Some("num")),
            ("--n", Some("num")),
            ("n++", Some("num")),
            ("n--", Some("num")),
            ("i + d", Some("double")),
            ("n + d", Some("double")),
            ("n * d", Some("double")),
            ("n % d", Some("double")),
            ("d + n", Some("double")),
            ("i + n", Some("num")),
            ("i + i", Some("int")),
            ("s + s", Some("String")),
            ("x ?? i", Some("dynamic")),
            ("i / i", Some("double")),
            ("d / n", Some("double")),
            ("n / i", Some("double")),
            ("d ~/ n", Some("int")),
            ("n ~/ i", Some("int")),
            ("i ~/ d", Some("int")),
            ("box / i", Some("Box")),
            ("box ~/ i", None),
            ("x / i", Some("dynamic")),
            ("i / x", Some("double")),
            ("x ~/ i", Some("dynamic")),
            ("i ~/ x", Some("int")),
            ("i + x", Some("num")),
            ("n + x", Some("num")),
            ("d + x", Some("double")),
            ("i & x", Some("int")),
            ("i << x", Some("int")),
            ("missing / i", None),
        ];
        let mut source = String::from(
            "class Box { Box.named(); int field = 0; int method() => 0; Never fail() => throw 0; Box operator /(num s) => this; }",
        );
        for (index, (expression, expected)) in cases.iter().enumerate() {
            source.push_str(&format!(
                "{} f{index}(Box box, dynamic x, int i, double d, num n, String s, bool flag, int Function() callback) => {expression};",
                expected.unwrap_or("dynamic"),
            ));
        }
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for ((expression, expected), declaration) in
                cases.iter().zip(&program.declarations[1..])
            {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                    panic!()
                };
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let expected = expected.map_or(ResolvedType::Unknown, |_| {
                    model.resolve_type(function.return_type.as_ref().unwrap())
                });
                assert_eq!(
                    environment.infer_with_signatures(body, model, &signatures, &scope),
                    expected,
                    "{expression}",
                );
            }
        });
    }

    #[test]
    fn named_extension_static_members_retain_declaration_facts_and_types() {
        with_model(
            "extension Tools on Object { static int value = 0; static int get count => 1; static int read() => value; } int field() => Tools.value; int getter() => Tools.count; int method() => Tools.read();",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let owner = model.resolve_extension("Tools").unwrap();
                for name in ["value", "count", "read"] {
                    let facts = signatures.declared_member_facts(&owner, name);
                    assert_eq!(facts.len(), 1);
                    assert!(facts[0].is_static);
                    assert_eq!(facts[0].declaring_type, owner);
                }
                for declaration in &program.declarations[1..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                        panic!()
                    };
                    assert_eq!(
                        TypeEnvironment::new().infer_with_signatures(
                            body,
                            model,
                            &signatures,
                            &TypeParameterScope::default(),
                        ),
                        model.core_type("int", false),
                    );
                }
            },
        );
    }

    #[test]
    fn never_returning_calls_are_acceptable_in_void_contexts() {
        with_model(
            "Never stop() => throw 0; void use() => stop();",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = &program.declarations[1] else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                    panic!()
                };
                let actual = TypeEnvironment::new().infer_with_signatures(
                    expression,
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                );
                assert_eq!(actual, ResolvedType::Never);
                assert_eq!(
                    model.acceptable_for_void_context(&ResolvedType::Void, &actual),
                    Truth::Yes,
                );
            },
        );
    }

    #[test]
    fn type_literals_and_contextual_tearoffs() {
        with_model(
            concat!(
                "class Box<T> { Box(T value); Box.named(T value); T method(T value) => value; ",
                "U convert<U>(U value) => value; static int staticMethod(int value) => value; ",
                "static Box<int> get empty => Box(0); } ",
                "enum Choice { first, second } T identity<T>(T value) => value; ",
                "Type simple() => int; Type bottom() => Never; Type generic() => Box<int>; ",
                "Symbol symbol() => #member; ",
                "int Function(int) explicitFunction() => identity<int>; ",
                "int Function(int) downwardFunction() => identity; ",
                "Box<int> Function(int) constructor() => Box<int>.new; ",
                "Box<int> Function(int) namedConstructor() => Box<int>.named; ",
                "int Function(int) method(Box<int> box) => box.method; ",
                "int Function(int) genericMethod(Box<int> box) => box.convert<int>; ",
                "int Function(int) staticMethod() => Box.staticMethod; ",
                "Choice choice() => .first; Box<int> getter() => .empty;",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations[3..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(body, _)) = &function.body else {
                        panic!()
                    };
                    let expected = model.resolve_type(function.return_type.as_ref().unwrap());
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let inferred = environment.infer_with_context(
                        body,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    );
                    assert_eq!(inferred, expected, "{body:?}");
                }
            },
        );
    }

    #[test]
    fn core_never_resolves_as_bottom_but_project_never_does_not() {
        with_model(
            "Never stop() => throw 0; Never? nullable() => null;",
            |program, model| {
                for (index, declaration) in program.declarations.iter().enumerate() {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let ty = model.resolve_type(function.return_type.as_ref().unwrap());
                    assert_eq!(
                        ty,
                        if index == 0 {
                            ResolvedType::Never
                        } else {
                            ResolvedType::Null
                        }
                    );
                    assert_eq!(
                        model.acceptable_for_void_context(&ResolvedType::Void, &ty),
                        Truth::Yes
                    );
                }
            },
        );
        with_model(
            "class Never {} Never value() => Never();",
            |program, model| {
                let TopLevelDecl::Function(function) = &program.declarations[1] else {
                    panic!()
                };
                assert!(matches!(
                    model.resolve_type(function.return_type.as_ref().unwrap()),
                    ResolvedType::Interface {
                        identity: DeclarationIdentity::Project { .. },
                        ..
                    }
                ));
            },
        );
    }

    #[test]
    fn declaration_model_preserves_extension_types_and_parameter_owners() {
        with_model(
            "extension type Id(int value) {} Id f<T>() => Id(0);",
            |program, model| {
                let owner = model.resolve_value("f").unwrap();
                let declaration_model = model.for_declaration(&owner).unwrap();
                let TopLevelDecl::Function(function) = &program.declarations[1] else {
                    panic!()
                };
                assert!(matches!(
                    declaration_model.resolve_type(function.return_type.as_ref().unwrap()),
                    ResolvedType::Interface {
                        extension_type: true,
                        ..
                    }
                ));
                let mut original = TypeParameterScope::default();
                original.push(&function.type_params, model);
                let mut declaration = TypeParameterScope::default();
                declaration.push(&function.type_params, &declaration_model);
                assert_eq!(original.lookup("T"), declaration.lookup("T"));
            },
        );
    }

    #[test]
    fn imported_declaration_parameters_use_their_source_file() {
        let inputs = [
            (
                PathBuf::from("/project/lib/main.dart"),
                "import 'origin.dart'; void local<T>() {}",
            ),
            (
                PathBuf::from("/project/lib/origin.dart"),
                "void remote<T>() {}",
            ),
        ];
        let programs: Vec<_> = inputs
            .iter()
            .map(|(_, source)| {
                let falcon_dart_parser::ParseResult {
                    program, errors, ..
                } = parse(source);
                assert!(errors.is_empty(), "{errors:?}");
                program
            })
            .collect();
        let sources: Vec<_> = inputs
            .iter()
            .zip(&programs)
            .map(|((path, _), program)| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let caller = SemanticModel::new(&inputs[0].0, &identities, None);
        let origin = SemanticModel::new(&inputs[1].0, &identities, None);
        let owner = caller.resolve_value("remote").unwrap();
        let imported = caller.for_declaration(&owner).unwrap();
        let TopLevelDecl::Function(function) = &programs[1].declarations[0] else {
            panic!()
        };
        let mut direct_scope = TypeParameterScope::default();
        direct_scope.push(&function.type_params, &origin);
        let mut imported_scope = TypeParameterScope::default();
        imported_scope.push(&function.type_params, &imported);
        let mut caller_scope = TypeParameterScope::default();
        caller_scope.push(&function.type_params, &caller);
        assert_eq!(direct_scope.lookup("T"), imported_scope.lookup("T"));
        assert_ne!(caller_scope.lookup("T"), imported_scope.lookup("T"));
    }

    #[test]
    fn resolves_canonical_generic_and_special_types() {
        with_model(
            "import 'dart:async'; class Box<T> {} void f(Box<String?> box, Future<void> future) {}",
            |program, model| {
                let TopLevelDecl::Function(function) = &program.declarations[1] else {
                    panic!()
                };
                let box_type = model.resolve_type(
                    function.params().unwrap().params[0]
                        .param_type
                        .as_ref()
                        .unwrap(),
                );
                let ResolvedType::Interface {
                    identity,
                    arguments,
                    ..
                } = box_type
                else {
                    panic!()
                };
                assert!(
                    matches!(identity, DeclarationIdentity::Project { name, .. } if name == "Box")
                );
                assert!(arguments[0].nullable());

                let future = model.resolve_type(
                    function.params().unwrap().params[1]
                        .param_type
                        .as_ref()
                        .unwrap(),
                );
                assert!(model.void_context(&future));
            },
        );
    }

    #[test]
    fn unrelatedness_is_conservative_and_numeric_aware() {
        with_model(
            "void f(int i, double d, String s, dynamic x) {}",
            |program, model| {
                let TopLevelDecl::Function(function) = &program.declarations[0] else {
                    panic!()
                };
                let types: Vec<_> = function
                    .params()
                    .unwrap()
                    .params
                    .iter()
                    .map(|param| model.resolve_type(param.param_type.as_ref().unwrap()))
                    .collect();
                assert_eq!(model.unrelated(&types[0], &types[1]), Truth::No);
                assert_eq!(model.unrelated(&types[0], &types[2]), Truth::Yes);
                assert_eq!(model.unrelated(&types[0], &types[3]), Truth::Unknown);
            },
        );
    }

    #[test]
    fn type_parameter_bounds_can_reference_the_complete_parameter_list() {
        with_model(
            "void compare<T extends Comparable<U>, U extends T>(T left, U right) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let signature = signatures
                    .function(&model.resolve_value("compare").unwrap())
                    .unwrap();
                let ResolvedType::TypeParameter { bound, .. } = &signature.positional[0] else {
                    panic!()
                };
                let ResolvedType::Interface { arguments, .. } = bound.as_ref() else {
                    panic!()
                };
                assert!(
                    matches!(arguments.first(), Some(ResolvedType::TypeParameter { id, .. }) if id.name.as_ref() == "U")
                );
            },
        );
    }

    #[test]
    fn signature_shapes_preserve_required_names_and_unused_declared_binders() {
        with_model(
            "void accept<T extends num, U extends T>({required int z, int a = 0}) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let signature = signatures
                    .function(&model.resolve_value("accept").unwrap())
                    .unwrap();
                let shape = signature.shape.as_ref().unwrap();
                assert_eq!(shape.required_named.as_ref(), &["z".to_string()]);
                assert_eq!(shape.type_parameters.len(), 2);
                let ResolvedType::TypeParameter { id, bound, .. } = &shape.type_parameters[0]
                else {
                    panic!("missing declared binder");
                };
                assert_eq!(bound.as_ref(), &model.core_type("num", false));
                assert!(matches!(
                    &shape.type_parameters[1],
                    ResolvedType::TypeParameter { bound, .. }
                        if matches!(bound.as_ref(), ResolvedType::TypeParameter { id: dependency, .. } if dependency == id)
                ));
                let ResolvedType::Function {
                    shape: Some(forwarded),
                    ..
                } = signature.function_type()
                else {
                    panic!("missing function shape");
                };
                assert!(Arc::ptr_eq(shape, &forwarded));
                let mut local = signature.clone();
                local.call_parameters.clear();
                local.bind_local_type_parameters(&TypeParameterScope::default());
                assert_eq!(local.call_parameters, signature.call_parameters);
                let substitutions = HashMap::from([(id.clone(), model.core_type("int", false))]);
                let instantiated = signature.substituted_shape(model, &substitutions).unwrap();
                assert_eq!(instantiated.required_named, shape.required_named);
                assert!(matches!(
                    instantiated.type_parameters.as_ref(),
                    [ResolvedType::TypeParameter { bound, .. }]
                        if bound.as_ref() == &model.core_type("int", false)
                ));
            },
        );
    }

    #[test]
    fn declaration_contracts_preserve_names_kinds_defaults_and_optional_arity() {
        with_model(
            "class A { void m(covariant int first, [int second]) {} \
             void n({required int named, int optional = 1 + 1}) {} \
             int get value => 0; set value(covariant int incoming) {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let identity = model.resolve_name("A").unwrap();
                let method = signatures.member(&identity, "m").unwrap();
                assert_eq!(method.optional_positional, 1);
                assert!(matches!(
                    method.function_type(),
                    ResolvedType::Function {
                        optional_positional: 1,
                        ..
                    }
                ));
                let facts = &signatures.declared_member_facts(&identity, "m")[0];
                let contracts = facts.parameter_contracts.as_ref().unwrap();
                assert_eq!(contracts[0].name, "first");
                assert_eq!(contracts[0].kind, ParamKind::Positional);
                assert!(contracts[0].is_covariant);
                assert_eq!(contracts[1].name, "second");
                assert_eq!(contracts[1].kind, ParamKind::OptionalPositional);
                assert!(contracts[1].default_value_code.is_none());
                let cloned_facts = facts.clone();
                assert!(Arc::ptr_eq(
                    contracts,
                    cloned_facts.parameter_contracts.as_ref().unwrap(),
                ));
                let contracts = signatures.declared_member_facts(&identity, "n")[0]
                    .parameter_contracts
                    .as_ref()
                    .unwrap();
                assert_eq!(contracts[0].name, "named");
                assert_eq!(contracts[0].kind, ParamKind::Named { required: true });
                assert_eq!(contracts[1].name, "optional");
                assert!(contracts[1].default_value_code.is_some());
                let accessors = &signatures.member_entry(&identity, "value").unwrap().facts;
                let getter = accessors
                    .iter()
                    .find(|facts| facts.kind == MemberKind::Getter)
                    .unwrap();
                assert!(getter.parameter_contracts.as_ref().unwrap().is_empty());
                let setter = accessors
                    .iter()
                    .find(|facts| facts.kind == MemberKind::Setter)
                    .unwrap();
                let contract = &setter.parameter_contracts.as_ref().unwrap()[0];
                assert_eq!(contract.name, "incoming");
                assert!(contract.is_covariant);
            },
        );
    }

    #[test]
    fn signatures_preserve_type_parameters_for_void_substitution() {
        with_model("void accept<T>(T value) {}", |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let signature = signatures
                .function(&model.resolve_value("accept").unwrap())
                .unwrap();
            assert!(contains_parameter(&signature.positional[0]));
            let substitutions =
                HashMap::from([(signature.call_parameters[0].clone(), ResolvedType::Void)]);
            let expected = model.substitute(&signature.positional[0], &substitutions);
            assert_eq!(expected, ResolvedType::Void);
            assert_eq!(
                model.acceptable_for_void_context(&expected, &model.core_type("int", false)),
                Truth::No
            );
            assert_eq!(
                model.acceptable_for_void_context(&expected, &ResolvedType::Null),
                Truth::Yes
            );
        });
    }

    #[test]
    fn field_formal_signatures_include_fields_declared_after_constructors() {
        with_model(
            "class Value<T> { Value(this.value); final T value; T read() => value; }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let identity = model.resolve_name("Value").unwrap();
                let constructor = signatures.constructor(&identity, "new").unwrap();
                let (parameters, field) = signatures.field(&identity, "value").unwrap();
                assert_eq!(&constructor.owner_parameters, parameters);
                assert_eq!(&constructor.positional[0], field);
                assert_eq!(
                    &signatures.member(&identity, "read").unwrap().return_type,
                    field
                );
                let facts = signatures.declaration(&identity).unwrap();
                assert_eq!(&facts.constructors[0].parameters[0].ty, field);
            },
        );
    }

    #[test]
    fn synthetic_enum_values_have_static_list_types_with_parsed_core() {
        let source = "enum Choice { first, second } \
             enum Generic<T> { integer<int>(1), text<String>('a'); const Generic(this.value); final T value; } \
             enum Bounded<T extends num> { integer<int>(1), decimal<double>(1.5); const Bounded(this.value); final T value; } \
             List<Choice> choices() => Choice.values; \
             Choice selected(int? index) => Choice.values[index!]; \
             int position(Choice previous) => Choice.values.indexOf(previous); \
             List<Generic<dynamic>> generic() => Generic.values; \
             List<Bounded<num>> bounded() => Bounded.values;";
        with_parsed_core(
            source,
            "library dart.core; class Object {} abstract class num {} \
             abstract class int extends num {} abstract class double extends num {} \
             abstract class String {} abstract class Enum {} \
             abstract class List<E> { E operator [](int index); int indexOf(E value); }",
            |program, model, signatures| {
                let scope = TypeParameterScope::default();
                for declaration in &program.declarations {
                    let TopLevelDecl::Function(function) = declaration else {
                        continue;
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    let inferred =
                        environment.infer_with_signatures(expression, model, signatures, &scope);
                    assert_eq!(
                        inferred,
                        model.resolve_type(function.return_type.as_ref().unwrap()),
                        "{}",
                        function.name.name,
                    );
                }
                let identity = model.resolve_name("Choice").unwrap();
                let facts = signatures.declared_member_facts(&identity, "values");
                assert!(facts.iter().any(|fact| {
                    fact.is_static
                        && matches!(
                            fact.kind,
                            MemberKind::Field {
                                writable: false,
                                ..
                            }
                        )
                }));
                let receiver = ResolvedType::interface_identity(identity);
                assert!(
                    signatures
                        .resolved_field(&receiver, "values", model)
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn inference_lookup_fallback_reaches_recursive_expressions_and_respects_bindings() {
        with_model(
            "Object direct() => value; Object asserted() => value!; Object binary() => value + value; Object conditional() => true ? value : value; Object method() => value.toUpperCase(); Object closure() => () => value;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                let environment = TypeEnvironment::new();
                let expected = model.core_type("String", false);
                let lookup = |name: &str| (name == "value").then(|| expected.clone());
                for (index, declaration) in program.declarations.iter().enumerate() {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = function.body.as_ref() else {
                        panic!()
                    };
                    let inferred = environment.infer_with_signatures_using_lookup(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        &lookup,
                    );
                    if index == 5 {
                        let ResolvedType::Function { return_type, .. } = inferred else {
                            panic!()
                        };
                        assert_eq!(*return_type, expected);
                    } else {
                        assert_eq!(inferred, expected);
                    }
                    if index == 0 {
                        let mut bound = environment.clone();
                        bound.declare("value", model.core_type("int", false));
                        assert_eq!(
                            bound.infer_with_signatures_using_lookup(
                                expression,
                                model,
                                &signatures,
                                &scope,
                                &|_| Some(ResolvedType::Unknown),
                            ),
                            model.core_type("int", false),
                        );
                        assert!(
                            bound
                                .infer_with_signatures_using_lookup(
                                    expression,
                                    model,
                                    &signatures,
                                    &scope,
                                    &|_| None,
                                )
                                .interface("dart:core", "int")
                        );
                    }
                }
            },
        );
    }

    #[test]
    fn cast_inference_preserves_written_nullability_and_type_parameter_scope() {
        with_model(
            "int? nullable(Object? value) => value as int?; int nonNullable(Object value) => value as int; T? generic<T>(Object? value) => value as T?;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (index, declaration) in program.declarations.iter().enumerate() {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = function.body.as_ref() else {
                        panic!()
                    };
                    let mut scope = TypeParameterScope::default();
                    scope.push(&function.type_params, model);
                    let expected =
                        model.resolve_type_in(function.return_type.as_ref().unwrap(), &scope);
                    let environment = TypeEnvironment::new();
                    assert_eq!(
                        environment.infer_with_signatures_using_lookup(
                            expression,
                            model,
                            &signatures,
                            &scope,
                            &|_| None
                        ),
                        expected
                    );
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, &signatures, &scope),
                        expected,
                    );
                    assert_eq!(expected.nullable(), index != 1);
                    if index == 2 {
                        assert!(matches!(expected, ResolvedType::TypeParameter { .. }));
                    }
                }
            },
        );
    }

    #[test]
    fn inherited_method_calls_use_instantiated_return_types() {
        with_model(
            "class Base { int value() => 0; } class Child extends Base {} int inherited(Child child) => child.value(); class GenericBase<T> { T value() => throw UnimplementedError(); } class GenericChild extends GenericBase<int> {} int generic(GenericChild child) => child.value();",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in [&program.declarations[2], &program.declarations[5]] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = function.body.as_ref() else {
                        panic!()
                    };
                    let type_parameters = TypeParameterScope::default();
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &type_parameters);

                    let inferred = environment.infer_with_signatures(
                        expression.as_ref(),
                        model,
                        &signatures,
                        &type_parameters,
                    );
                    assert!(inferred.interface("dart:core", "int"));
                }
            },
        );
    }

    #[test]
    fn same_named_type_parameters_of_different_declarations_stay_distinct() {
        with_model(
            "class B<T> { T get x => throw 0; } class A<T> extends B<int> {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let a = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("A").unwrap(),
                    arguments: vec![model.core_type("String", false)].into(),
                    nullable: false,
                    extension_type: false,
                };
                let (field, substitutions) = signatures.resolved_field(&a, "x", model).unwrap();
                assert!(
                    model
                        .substitute(&field, &substitutions)
                        .interface("dart:core", "int")
                );
            },
        );
    }

    #[test]
    fn member_lookup_prefers_later_mixins_and_includes_interface_contracts() {
        with_model(
            "class Base { num value() => 0; num field = 0; } mixin First<T> { T value() => throw UnimplementedError(); T field; } mixin Second<T> { T value() => throw UnimplementedError(); T field; } abstract class Contract { bool value(); bool field; int only(); } class Child extends Base with First<String>, Second<int> implements Contract {} class InterfaceOnly implements Contract {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child_identity = model.resolve_name("Child").unwrap();
                let child = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: child_identity,
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                let (signature, substitutions) =
                    signatures.resolved_member(&child, "value", model).unwrap();
                assert!(
                    model
                        .substitute(&signature.return_type, &substitutions)
                        .interface("dart:core", "int")
                );
                let (field, substitutions) =
                    signatures.resolved_field(&child, "field", model).unwrap();
                assert!(
                    model
                        .substitute(&field, &substitutions)
                        .interface("dart:core", "int")
                );
                assert_eq!(
                    signatures.resolved_member_owner(&child, "value", model),
                    model.resolve_name("Second")
                );

                let contract = model.resolve_name("Contract").unwrap();
                let interface_only = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("InterfaceOnly").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                assert_eq!(
                    signatures.is_subtype_of(&interface_only, &contract, model),
                    Truth::Yes
                );
                let (signature, substitutions) = signatures
                    .resolved_member(&interface_only, "only", model)
                    .unwrap();
                assert!(
                    model
                        .substitute(&signature.return_type, &substitutions)
                        .interface("dart:core", "int")
                );
                assert_eq!(
                    signatures.resolved_member_owner(&interface_only, "only", model),
                    Some(contract)
                );
            },
        );
    }

    #[test]
    fn static_mixin_members_do_not_shadow_inherited_instance_members() {
        with_model(
            "class Base { int method() => 0; int field = 0; int get getter => 0; set setter(int value) {} } mixin StaticMembers { static String method() => ''; static bool field = false; static double get getter => 0; static set setter(num value) {} } class Child extends Base with StaticMembers {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                let base = model.resolve_name("Base").unwrap();
                let mixin = model.resolve_name("StaticMembers").unwrap();

                let (signature, substitutions) =
                    signatures.resolved_member(&child, "method", model).unwrap();
                assert!(
                    model
                        .substitute(&signature.return_type, &substitutions)
                        .interface("dart:core", "int")
                );
                for name in ["method", "field", "getter"] {
                    assert_eq!(
                        signatures.resolved_member_owner(&child, name, model),
                        Some(base.clone())
                    );
                }

                for name in ["field", "getter"] {
                    let (field, substitutions) =
                        signatures.resolved_field(&child, name, model).unwrap();
                    assert!(
                        model
                            .substitute(&field, &substitutions)
                            .interface("dart:core", "int")
                    );
                }
                let (setter, substitutions) = signatures
                    .resolved_writable_field(&child, "setter", model)
                    .unwrap();
                assert!(
                    model
                        .substitute(&setter, &substitutions)
                        .interface("dart:core", "int")
                );

                assert!(
                    signatures
                        .member(&mixin, "method")
                        .unwrap()
                        .return_type
                        .interface("dart:core", "String")
                );
                for (name, expected) in [("field", "bool"), ("getter", "double"), ("setter", "num")]
                {
                    assert!(
                        signatures
                            .field(&mixin, name)
                            .unwrap()
                            .1
                            .interface("dart:core", expected)
                    );
                }
            },
        );
    }

    #[test]
    fn legacy_function_typed_setter_has_a_nullable_function_write_type() {
        with_model(
            "class Value { set callback(void value()?) {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let receiver = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Value").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                let (write_type, substitutions) = signatures
                    .resolved_writable_field(&receiver, "callback", model)
                    .unwrap();
                let write_type = model.substitute(&write_type, &substitutions);
                let ResolvedType::Function {
                    return_type,
                    positional,
                    named,
                    nullable,
                    ..
                } = write_type
                else {
                    panic!("setter parameter must remain a function type");
                };
                assert_eq!(*return_type, ResolvedType::Void);
                assert!(positional.is_empty());
                assert!(named.is_empty());
                assert!(nullable);
            },
        );
    }

    #[test]
    fn setter_does_not_overwrite_getter_return_type_index() {
        for source in [
            "class Value { String get value => ''; set value(num next) {} }",
            "class Value { set value(num next) {} String get value => ''; }",
        ] {
            with_model(source, |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let identity = model.resolve_name("Value").unwrap();
                assert!(
                    signatures
                        .field(&identity, "value")
                        .unwrap()
                        .1
                        .interface("dart:core", "String")
                );
            });
        }
    }

    #[test]
    fn only_uninitialized_late_final_fields_have_writable_owners() {
        with_model(
            "class Value {} class Base { late final Value pending, initialized = Value(); final Value ordinary = Value(); } class Child extends Base {} abstract class View implements Base {} mixin Init on Base {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let base = model.resolve_name("Base").unwrap();
                for owner in ["Child", "View", "Init"] {
                    let child = ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name(owner).unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    };
                    assert_eq!(
                        signatures.resolved_writable_member_owner(&child, "pending", model),
                        Some(base.clone())
                    );
                    let (ty, substitutions) = signatures
                        .resolved_writable_field(&child, "pending", model)
                        .unwrap();
                    let ResolvedType::Interface { identity, .. } =
                        model.substitute(&ty, &substitutions)
                    else {
                        panic!("late-final setter should retain its declared field type");
                    };
                    assert_eq!(Some(identity), model.resolve_name("Value"));
                    for name in ["initialized", "ordinary"] {
                        assert_eq!(
                            signatures.resolved_readable_member_owner(&child, name, model),
                            Some(base.clone())
                        );
                        assert!(
                            signatures
                                .resolved_writable_member_owner(&child, name, model)
                                .is_none()
                        );
                        assert!(
                            signatures
                                .resolved_writable_field(&child, name, model)
                                .is_none()
                        );
                    }
                }
            },
        );
    }

    #[test]
    fn child_setter_does_not_hide_inherited_generic_getter() {
        with_model(
            "class Base<T> { T get value => throw UnimplementedError(); } class Child extends Base<String> { set value(num next) {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };

                let (read_type, read_substitutions) =
                    signatures.resolved_field(&child, "value", model).unwrap();
                assert!(contains_parameter(&read_type));
                assert!(
                    model
                        .substitute(&read_type, &read_substitutions)
                        .interface("dart:core", "String")
                );

                let (write_type, write_substitutions) = signatures
                    .resolved_writable_field(&child, "value", model)
                    .unwrap();
                assert!(
                    model
                        .substitute(&write_type, &write_substitutions)
                        .interface("dart:core", "num")
                );
            },
        );
    }

    #[test]
    fn named_constructor_and_instance_method_signatures_do_not_collide() {
        with_model(
            "class Base { Base.named(String input); int named(bool input) => 0; } class Child extends Base { Child(super.input) : super.named(); }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let base = model.resolve_name("Base").unwrap();
                let receiver = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: base.clone(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };

                let constructor = signatures.constructor(&base, "named").unwrap();
                assert!(constructor.positional[0].interface("dart:core", "String"));

                let (method, substitutions) = signatures
                    .resolved_member(&receiver, "named", model)
                    .unwrap();
                assert!(method.positional[0].interface("dart:core", "bool"));
                assert!(
                    model
                        .substitute(&method.return_type, &substitutions)
                        .interface("dart:core", "int")
                );

                let child = model.resolve_name("Child").unwrap();
                assert!(
                    signatures.constructor(&child, "new").unwrap().positional[0]
                        .interface("dart:core", "String")
                );
            },
        );
    }

    #[test]
    fn mixin_constraints_do_not_override_applied_generic_superclass_fields() {
        with_model(
            "class Widget {} class State<T> { T get widget => throw 0; } mixin Constraint on State {} class Concrete extends State<Widget> with Constraint {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let receiver =
                    ResolvedType::interface_identity(model.resolve_name("Concrete").unwrap());
                let (field, substitutions) = signatures
                    .resolved_field(&receiver, "widget", model)
                    .unwrap();
                assert_eq!(
                    model.substitute(&field, &substitutions),
                    ResolvedType::interface_identity(model.resolve_name("Widget").unwrap()),
                );
                let constraint = model.resolve_name("Constraint").unwrap();
                assert_eq!(
                    signatures.member_supertypes[&constraint],
                    vec![ResolvedType::interface_identity(
                        DeclarationIdentity::OBJECT
                    )],
                );
            },
        );
    }

    #[test]
    fn later_mixin_getter_overrides_base_field_with_owner_substitution() {
        with_model(
            "class Base { num value = 0; } mixin ReadValue<T> { T get value => throw UnimplementedError(); } class Child extends Base with ReadValue<int> {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };

                let (field, substitutions) =
                    signatures.resolved_field(&child, "value", model).unwrap();
                assert!(contains_parameter(&field));
                assert!(
                    model
                        .substitute(&field, &substitutions)
                        .interface("dart:core", "int")
                );
            },
        );
    }

    #[test]
    fn later_mixin_getter_shadows_base_method_for_all_instance_lookup() {
        with_model(
            "class Base { num value() => 0; } mixin ReadValue<T> { T get value => throw UnimplementedError(); } class Child extends Base with ReadValue<String> {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };

                assert!(signatures.resolved_member(&child, "value", model).is_none());
                let (read, substitutions) =
                    signatures.resolved_field(&child, "value", model).unwrap();
                assert!(contains_parameter(&read));
                assert!(
                    model
                        .substitute(&read, &substitutions)
                        .interface("dart:core", "String")
                );
            },
        );
    }

    #[test]
    fn later_mixin_method_shadows_base_read_for_all_instance_lookup() {
        with_model(
            "class Base { num get value => 0; } mixin ComputeValue<T> { T value() => throw UnimplementedError(); } class Child extends Base with ComputeValue<int> {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };

                assert!(signatures.resolved_field(&child, "value", model).is_none());
                let (signature, substitutions) =
                    signatures.resolved_member(&child, "value", model).unwrap();
                assert!(contains_parameter(&signature.return_type));
                assert!(
                    model
                        .substitute(&signature.return_type, &substitutions)
                        .interface("dart:core", "int")
                );
            },
        );
    }

    #[test]
    fn implemented_member_combination_is_order_independent_and_instantiated() {
        with_model(
            "class Result {} class Narrow extends Result {} \
             abstract class A<T> { T get value; T method(); set output(Narrow value); } \
             abstract class B<T> { T get value; T method(); set output(Result value); } \
             abstract class Forward implements A<Result>, B<Narrow> {} \
             abstract class Reverse implements B<Narrow>, A<Result> {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let narrow =
                    ResolvedType::interface_identity(model.resolve_name("Narrow").unwrap());
                let wide = ResolvedType::interface_identity(model.resolve_name("Result").unwrap());
                for name in ["Forward", "Reverse"] {
                    let receiver =
                        ResolvedType::interface_identity(model.resolve_name(name).unwrap());
                    let (field, substitutions) = signatures
                        .resolved_field(&receiver, "value", model)
                        .unwrap();
                    assert_eq!(model.substitute(&field, &substitutions), narrow);
                    let (method, substitutions) = signatures
                        .resolved_member(&receiver, "method", model)
                        .unwrap();
                    assert_eq!(
                        model.substitute(&method.return_type, &substitutions),
                        narrow
                    );
                    let (field, substitutions) = signatures
                        .resolved_writable_field(&receiver, "output", model)
                        .unwrap();
                    assert_eq!(model.substitute(&field, &substitutions), wide);
                }
            },
        );
    }

    #[test]
    fn incompatible_implemented_members_remain_unknown() {
        with_model(
            "class Left {} class Right {} \
             abstract class A { Left get value; Left method(); } \
             abstract class B { Right get value; Right method(); } \
             abstract class Forward implements A, B {} \
             abstract class Reverse implements B, A {} \
             extension Fallback on Object { Left get value => Left(); Left method() => Left(); }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for name in ["Forward", "Reverse"] {
                    let receiver =
                        ResolvedType::interface_identity(model.resolve_name(name).unwrap());
                    assert!(
                        signatures
                            .resolved_field(&receiver, "value", model)
                            .is_none()
                    );
                    assert!(
                        signatures
                            .resolved_member(&receiver, "method", model)
                            .is_none()
                    );
                }
            },
        );
    }

    #[test]
    fn negative_supertype_queries_visit_each_diamond_declaration_once() {
        let mut source = String::from("class Root<T> {} class Missing {} ");
        for level in 0..24 {
            let base = if level == 0 {
                "Root".to_owned()
            } else {
                format!("Join{}", level - 1)
            };
            source.push_str(&format!(
                "abstract class Left{level}<T> implements {base}<T> {{}} \
                 abstract class Right{level}<T> implements {base}<T> {{}} \
                 abstract class Join{level}<T> implements Left{level}<T>, Right{level}<T> {{}} "
            ));
        }
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: model.resolve_name("Join23").unwrap(),
                arguments: vec![model.core_type("int", false)].into(),
                nullable: false,
                extension_type: false,
            };
            let visits = std::cell::Cell::new(0);
            let mut visited = FxHashSet::default();
            assert!(
                signatures
                    .instantiated_matching(
                        &receiver,
                        &|_| {
                            visits.set(visits.get() + 1);
                            false
                        },
                        model,
                        &mut visited,
                    )
                    .is_none()
            );
            assert_eq!(visits.get(), visited.len());
            assert_eq!(visited.len(), 74); // 24 diamonds, Root and Object.
            assert!(
                signatures
                    .instantiated_identity(
                        &receiver,
                        &model.resolve_name("Missing").unwrap(),
                        model,
                    )
                    .is_none()
            );
            assert!(
                signatures
                    .instantiated_supertype(&receiver, "dart:async", "Future", model)
                    .is_none()
            );
            let root = signatures
                .instantiated_identity(&receiver, &model.resolve_name("Root").unwrap(), model)
                .unwrap();
            assert_eq!(root.arguments(), &[model.core_type("int", false)]);
        });
    }

    #[test]
    fn resolved_calls_borrow_signatures_and_share_default_expressions() {
        with_model(
            "class Value { Value({int n = 1}); void method({int n = 1}) {} } \
             class Receiver {} extension Methods on Receiver { void method({int n = 1}) {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let owner = model.resolve_name("Value").unwrap();
                let receiver = ResolvedType::interface_identity(owner.clone());
                let (constructor, _) = signatures.resolved_constructor(&receiver, "new").unwrap();
                assert!(std::ptr::eq(
                    constructor,
                    signatures.constructor(&owner, "new").unwrap()
                ));
                let (method, _) = signatures
                    .resolved_member(&receiver, "method", model)
                    .unwrap();
                assert!(std::ptr::eq(
                    method,
                    signatures.instance_member(&owner, "method").unwrap()
                ));
                let cloned = (*method).clone();
                let ParameterDefault::Declared(default) = &method.defaults["n"] else {
                    panic!()
                };
                let ParameterDefault::Declared(copy) = &cloned.defaults["n"] else {
                    panic!()
                };
                assert!(Arc::ptr_eq(default, copy));
                let receiver =
                    ResolvedType::interface_identity(model.resolve_name("Receiver").unwrap());
                let (resolved, _) = signatures
                    .resolved_member(&receiver, "method", model)
                    .unwrap();
                let (_, _, declared) = signatures
                    .extension_method(&receiver, "method", model)
                    .unwrap();
                assert!(std::ptr::eq(resolved, declared));
            },
        );
    }

    #[test]
    fn object_fallback_does_not_beat_earlier_application_chain_members() {
        with_model(
            "class Base { String toString() => ''; } mixin First { String toString() => ''; } mixin Last {} class FromBase extends Base with Last {} class FromMixin extends Base with First, Last {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (class_name, owner_name) in [("FromBase", "Base"), ("FromMixin", "First")] {
                    let receiver = ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name(class_name).unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    };
                    assert_eq!(
                        signatures.resolved_member_owner(&receiver, "toString", model),
                        model.resolve_name(owner_name)
                    );
                }
            },
        );
    }

    #[test]
    fn super_owner_checks_mixin_constraints_before_object_and_superclasses() {
        with_model(
            "class Base { String toString() => ''; void member() {} } mixin Applied on Base { void member() {} } class Concrete extends Base with Applied {} mixin Constrained on Concrete {} class Child extends Base with Applied { void member() {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (receiver, member, owner) in [
                    ("Constrained", "toString", "Base"),
                    ("Constrained", "member", "Applied"),
                    ("Child", "toString", "Base"),
                    ("Child", "member", "Applied"),
                ] {
                    let receiver = ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name(receiver).unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    };
                    assert_eq!(
                        signatures.resolved_super_member_owner(&receiver, member, false, model),
                        model.resolve_name(owner)
                    );
                }
            },
        );
    }

    #[test]
    fn extension_owner_checks_concrete_arguments_and_preserves_unknowns() {
        with_model(
            "class IntElement {} class StringElement {} class SourceList<T> {} class IntList extends SourceList<IntElement> {} extension Strings on SourceList<StringElement> { void member() {} } extension General on Object { void member() {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (element, owner) in [("IntElement", "General"), ("StringElement", "Strings")] {
                    let receiver = ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name("SourceList").unwrap(),
                        arguments: vec![ResolvedType::Interface {
                            occurrence: Default::default(),
                            identity: model.resolve_name(element).unwrap(),
                            arguments: Vec::new().into(),
                            nullable: false,
                            extension_type: false,
                        }]
                        .into(),
                        nullable: false,
                        extension_type: false,
                    };
                    assert_eq!(
                        signatures.resolved_extension_owner(&receiver, "member", model),
                        model.resolve_extension(owner)
                    );
                }
                let receiver = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("IntList").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                assert_eq!(
                    signatures.resolved_extension_owner(&receiver, "member", model),
                    model.resolve_extension("General")
                );
                let unknown = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("SourceList").unwrap(),
                    arguments: vec![ResolvedType::Unknown].into(),
                    nullable: false,
                    extension_type: false,
                };
                assert_eq!(
                    signatures.resolved_extension_owner(&unknown, "member", model),
                    None
                );
            },
        );
    }

    #[test]
    fn extension_owner_instantiates_raw_generic_and_bounded_on_types() {
        with_model(
            concat!(
                "class Element {} class NumberElement extends Element {} class TextElement extends Element {} ",
                "class SourceIterable<T> {} class SourceList<T> extends SourceIterable<T> {} ",
                "class SourceMap<K, V> {} class NestedList extends SourceList<SourceList<NumberElement>> {} ",
                "extension RawList on SourceList { void raw() {} } ",
                "extension RawIterable on SourceIterable { void raw() {} } ",
                "extension GenericList<T> on SourceList<T> { void generic() {} void concrete() {} T element() => throw 0; } ",
                "extension GenericIterable<U> on SourceIterable<U> { void generic() {} } ",
                "extension ConcreteIterable on SourceIterable<NumberElement> { void concrete() {} } ",
                "extension TextMap<K> on SourceMap<K, TextElement> { void mapping() {} } ",
                "extension BoundedList<T extends NumberElement> on SourceList<T> { void bounded() {} } ",
                "extension Nested<T extends NumberElement> on SourceList<SourceList<T>> { T nested() => throw 0; } ",
                "extension General on Object { void mapping() {} void bounded() {} } ",
                "void uses(SourceList<NumberElement> numbers, SourceList<TextElement> texts, ",
                "SourceList raw, SourceMap<NumberElement, NumberElement> numberMap, ",
                "SourceMap<NumberElement, TextElement> textMap, NestedList nested) {}",
            ),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = program.declarations.last().unwrap() else {
                    panic!()
                };
                let receivers: Vec<_> = function
                    .params()
                    .unwrap()
                    .params
                    .iter()
                    .map(|parameter| model.resolve_type(parameter.param_type.as_ref().unwrap()))
                    .collect();
                for (index, member, owner) in [
                    (0, "raw", "RawList"),
                    (2, "raw", "RawList"),
                    (0, "generic", "GenericList"),
                    (0, "concrete", "GenericList"),
                    (0, "bounded", "BoundedList"),
                    (1, "bounded", "General"),
                    (3, "mapping", "General"),
                    (4, "mapping", "TextMap"),
                    (5, "nested", "Nested"),
                ] {
                    assert_eq!(
                        signatures.resolved_extension_owner(&receivers[index], member, model),
                        model.resolve_extension(owner),
                        "{index}: {member}",
                    );
                }
                for (index, method) in [(0, "element"), (5, "nested")] {
                    let (signature, substitutions) = signatures
                        .resolved_member(&receivers[index], method, model)
                        .unwrap();
                    assert_eq!(
                        model.substitute(&signature.return_type, &substitutions),
                        ResolvedType::interface_identity(
                            model.resolve_name("NumberElement").unwrap()
                        ),
                    );
                }
            },
        );
    }

    #[test]
    fn generic_extension_getters_apply_to_void_type_arguments() {
        with_model(
            "class SourceFuture<T> {} extension Completion<T> on SourceFuture<T> { void get completed {} } void uses(SourceFuture<void> future) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = program.declarations.last().unwrap() else {
                    panic!()
                };
                let receiver = model.resolve_type(
                    function.params().unwrap().params[0]
                        .param_type
                        .as_ref()
                        .unwrap(),
                );
                assert!(signatures.has_possible_extension_getter(&receiver, "completed", model));
                assert_eq!(
                    signatures.resolved_extension_owner(&receiver, "completed", model),
                    model.resolve_extension("Completion"),
                );
            },
        );
    }

    #[test]
    fn extension_specificity_breaks_instantiated_ties_at_parameter_bounds() {
        with_model(
            "class Element {} class SourceList<T> {} extension Exact on SourceList<Element> { void choose() {} } extension Generic<T> on SourceList<T> { void choose() {} } void uses(SourceList<Element> values) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let receiver = &signatures
                    .function(&model.resolve_value("uses").unwrap())
                    .unwrap()
                    .positional[0];
                assert_eq!(
                    signatures.resolved_extension_owner(receiver, "choose", model),
                    model.resolve_extension("Exact"),
                );
            },
        );
    }

    #[test]
    fn extension_inference_looks_through_receiver_bounds_and_retains_missing_arguments() {
        with_model(
            "class Element {} class SourceList<T> {} class Holder {} extension Pick<T> on SourceList<T> { T pick() => throw 0; } extension Unused<T extends Element> on Holder { T unused() => throw 0; } void uses<X extends SourceList<Element>>(X values, Holder holder) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let signature = signatures
                    .function(&model.resolve_value("uses").unwrap())
                    .unwrap();
                for (receiver, member) in [
                    (&signature.positional[0], "pick"),
                    (&signature.positional[1], "unused"),
                ] {
                    let (method, substitutions) =
                        signatures.resolved_member(receiver, member, model).unwrap();
                    assert_eq!(
                        model.substitute(&method.return_type, &substitutions),
                        ResolvedType::interface_identity(model.resolve_name("Element").unwrap()),
                    );
                }
                let (method, substitutions) = signatures
                    .resolved_extension_override_method(
                        &model.resolve_extension("Pick").unwrap(),
                        &ResolvedType::Unknown,
                        &[],
                        "pick",
                        model,
                    )
                    .unwrap();
                assert_eq!(
                    model.substitute(&method.return_type, &substitutions),
                    ResolvedType::Unknown,
                );
            },
        );
    }

    #[test]
    fn extension_applicability_proves_only_universal_missing_arguments() {
        with_model(
            "class Element {} class SourceFuture<T> {} extension Completion<T> on SourceFuture<T> { void get completed {} T element() => throw 0; } extension Fixed on SourceFuture<Element> { void fixed() {} } extension Bounded<T extends Element> on SourceFuture<T> { void bounded() {} } extension FunctionOwner<T> on T { void invoke() {} } void uses(SourceFuture<Element> future, void Function() callback) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let signature = signatures
                    .function(&model.resolve_value("uses").unwrap())
                    .unwrap();
                let mut receiver = signature.positional[0].clone();
                let ResolvedType::Interface { arguments, .. } = &mut receiver else {
                    panic!()
                };
                Arc::make_mut(arguments)[0] = ResolvedType::Unknown;
                for (name, expected) in [
                    ("Completion", Truth::Yes),
                    ("Fixed", Truth::Unknown),
                    ("Bounded", Truth::Unknown),
                ] {
                    let extension = signatures
                        .extensions
                        .iter()
                        .find(|extension| extension.name.as_deref() == Some(name))
                        .unwrap();
                    assert_eq!(
                        signatures
                            .instantiated_extension_on_type(&receiver, extension, model)
                            .1,
                        expected,
                        "{name}",
                    );
                }
                assert!(signatures.has_possible_extension_getter(&receiver, "completed", model));
                assert!(signatures.has_possible_extension_getter(
                    &signature.positional[0].with_nullable(true),
                    "completed",
                    model,
                ));
                let (method, substitutions) = signatures
                    .resolved_member(&receiver, "element", model)
                    .unwrap();
                assert_eq!(
                    model.substitute(&method.return_type, &substitutions),
                    ResolvedType::Unknown
                );
                assert_eq!(
                    signatures.resolved_extension_owner(&signature.positional[1], "invoke", model),
                    model.resolve_extension("FunctionOwner"),
                );
                assert_eq!(
                    signatures.subtype(&ResolvedType::Unknown, &ResolvedType::Dynamic, model),
                    Truth::Unknown,
                );
            },
        );
    }

    #[test]
    fn mixed_variance_extension_identity_does_not_conflate_missing_arguments() {
        with_model(
            "class Element {} class Pair<T, S> {} extension Mixed<T> on Pair<T, void Function(T)> { void mixed() {} } void uses(Pair<Element, void Function(Element)> pair) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let mut receiver = signatures
                    .function(&model.resolve_value("uses").unwrap())
                    .unwrap()
                    .positional[0]
                    .clone();
                let ResolvedType::Interface { arguments, .. } = &mut receiver else {
                    panic!()
                };
                Arc::make_mut(arguments)[0] = ResolvedType::Unknown;
                let ResolvedType::Function { positional, .. } = &mut Arc::make_mut(arguments)[1]
                else {
                    panic!()
                };
                Arc::make_mut(positional)[0] = ResolvedType::Unknown;
                assert_eq!(
                    signatures.instantiated_extension_on_type(
                        &receiver, &signatures.extensions[0], model,
                    ).1,
                    Truth::Unknown,
                );
                assert_eq!(
                    signatures.resolved_extension_owner(&receiver, "mixed", model),
                    None,
                );
            },
        );
    }

    #[test]
    fn dependent_extension_bounds_do_not_accept_proof_only_dynamic_arguments() {
        with_model(
            "class Element {} class Pair<T, S> {} extension Dependent<T, S extends T> on Pair<T, S> { void get dependent {} } void uses(Pair<Element, Element> pair) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let mut receiver = signatures
                    .function(&model.resolve_value("uses").unwrap())
                    .unwrap()
                    .positional[0]
                    .clone();
                let ResolvedType::Interface { arguments, .. } = &mut receiver else {
                    panic!()
                };
                Arc::make_mut(arguments)[0] = ResolvedType::Unknown;
                let extension = &signatures.extensions[0];
                assert_eq!(
                    signatures
                        .instantiated_extension_on_type(&receiver, extension, model)
                        .1,
                    Truth::Unknown,
                );
                assert_eq!(
                    signatures.resolved_extension_owner(&receiver, "dependent", model),
                    None,
                );
                assert!(signatures.has_possible_extension_getter(&receiver, "dependent", model));
            },
        );
    }

    #[test]
    fn extension_specificity_retains_missing_arguments_instead_of_proof_dynamic() {
        with_model(
            "class Element {} class Pair<T, S> {} extension Constrained<S extends Element> on Pair<dynamic, S> { void choose() {} } extension Ordinary<T, S> on Pair<T, S> { void choose() {} } void uses(Pair<Element, Element> pair) {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let mut receiver = signatures
                    .function(&model.resolve_value("uses").unwrap())
                    .unwrap()
                    .positional[0]
                    .clone();
                let ResolvedType::Interface { arguments, .. } = &mut receiver else {
                    panic!()
                };
                Arc::make_mut(arguments)[0] = ResolvedType::Unknown;
                let extension = signatures
                    .extensions
                    .iter()
                    .find(|extension| extension.name.as_deref() == Some("Ordinary"))
                    .unwrap();
                let (on_type, applicability, _) =
                    signatures.instantiated_extension_on_type(&receiver, extension, model);
                assert_eq!(applicability, Truth::Yes);
                assert_eq!(on_type.as_ref(), &receiver);
                assert_eq!(
                    signatures.resolved_extension_owner(&receiver, "choose", model),
                    None,
                );
            },
        );
    }

    #[test]
    fn extension_identity_proof_distinguishes_declared_from_receiver_derived_holes() {
        with_model(
            "class Element {} class Pair<T, S> {} extension Generic<T> on T { void invoke() {} } extension Fixed on Pair<Element, Element> { void get fixed {} } void uses(void Function(Element) callback, Pair<Element, Element> pair) {}",
            |program, model| {
                let mut signatures = SignatureIndex::from_program(program, model);
                let signature = signatures
                    .function(&model.resolve_value("uses").unwrap())
                    .unwrap();
                let mut callback = signature.positional[0].clone();
                let mut pair = signature.positional[1].clone();
                let ResolvedType::Function { positional, .. } = &mut callback else {
                    panic!()
                };
                Arc::make_mut(positional)[0] = ResolvedType::Unknown;
                assert_eq!(
                    signatures.resolved_extension_owner(&callback, "invoke", model),
                    model.resolve_extension("Generic"),
                );
                let ResolvedType::Interface { arguments, .. } = &mut pair else {
                    panic!()
                };
                Arc::make_mut(arguments)[0] = ResolvedType::Unknown;
                let fixed = signatures
                    .extensions
                    .iter_mut()
                    .find(|extension| extension.name.as_deref() == Some("Fixed"))
                    .unwrap();
                let ResolvedType::Interface { arguments, .. } = &mut fixed.on_type else {
                    panic!()
                };
                Arc::make_mut(arguments)[0] = ResolvedType::Unknown;
                let fixed = signatures
                    .extensions
                    .iter()
                    .find(|extension| extension.name.as_deref() == Some("Fixed"))
                    .unwrap();
                assert_eq!(
                    signatures
                        .instantiated_extension_on_type(&pair, fixed, model)
                        .1,
                    Truth::Unknown,
                );
                assert!(signatures.has_possible_extension_getter(&pair, "fixed", model));
            },
        );
    }

    #[test]
    fn mixin_super_concrete_lookup_excludes_implemented_interfaces() {
        with_model(
            "class Constraint {} class Interface { String toString() => ''; } mixin Mixed on Constraint implements Interface {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let receiver =
                    ResolvedType::interface_identity(model.resolve_name("Mixed").unwrap());
                assert_eq!(
                    signatures.resolved_super_member_owner(&receiver, "toString", false, model),
                    Some(DeclarationIdentity::OBJECT),
                );
            },
        );
    }

    #[test]
    fn project_extension_owner_beats_more_specific_platform_extension() {
        with_model(
            "class Receiver {} extension Platform on Receiver { void member() {} } extension Project on Object { void member() {} }",
            |program, model| {
                let mut signatures = SignatureIndex::from_program(program, model);
                signatures
                    .extensions
                    .iter_mut()
                    .find(|extension| extension.name.as_deref() == Some("Platform"))
                    .unwrap()
                    .identity = Some(DeclarationIdentity::sdk("dart:core", "Platform"));
                let receiver = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Receiver").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };
                assert_eq!(
                    signatures.resolved_extension_owner(&receiver, "member", model),
                    model.resolve_extension("Project")
                );
            },
        );
    }

    #[test]
    fn sdk_set_add_returns_bool() {
        with_model("void f(Set<int> values) {}", |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = &program.declarations[0] else {
                panic!()
            };
            let receiver = model.resolve_type(
                function.params().unwrap().params[0]
                    .param_type
                    .as_ref()
                    .unwrap(),
            );
            let (signature, _) = signatures.resolved_member(&receiver, "add", model).unwrap();
            assert!(signature.return_type.interface("dart:core", "bool"));
        });
    }

    #[test]
    fn sdk_iterable_map_preserves_its_call_type_parameter() {
        with_model(
            "Iterable<String> mapped(List<int> values) => values.map<String>((value) => '$value');",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let iterable = DeclarationIdentity::sdk("dart:core", "Iterable");
                let signature = signatures.member(&iterable, "map").unwrap();
                assert_eq!(signature.call_parameters.len(), 1);
                let ResolvedType::Function { return_type, .. } = &signature.positional[0] else {
                    panic!()
                };
                assert_eq!(
                    return_type.as_ref(),
                    &ResolvedType::TypeParameter {
                        id: signature.call_parameters[0].clone(),
                        bound: Arc::new(ResolvedType::Dynamic),
                        nullable: false,
                    }
                );

                let TopLevelDecl::Function(function) = &program.declarations[0] else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression, _)) = function.body.as_ref() else {
                    panic!()
                };
                let type_parameters = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &type_parameters);
                let inferred = environment.infer_with_signatures(
                    expression,
                    model,
                    &signatures,
                    &type_parameters,
                );
                let ResolvedType::Interface {
                    identity,
                    arguments,
                    ..
                } = inferred
                else {
                    panic!()
                };
                assert_eq!(identity, iterable);
                assert!(arguments[0].interface("dart:core", "String"));
            },
        );
    }

    #[test]
    fn private_inherited_members_are_visible_from_same_library_parts() {
        let root_path = PathBuf::from("/project/lib/root.dart");
        let part_path = PathBuf::from("/project/lib/part.dart");
        let falcon_dart_parser::ParseResult {
            program: root,
            errors: root_errors,
            ..
        } = parse(
            "library shared; part 'part.dart'; class Base { int _method() => 0; int _field = 0; }",
        );
        let falcon_dart_parser::ParseResult {
            program: part,
            errors: part_errors,
            ..
        } = parse("part of shared; class Child extends Base {}");
        assert!(root_errors.is_empty());
        assert!(part_errors.is_empty());
        let sources = [
            IdentitySource {
                path: &root_path,
                program: &root,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &part_path,
                program: &part,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: source.has_parse_errors,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files = [(root_path, &root), (part_path.clone(), &part)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&part_path, &identities, Some(&types));
        let child = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        };

        assert!(
            signatures
                .resolved_member(&child, "_method", &model)
                .is_some()
        );
        assert!(
            signatures
                .resolved_field(&child, "_field", &model)
                .is_some()
        );
        assert!(
            signatures
                .resolved_writable_field(&child, "_field", &model)
                .is_some()
        );
    }

    #[test]
    fn private_inherited_members_are_hidden_across_libraries() {
        let base_path = PathBuf::from("/project/lib/base.dart");
        let child_path = PathBuf::from("/project/lib/child.dart");
        let falcon_dart_parser::ParseResult {
            program: base,
            errors: base_errors,
            ..
        } = parse("class Base { int _method() => 0; int _field = 0; }");
        let falcon_dart_parser::ParseResult {
            program: child_program,
            errors: child_errors,
            ..
        } = parse("import 'base.dart'; class Child extends Base {}");
        assert!(base_errors.is_empty());
        assert!(child_errors.is_empty());
        let sources = [
            IdentitySource {
                path: &base_path,
                program: &base,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &child_path,
                program: &child_program,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: source.has_parse_errors,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files = [(base_path, &base), (child_path.clone(), &child_program)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&child_path, &identities, Some(&types));
        let child = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        };

        assert!(
            signatures
                .resolved_member(&child, "_method", &model)
                .is_none()
        );
        assert!(
            signatures
                .resolved_field(&child, "_field", &model)
                .is_none()
        );
        assert!(
            signatures
                .resolved_writable_field(&child, "_field", &model)
                .is_none()
        );
    }

    #[test]
    fn private_inherited_lookup_skips_invisible_override() {
        let base_path = PathBuf::from("/project/lib/a.dart");
        let middle_path = PathBuf::from("/project/lib/b.dart");
        let falcon_dart_parser::ParseResult {
            program: base,
            errors: base_errors,
            ..
        } = parse(
            "import 'b.dart'; class Base { int _method() => 0; int _field = 0; } class Child extends Middle {}",
        );
        let falcon_dart_parser::ParseResult {
            program: middle,
            errors: middle_errors,
            ..
        } = parse(
            "import 'a.dart'; class Middle extends Base { int _method() => 1; int _field = 1; }",
        );
        assert!(base_errors.is_empty());
        assert!(middle_errors.is_empty());
        let sources = [
            IdentitySource {
                path: &base_path,
                program: &base,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &middle_path,
                program: &middle,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: source.has_parse_errors,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files = [(base_path.clone(), &base), (middle_path, &middle)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&base_path, &identities, Some(&types));
        let base_identity = model.resolve_name("Base").unwrap();
        let child = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        };

        for (name, access) in [
            ("_method", InstanceAccess::Invoke),
            ("_field", InstanceAccess::Read),
            ("_field", InstanceAccess::Write),
        ] {
            let ResolvedType::Interface { identity, .. } = signatures
                .resolved_instance_owner(&child, name, access, &model)
                .unwrap()
            else {
                panic!()
            };
            assert_eq!(identity, base_identity);
        }
    }

    #[test]
    fn precomputed_cycles_match_uncached_queries_for_every_owner() {
        with_model(
            concat!(
                "class A extends B {} class B extends A {} ",
                "class C extends A {} class D extends C {} ",
                "class E {} class F implements E {} class G implements E, F {}"
            ),
            |program, model| {
                let mut signatures = SignatureIndex::from_program(program, model);
                let cached = signatures.cyclic_supertypes.take().unwrap();
                for identity in signatures.supertypes.keys() {
                    assert_eq!(
                        cached.contains(identity),
                        signatures.has_cyclic_supertypes(identity),
                        "{identity:?}"
                    );
                }
                for name in ["A", "B", "C", "D"] {
                    assert!(cached.contains(&model.resolve_name(name).unwrap()));
                }
                for name in ["E", "F", "G"] {
                    assert!(!cached.contains(&model.resolve_name(name).unwrap()));
                }
            },
        );
    }

    #[test]
    fn cyclic_supertypes_are_unknown() {
        with_model(
            "class A extends B { int field = 0; void method() {} } class B extends A {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let a = model.resolve_name("A").unwrap();
                let b = model.resolve_name("B").unwrap();
                let receiver = ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: a,
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                };

                assert_eq!(
                    signatures.is_subtype_of(&receiver, &b, model),
                    Truth::Unknown
                );
                assert!(
                    signatures
                        .instantiated_supertype(&receiver, "dart:core", "Object", model)
                        .is_none()
                );
                assert!(
                    signatures
                        .resolved_member(&receiver, "method", model)
                        .is_none()
                );
                assert!(
                    signatures
                        .resolved_field(&receiver, "field", model)
                        .is_none()
                );
            },
        );
    }

    #[test]
    fn field_formals_preserve_types_through_super_formal_chains() {
        with_model(
            "class Base<T> { final T value; Base(this.value); } class Middle extends Base<String> { Middle(super.value); } class Leaf extends Middle { Leaf(super.value); }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let base = model.resolve_name("Base").unwrap();
                assert!(contains_parameter(
                    &signatures.declaration(&base).unwrap().constructors[0].parameters[0].ty
                ));

                for name in ["Middle", "Leaf"] {
                    let identity = model.resolve_name(name).unwrap();
                    let facts = signatures.declaration(&identity).unwrap();
                    assert!(facts.constructors[0].parameters[0].is_super);
                    assert!(
                        facts.constructors[0].parameters[0]
                            .ty
                            .interface("dart:core", "String")
                    );
                    assert!(
                        signatures.constructor(&identity, "new").unwrap().positional[0]
                            .interface("dart:core", "String")
                    );
                }
            },
        );
    }

    #[test]
    fn super_formal_chains_resolve_across_files_regardless_of_index_order() {
        let base_path = PathBuf::from("/project/lib/base.dart");
        let middle_path = PathBuf::from("/project/lib/middle.dart");
        let leaf_path = PathBuf::from("/project/lib/leaf.dart");
        let falcon_dart_parser::ParseResult {
            program: base,
            errors: base_errors,
            ..
        } = parse("class Base<T> { final T value; Base(this.value); }");
        let falcon_dart_parser::ParseResult {
            program: middle,
            errors: middle_errors,
            ..
        } = parse("import 'base.dart'; class Middle extends Base<String> { Middle(super.value); }");
        let falcon_dart_parser::ParseResult {
            program: leaf,
            errors: leaf_errors,
            ..
        } = parse("import 'middle.dart'; class Leaf extends Middle { Leaf(super.value); }");
        assert!(base_errors.is_empty());
        assert!(middle_errors.is_empty());
        assert!(leaf_errors.is_empty());
        let sources = [
            IdentitySource {
                path: &leaf_path,
                program: &leaf,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &middle_path,
                program: &middle,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &base_path,
                program: &base,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: source.has_parse_errors,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files = [
            (leaf_path.clone(), &leaf),
            (middle_path, &middle),
            (base_path, &base),
        ];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&leaf_path, &identities, Some(&types));
        let leaf_identity = model.resolve_name("Leaf").unwrap();
        assert!(
            signatures.declaration(&leaf_identity).unwrap().constructors[0].parameters[0]
                .ty
                .interface("dart:core", "String")
        );
        assert!(
            signatures
                .constructor(&leaf_identity, "new")
                .unwrap()
                .positional[0]
                .interface("dart:core", "String")
        );
    }

    #[test]
    fn constructor_formal_fixpoint_handles_deep_chains_across_two_files() {
        let a_path = PathBuf::from("/project/lib/a.dart");
        let b_path = PathBuf::from("/project/lib/b.dart");
        let falcon_dart_parser::ParseResult {
            program: a,
            errors: a_errors,
            ..
        } = parse(
            "import 'b.dart'; class A0<T> { final T value; A0(this.value); } class A2 extends B1 { A2(super.value); } class A4 extends B3 { A4(super.value); } class A6 extends B5 { A6(super.value); }",
        );
        let falcon_dart_parser::ParseResult {
            program: b,
            errors: b_errors,
            ..
        } = parse(
            "import 'a.dart'; class B1 extends A0<String> { B1(super.value); } class B3 extends A2 { B3(super.value); } class B5 extends A4 { B5(super.value); }",
        );
        assert!(a_errors.is_empty());
        assert!(b_errors.is_empty());
        let sources = [
            IdentitySource {
                path: &a_path,
                program: &a,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &b_path,
                program: &b,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: source.has_parse_errors,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files = [(a_path.clone(), &a), (b_path, &b)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&a_path, &identities, Some(&types));
        let leaf = model.resolve_name("A6").unwrap();

        assert!(
            signatures.declaration(&leaf).unwrap().constructors[0].parameters[0]
                .ty
                .interface("dart:core", "String")
        );
        assert!(
            signatures.constructor(&leaf, "new").unwrap().positional[0]
                .interface("dart:core", "String")
        );
    }

    #[test]
    fn field_forwarders_check_private_getters_from_their_own_library() {
        // The unrelated library is indexed first: forwarder detection must not
        // judge `_value` from its scope, where every `_` name is invisible.
        let other_path = PathBuf::from("/project/lib/other.dart");
        let fake_path = PathBuf::from("/project/lib/fake.dart");
        let falcon_dart_parser::ParseResult {
            program: other,
            errors: other_errors,
            ..
        } = parse("class Other {}");
        let falcon_dart_parser::ParseResult {
            program: fake,
            errors: fake_errors,
            ..
        } = parse(
            "abstract class Api { int? get _value; } class Fake { dynamic noSuchMethod(Invocation invocation) => null; } class Impl extends Fake implements Api { final int? _value; Impl(this._value); }",
        );
        assert!(other_errors.is_empty());
        assert!(fake_errors.is_empty());
        let sources = [
            IdentitySource {
                path: &other_path,
                program: &other,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &fake_path,
                program: &fake,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let files = [(other_path, &other), (fake_path.clone(), &fake)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&fake_path, &identities, Some(&types));
        let receiver = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Impl").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        };

        assert!(signatures.is_promotable_field(&receiver, "_value", &model));
    }

    #[test]
    fn extension_getters_do_not_supply_override_inference() {
        with_model(
            "class Base {} extension Values on Base { String get value => ''; } \
             class Child extends Base { final value = 1; }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let base = ResolvedType::interface_identity(model.resolve_name("Base").unwrap());
                assert!(signatures.resolved_field(&base, "value", model).is_none());
                assert!(
                    signatures
                        .resolved_extension_getter(&base, "value", model)
                        .is_some()
                );
                let child = ResolvedType::interface_identity(model.resolve_name("Child").unwrap());
                let (ty, substitutions) =
                    signatures.resolved_field(&child, "value", model).unwrap();
                assert!(
                    model
                        .substitute(&ty, &substitutions)
                        .interface("dart:core", "int")
                );
            },
        );
    }

    #[test]
    fn extension_getter_reads_and_callable_getters_still_infer() {
        with_model(
            "class Target {} extension Values on Target { \
             int get value => 1; int Function() get callback => () => 1; } \
             int read(Target target) => target.value; \
             int call(Target target) => target.callback();",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for declaration in &program.declarations[2..] {
                    let TopLevelDecl::Function(function) = declaration else {
                        panic!()
                    };
                    let Some(FunctionBody::Arrow(expression, _)) = &function.body else {
                        panic!()
                    };
                    let scope = TypeParameterScope::default();
                    let mut environment = TypeEnvironment::new();
                    environment.bind_params(function.params().unwrap(), model, &scope);
                    assert!(
                        environment
                            .infer_with_signatures(expression, model, &signatures, &scope)
                            .interface("dart:core", "int")
                    );
                }
            },
        );
    }

    #[test]
    fn interface_getter_implementations_do_not_hide_no_such_method_forwarders() {
        with_model(
            "abstract class Contract { int? get _value; } \
             class Api implements Contract { final int? _value; Api(this._value); } \
             class Fake { dynamic noSuchMethod(Invocation invocation) => null; } \
             class Forwarded extends Fake implements Api {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let api = ResolvedType::interface_identity(model.resolve_name("Api").unwrap());
                assert!(!signatures.is_promotable_field(&api, "_value", model));
                let forwarded =
                    ResolvedType::interface_identity(model.resolve_name("Forwarded").unwrap());
                assert!(
                    signatures
                        .resolved_field(&forwarded, "_value", model)
                        .is_some()
                );
                assert!(
                    signatures
                        .resolved_owner_through(
                            &forwarded,
                            "_value",
                            InstanceAccess::Read,
                            false,
                            model,
                        )
                        .is_none()
                );
            },
        );
    }

    fn contains_parameter(ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::TypeParameter { .. })
    }
}
