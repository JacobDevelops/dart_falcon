//! The linter's `typesAreUnrelated` (`pkg/linter/lib/src/util/dart_type_utilities.dart`),
//! shared by `unrelated_type_equality_checks` and `collection_methods_unrelated_type`.
//!
//! Two types are related when one is a subtype of the other after promoting
//! both to non-nullable. Otherwise two interface types of the same declaration
//! are unrelated when a pair of type arguments is; two distinct declarations
//! are related only when they extend the same non-`Object` superclass and
//! neither is an enum (nor a `ProtobufEnum` subclass). Function types are
//! unrelated to everything but function types and classes with a concrete
//! `call` method; records are unrelated to what they are not assignable to or
//! from. Everything else (type parameters against non-type-parameters, ...)
//! might be related.

use crate::resolve::semantic::ResolvedTypeKind;
use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;

use super::{
    BorrowedIdentity, DeclarationIdentity, MemberFacts, ResolvedType, SemanticModel,
    SignatureIndex, Truth,
};
use crate::resolve::{MemberKind, TypeKind};

const PROTOBUF_ENUM_URI: &str = "package:protobuf/src/protobuf/protobuf_enum.dart";

type RelationMemo = FxHashMap<(ResolvedType, ResolvedType), Truth>;

/// Runtime overlap classification, separating proof from the concrete-type heuristic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeIntersection {
    Disjoint,
    UnrelatedConcrete,
    PossiblyRelated,
    Unknown,
}

fn memoized_relation(
    left: &ResolvedType,
    right: &ResolvedType,
    memo: &mut RelationMemo,
    compute: impl FnOnce(&mut RelationMemo) -> Truth,
) -> Truth {
    let has_children = |ty: &ResolvedType| match ty {
        ResolvedType {
            kind: ResolvedTypeKind::Interface { arguments, .. },
        } => !arguments.is_empty(),
        ResolvedType {
            kind: ResolvedTypeKind::Record {
                positional, named, ..
            },
        } => !positional.is_empty() || !named.is_empty(),
        ResolvedType {
            kind:
                ResolvedTypeKind::TypeParameter { .. }
                | ResolvedTypeKind::NonNullTypeParameter { .. }
                | ResolvedTypeKind::Function { .. },
        } => true,
        _ => false,
    };
    if !has_children(left) && !has_children(right) {
        return compute(memo);
    }
    // Owned shallow keys retain temporary promoted/instantiated types. Raw
    // pointer keys could alias a later temporary after its predecessor drops.
    let key = (left.clone(), right.clone());
    if let Some(result) = memo.get(&key) {
        return *result;
    }
    let result = compute(memo);
    memo.insert(key, result);
    result
}

impl SignatureIndex {
    /// Classifies runtime intersections separately from the equality lint's
    /// heuristic notion of unrelated declarations.
    #[must_use]
    pub fn runtime_intersection(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> RuntimeIntersection {
        let (Some(left), Some(right)) = (self.erase(left, model), self.erase(right, model)) else {
            return RuntimeIntersection::Unknown;
        };
        self.erased_intersection(&left, &right, model)
    }

    fn erased_intersection(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> RuntimeIntersection {
        use ResolvedTypeKind::{
            Dynamic, Function, Interface, NonNullTypeParameter, Null, Record, TypeParameter, Void,
        };
        use RuntimeIntersection::{Disjoint, PossiblyRelated, Unknown};
        if matches!(left.kind, ResolvedTypeKind::Unknown)
            || matches!(right.kind, ResolvedTypeKind::Unknown)
        {
            return Unknown;
        }
        if is_bottom(left)
            || is_bottom(right)
            || matches!(left.kind, Dynamic | Void)
            || matches!(right.kind, Dynamic | Void)
        {
            return PossiblyRelated;
        }
        if non_nullable(left) == Truth::No && non_nullable(right) == Truth::No {
            return PossiblyRelated;
        }
        if matches!(left.kind, Null) || matches!(right.kind, Null) {
            return if non_nullable(left) == Truth::Yes || non_nullable(right) == Truth::Yes {
                Disjoint
            } else {
                Unknown
            };
        }
        let left = left.non_null();
        let right = right.non_null();
        if self.resolved_subtype(&left, &right, model) == Truth::Yes
            || self.resolved_subtype(&right, &left, model) == Truth::Yes
        {
            return PossiblyRelated;
        }
        match (&left.kind, &right.kind) {
            (TypeParameter { bound, .. } | NonNullTypeParameter { bound, .. }, _) => {
                self.erased_intersection(bound, &right, model)
            }
            (_, TypeParameter { bound, .. } | NonNullTypeParameter { bound, .. }) => {
                self.erased_intersection(&left, bound, model)
            }
            (
                Record {
                    positional: a,
                    named: an,
                    ..
                },
                Record {
                    positional: b,
                    named: bn,
                    ..
                },
            ) => {
                if a.len() != b.len()
                    || an.len() != bn.len()
                    || an.iter().zip(bn.iter()).any(|((a, _), (b, _))| a != b)
                    || a.iter()
                        .zip(b.iter())
                        .chain(an.iter().zip(bn.iter()).map(|((_, a), (_, b))| (a, b)))
                        .any(|(a, b)| self.erased_intersection(a, b, model) == Disjoint)
                {
                    Disjoint
                } else {
                    PossiblyRelated
                }
            }
            (Function { .. }, Function { .. }) => PossiblyRelated,
            (Record { .. }, Function { .. }) | (Function { .. }, Record { .. }) => Disjoint,
            (
                Interface {
                    identity: a,
                    arguments: aa,
                    ..
                },
                Interface {
                    identity: b,
                    arguments: ba,
                    ..
                },
            ) if a == b => {
                // Covariant types can overlap at Never; retain that distinction
                // from a proof that no runtime value can inhabit both types.
                if aa
                    .iter()
                    .zip(ba.iter())
                    .any(|(a, b)| self.erased_intersection(a, b, model) == Disjoint)
                {
                    RuntimeIntersection::UnrelatedConcrete
                } else {
                    PossiblyRelated
                }
            }
            (Interface { .. }, _) | (_, Interface { .. }) => {
                self.nominal_intersection(&left, &right, model)
            }
            _ => Unknown,
        }
    }

    fn nominal_intersection(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> RuntimeIntersection {
        use RuntimeIntersection::{Disjoint, PossiblyRelated, Unknown, UnrelatedConcrete};
        let forward = self.closed_intersection(left, right, model);
        if forward == Disjoint {
            return Disjoint;
        }
        let backward = self.closed_intersection(right, left, model);
        if backward == Disjoint {
            Disjoint
        } else if forward == Unknown || backward == Unknown {
            Unknown
        } else if self.concrete_cast_type(left, model)
            && self.concrete_cast_type(right, model)
            && self.resolved_subtype(left, right, model) == Truth::No
            && self.resolved_subtype(right, left, model) == Truth::No
        {
            UnrelatedConcrete
        } else {
            PossiblyRelated
        }
    }

    fn concrete_cast_type(&self, ty: &ResolvedType, model: &SemanticModel<'_>) -> bool {
        let ResolvedTypeKind::Interface { identity, .. } = &ty.kind else {
            return false;
        };
        if matches!(
            self.effective_restriction(identity),
            Some(Some(super::ClassRestriction::Base))
        ) {
            return false;
        }
        matches!(model.identities().source_declaration(identity),
            Some((_, _, super::TopLevelDecl::Class(node)))
                if !node.modifiers().is_abstract
                    && node.modifiers().restriction != Some(super::ClassRestriction::Interface))
    }

    fn closed_intersection(
        &self,
        closed: &ResolvedType,
        other: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> RuntimeIntersection {
        use RuntimeIntersection::{Disjoint, PossiblyRelated, Unknown};
        let ResolvedTypeKind::Interface {
            identity,
            arguments,
            ..
        } = &closed.kind
        else {
            return PossiblyRelated;
        };
        // These SDK types cannot acquire new user-defined subtypes.
        let primitive = matches!(identity, DeclarationIdentity::Sdk { library, name }
            if library == "dart:core" && matches!(name.as_str(), "int" | "double" | "bool" | "String"));
        if primitive || self.type_kind(identity) == Some(TypeKind::Enum) {
            if !arguments.is_empty()
                || self.type_facts.get(identity).is_some_and(|facts| {
                    facts
                        .supertypes
                        .as_ref()
                        .is_some_and(|(parameters, _)| !parameters.is_empty())
                })
            {
                return Unknown;
            }
            return match self.resolved_subtype(closed, other, model) {
                Truth::No => Disjoint,
                Truth::Yes => PossiblyRelated,
                Truth::Unknown => Unknown,
            };
        }
        let Some(facts) = self.type_facts.get(identity) else {
            return Unknown;
        };
        if !matches!(
            facts.restriction,
            Some(super::ClassRestriction::Final | super::ClassRestriction::Sealed)
        ) {
            return PossiblyRelated;
        }
        if !arguments.is_empty() {
            return Unknown;
        }
        let identities = model.identities();
        if identities.declaration_library(identity).is_none() {
            return Unknown;
        }
        // A missing part can hide an implementer. Do not use a partial source
        // unit as evidence that a library's subtype set is closed.
        let Some((_, parsed, _)) = identities.source_declaration(identity) else {
            return Unknown;
        };
        if parsed.program().part_directives().next().is_some()
            || parsed.program().part_of_directive().is_some()
        {
            return Unknown;
        }
        let Some(candidates) = self
            .closed_subtypes
            .get(identity)
            .and_then(Option::as_deref)
        else {
            return Unknown;
        };
        let mut unknown = false;
        for candidate in candidates {
            let facts = self
                .type_facts
                .get(candidate)
                .expect("cached indexed subtype");
            // A base or open descendant can be extended outside the library
            // and that subclass can implement another interface.
            if !matches!(
                facts.restriction,
                Some(super::ClassRestriction::Final | super::ClassRestriction::Sealed)
            ) && facts.kind != Some(TypeKind::Enum)
            {
                return PossiblyRelated;
            }
            if facts
                .supertypes
                .as_ref()
                .is_some_and(|(parameters, _)| !parameters.is_empty())
            {
                unknown = true;
                continue;
            }
            match self.resolved_subtype(&model.declared_type(candidate), other, model) {
                Truth::Yes => return PossiblyRelated,
                Truth::Unknown => unknown = true,
                Truth::No => {}
            }
        }
        if unknown { Unknown } else { Disjoint }
    }

    pub(super) fn rebuild_closed_subtypes(&mut self, identities: &super::IdentityIndex) {
        let mut libraries: super::FxHashMap<_, Vec<_>> = super::FxHashMap::default();
        for (identity, facts) in self.type_facts.iter() {
            if matches!(
                facts.kind,
                Some(TypeKind::Class | TypeKind::MixinClass | TypeKind::Mixin | TypeKind::Enum)
            ) && let Some(library) = identities.declaration_library(identity)
            {
                libraries.entry(library).or_default().push(identity);
            }
        }
        self.closed_subtypes = self
            .type_facts
            .iter()
            .filter(|(_, facts)| {
                matches!(
                    facts.restriction,
                    Some(super::ClassRestriction::Final | super::ClassRestriction::Sealed)
                ) || facts.kind == Some(TypeKind::Enum)
            })
            .filter_map(|(identity, _)| {
                let library = identities.declaration_library(identity)?;
                let candidates = libraries
                    .get(&library)?
                    .iter()
                    .filter_map(|candidate| {
                        match self.supertype_matches(candidate, &|ancestor| ancestor == identity) {
                            Truth::No => None,
                            Truth::Unknown => Some(None),
                            Truth::Yes => Some(Some((*candidate).clone())),
                        }
                    })
                    .collect::<Option<Vec<_>>>();
                Some((identity.clone(), candidates))
            })
            .collect();
    }

    /// Erases extension types to their instantiated runtime representation.
    /// Unknown representations or cyclic declarations remain unknown.
    #[must_use]
    pub fn extension_type_erasure(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.erase(ty, model)
    }

    /// Whether a type excludes null, including extension types implementing Object.
    #[must_use]
    pub fn is_non_nullable(&self, ty: &ResolvedType) -> bool {
        match ty {
            ResolvedType {
                kind:
                    ResolvedTypeKind::TypeParameter {
                        bound, nullable, ..
                    },
            } => !nullable && self.is_non_nullable(bound),
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        extension_type: true,
                        nullable: false,
                        ..
                    },
            } => self.implements_sdk(ty, "dart:core", "Object") == Truth::Yes,
            ty if !ty.nullable() => future_or_argument(ty).map_or_else(
                || ty.is_non_nullable(),
                |argument| self.is_non_nullable(argument),
            ),
            _ => false,
        }
    }

    /// The linter's `typesAreUnrelated`: `Yes` when the types are proven
    /// unrelated, `No` when they might be related, `Unknown` when falcon cannot
    /// tell.
    pub(super) fn runtime_unrelated(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Truth {
        match (self.erase(left, model), self.erase(right, model)) {
            (Some(left), Some(right)) => self.erased_unrelated(&left, &right, model),
            _ => Truth::Unknown,
        }
    }

    fn erased_unrelated(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Truth {
        self.erased_unrelated_memoized(left, right, model, &mut RelationMemo::default())
    }

    fn erased_unrelated_memoized(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
        memo: &mut RelationMemo,
    ) -> Truth {
        memoized_relation(left, right, memo, |memo| {
            self.erased_unrelated_uncached(left, right, model, memo)
        })
    }

    fn erased_unrelated_uncached(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
        memo: &mut RelationMemo,
    ) -> Truth {
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
            return Truth::Unknown;
        }
        if is_bottom(left)
            || is_bottom(right)
            || *left == ResolvedType::DYNAMIC
            || *right == ResolvedType::DYNAMIC
        {
            return Truth::No;
        }
        if *left == ResolvedType::NULL || *right == ResolvedType::NULL {
            let other = if *left == ResolvedType::NULL {
                right
            } else {
                left
            };
            if non_nullable(other) == Truth::Yes {
                return Truth::Yes;
            }
        }
        let left = promote_non_null(left);
        let right = promote_non_null(right);
        if left == right {
            return Truth::No;
        }
        let forward = self.resolved_subtype(&left, &right, model);
        let backward = self.resolved_subtype(&right, &left, model);
        if forward == Truth::Yes || backward == Truth::Yes {
            return Truth::No;
        }
        let neither_subtype = if forward == Truth::No && backward == Truth::No {
            Truth::Yes
        } else {
            Truth::Unknown
        };
        let unrelated = self.promoted_unrelated(&left, &right, model, memo);
        match (neither_subtype, unrelated) {
            (_, Truth::No) => Truth::No,
            (Truth::Yes, unrelated) => unrelated,
            _ => Truth::Unknown,
        }
    }

    fn promoted_unrelated(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
        memo: &mut RelationMemo,
    ) -> Truth {
        match (left, right) {
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Interface { .. },
                },
                ResolvedType {
                    kind: ResolvedTypeKind::Interface { .. },
                },
            ) => self.interfaces_unrelated(left, right, model, memo),
            (
                ResolvedType {
                    kind:
                        ResolvedTypeKind::TypeParameter { bound: left, .. }
                        | ResolvedTypeKind::NonNullTypeParameter { bound: left, .. },
                },
                ResolvedType {
                    kind:
                        ResolvedTypeKind::TypeParameter { bound: right, .. }
                        | ResolvedTypeKind::NonNullTypeParameter { bound: right, .. },
                },
            ) => {
                if unbounded(left) || unbounded(right) {
                    Truth::No
                } else {
                    self.erased_unrelated_memoized(left, right, model, memo)
                }
            }
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Function { .. },
                },
                other,
            )
            | (
                other,
                ResolvedType {
                    kind: ResolvedTypeKind::Function { .. },
                },
            ) => self.unrelated_to_function(other),
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Record { .. },
                },
                _,
            )
            | (
                _,
                ResolvedType {
                    kind: ResolvedTypeKind::Record { .. },
                },
            ) => Truth::Yes,
            _ => Truth::No,
        }
    }

    fn interfaces_unrelated(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
        memo: &mut RelationMemo,
    ) -> Truth {
        let (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity: left_identity,
                        arguments: left_arguments,
                        ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity: right_identity,
                        arguments: right_arguments,
                        ..
                    },
            },
        ) = (left, right)
        else {
            return Truth::Unknown;
        };
        if left_identity == right_identity {
            if left_arguments.len() != right_arguments.len() {
                return Truth::No;
            }
            return any(left_arguments
                .iter()
                .zip(right_arguments.iter())
                .map(|(left, right)| self.erased_unrelated_memoized(left, right, model, memo)));
        }
        if [left_identity, right_identity].into_iter().any(|identity| {
            self.type_kind(identity).is_none()
                && matches!(identity, DeclarationIdentity::Sdk { .. })
                && self
                    .type_facts
                    .get(identity)
                    .is_some_and(|facts| facts.supertypes.is_some())
        }) {
            // Source-free SDK declarations retain the engine's existing
            // fallback relation; its supertype graph is not an extends table.
            // A project TypeIndex may be incomplete because of imports even
            // when this index has complete SDK edges. Use those identity
            // edges when the name-based fallback cannot prove either result.
            return match self.instantiated_unrelated(left, right, model) {
                Truth::Unknown => match (
                    self.resolved_subtype(left, right, model),
                    self.resolved_subtype(right, left, model),
                ) {
                    (Truth::No, Truth::No) => Truth::Yes,
                    (Truth::Yes, _) | (_, Truth::Yes) => Truth::No,
                    _ => Truth::Unknown,
                },
                result => result,
            };
        }
        let member_supertypes = |identity| {
            self.type_facts
                .get(identity)
                .and_then(|facts| facts.member_supertypes.as_ref())
                .map(|types| match types.as_slice() {
                    [
                        types @ ..,
                        ResolvedType {
                            kind: ResolvedTypeKind::Interface { identity, .. },
                        },
                    ] if *identity == DeclarationIdentity::OBJECT => types,
                    types => types,
                })
        };
        for identity in [left_identity, right_identity] {
            match self.type_kind(identity) {
                Some(TypeKind::Class | TypeKind::MixinClass) => {}
                Some(TypeKind::Mixin | TypeKind::Enum) => return Truth::Yes,
                _ => return Truth::Unknown,
            }
            let Some(supertypes) = member_supertypes(identity) else {
                return Truth::Unknown;
            };
            // Member lookup records `with` types followed by the explicit
            // superclass. Any additional entries mean a synthetic mixin
            // application, which distinct declarations cannot share.
            if supertypes.len() != usize::from(self.superclass(identity).is_some()) {
                return Truth::Yes;
            }
        }
        // These entries were resolved in the declaration's type-parameter
        // scope, unlike the raw explicit-extends metadata.
        let left_superclass = member_supertypes(left_identity).and_then(|types| types.last());
        let right_superclass = member_supertypes(right_identity).and_then(|types| types.last());
        if left_superclass != right_superclass {
            return Truth::Yes;
        }
        let Some(superclass) = left_superclass else {
            // Two implicit Object superclasses do not establish relatedness.
            return Truth::Yes;
        };
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = superclass
        else {
            return Truth::Unknown;
        };
        let protobuf_enum = identity.name() == "ProtobufEnum"
            && (identity.is_package("protobuf", "ProtobufEnum")
                || model.declaration_uri(identity) == Some(PROTOBUF_ENUM_URI));
        (protobuf_enum || *identity == DeclarationIdentity::OBJECT).into()
    }

    /// The linter's `_isTypeUnrelatedToFunctionType`.
    fn unrelated_to_function(&self, ty: &ResolvedType) -> Truth {
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = ty
        else {
            return (!matches!(
                ty,
                ResolvedType {
                    kind: ResolvedTypeKind::Function { .. }
                }
            ))
            .into();
        };
        match self.type_kind(identity) {
            None if matches!(identity, DeclarationIdentity::Sdk { .. })
                && self
                    .type_facts
                    .get(identity)
                    .is_some_and(|facts| facts.supertypes.is_some()) =>
            {
                Truth::Yes
            }
            None => Truth::Unknown,
            Some(TypeKind::Mixin | TypeKind::Enum) => Truth::Yes,
            Some(_) => {
                let concrete_call = |facts: &[MemberFacts]| {
                    facts.iter().any(|fact| {
                        fact.kind == MemberKind::Method && !fact.is_abstract && !fact.is_static
                    })
                };
                if concrete_call(self.declared_member_facts(identity, "call")) {
                    return Truth::No;
                }
                match self.inherited_member_facts(identity, "call") {
                    None => Truth::Unknown,
                    Some(facts) => (!concrete_call(&facts)).into(),
                }
            }
        }
    }

    /// Three-valued `sub <: sup` over erased types.
    pub(super) fn resolved_subtype(
        &self,
        sub: &ResolvedType,
        sup: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Truth {
        self.subtype_for_extension(sub, sup, model, false)
    }

    /// Extension constraints may prove a missing argument fits a universal top
    /// type, without treating missing concrete or bounded arguments as known.
    pub(super) fn subtype_for_extension(
        &self,
        sub: &ResolvedType,
        sup: &ResolvedType,
        model: &SemanticModel<'_>,
        prove_unknown_top: bool,
    ) -> Truth {
        self.subtype_memoized(
            sub,
            sup,
            model,
            prove_unknown_top,
            &mut RelationMemo::default(),
        )
    }

    fn subtype_memoized(
        &self,
        sub: &ResolvedType,
        sup: &ResolvedType,
        model: &SemanticModel<'_>,
        prove_unknown_top: bool,
        memo: &mut RelationMemo,
    ) -> Truth {
        memoized_relation(sub, sup, memo, |memo| {
            self.subtype_uncached(sub, sup, model, prove_unknown_top, memo)
        })
    }

    fn subtype_uncached(
        &self,
        sub: &ResolvedType,
        sup: &ResolvedType,
        model: &SemanticModel<'_>,
        prove_unknown_top: bool,
        memo: &mut RelationMemo,
    ) -> Truth {
        match (sub, sup) {
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown,
                },
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic | ResolvedTypeKind::Void,
                },
            ) if prove_unknown_top => return Truth::Yes,
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown,
                },
                _,
            )
            | (
                _,
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown,
                },
            ) => return Truth::Unknown,
            (
                _,
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic | ResolvedTypeKind::Void,
                },
            )
            | (
                ResolvedType {
                    kind: ResolvedTypeKind::Never,
                },
                _,
            ) => return Truth::Yes,
            (
                _,
                ResolvedType {
                    kind:
                        ResolvedTypeKind::Interface {
                            identity,
                            nullable: true,
                            ..
                        },
                },
            ) if *identity == DeclarationIdentity::OBJECT => {
                return Truth::Yes;
            }
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Dynamic | ResolvedTypeKind::Void,
                },
                _,
            ) => {
                return self.subtype_memoized(
                    &model.core_type("Object", true),
                    sup,
                    model,
                    prove_unknown_top,
                    memo,
                );
            }
            (
                ResolvedType {
                    kind: ResolvedTypeKind::Null,
                },
                _,
            ) => return accepts_null(sup),
            _ => {}
        }
        self.nullable_subtype(sub, sup, model, prove_unknown_top, memo)
    }

    fn nullable_subtype(
        &self,
        sub: &ResolvedType,
        sup: &ResolvedType,
        model: &SemanticModel<'_>,
        prove_unknown_top: bool,
        memo: &mut RelationMemo,
    ) -> Truth {
        if sub.nullable() {
            match accepts_null(sup) {
                Truth::Yes => {}
                other => return other,
            }
        }
        let sub = sub.with_nullable(false);
        let sup = sup.with_nullable(false);
        if let Some(argument) = future_or_argument(&sub) {
            return all([
                self.subtype_memoized(argument, &sup, model, prove_unknown_top, memo),
                self.subtype_memoized(&future_of(argument), &sup, model, prove_unknown_top, memo),
            ]);
        }
        if let Some(argument) = future_or_argument(&sup) {
            return any([
                self.subtype_memoized(&sub, argument, model, prove_unknown_top, memo),
                self.subtype_memoized(&sub, &future_of(argument), model, prove_unknown_top, memo),
            ]);
        }
        match (&sub, &sup) {
            (
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { id, .. },
                },
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { id: target, .. },
                },
            ) if id == target => Truth::Yes,
            (
                ResolvedType {
                    kind: ResolvedTypeKind::NonNullTypeParameter { id, .. },
                },
                ResolvedType {
                    kind:
                        ResolvedTypeKind::TypeParameter { id: target, .. }
                        | ResolvedTypeKind::NonNullTypeParameter { id: target, .. },
                },
            ) if id == target => Truth::Yes,
            (
                ResolvedType {
                    kind: ResolvedTypeKind::NonNullTypeParameter { bound, .. },
                },
                _,
            ) => self.subtype_memoized(&bound.non_null(), &sup, model, prove_unknown_top, memo),
            (
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { bound, .. },
                },
                _,
            ) => self.subtype_memoized(bound, &sup, model, prove_unknown_top, memo),
            (
                _,
                ResolvedType {
                    kind: ResolvedTypeKind::Interface { identity, .. },
                },
            ) if *identity == DeclarationIdentity::OBJECT => Truth::Yes,
            (
                _,
                ResolvedType {
                    kind:
                        ResolvedTypeKind::TypeParameter { .. }
                        | ResolvedTypeKind::NonNullTypeParameter { .. },
                },
            ) => Truth::No,
            _ => self.structural_subtype(&sub, &sup, model, prove_unknown_top, memo),
        }
    }

    fn structural_subtype(
        &self,
        sub: &ResolvedType,
        sup: &ResolvedType,
        model: &SemanticModel<'_>,
        prove_unknown_top: bool,
        memo: &mut RelationMemo,
    ) -> Truth {
        match (&sub.kind, &sup.kind) {
            (ResolvedTypeKind::Function { .. }, ResolvedTypeKind::Function { .. }) => {
                self.function_subtype(sub, sup, model, prove_unknown_top, memo)
            }
            (ResolvedTypeKind::Function { .. }, ResolvedTypeKind::Interface { identity, .. }) => {
                identity.is_sdk("dart:core", "Function").into()
            }
            (ResolvedTypeKind::Record { .. }, ResolvedTypeKind::Interface { identity, .. }) => {
                identity.is_sdk("dart:core", "Record").into()
            }
            (
                ResolvedTypeKind::Record {
                    positional: source_positional,
                    named: source_named,
                    ..
                },
                ResolvedTypeKind::Record {
                    positional: target_positional,
                    named: target_named,
                    ..
                },
            ) => {
                if source_positional.len() != target_positional.len()
                    || !source_named
                        .iter()
                        .map(|(name, _)| name)
                        .eq(target_named.iter().map(|(name, _)| name))
                {
                    return Truth::No;
                }
                all(source_positional
                    .iter()
                    .zip(target_positional.iter())
                    .chain(
                        source_named
                            .iter()
                            .zip(target_named.iter())
                            .map(|((_, source), (_, target))| (source, target)),
                    )
                    .map(|(source, target)| {
                        self.subtype_memoized(source, target, model, prove_unknown_top, memo)
                    }))
            }
            (
                ResolvedTypeKind::Interface { .. },
                ResolvedTypeKind::Function { .. } | ResolvedTypeKind::Record { .. },
            )
            | (ResolvedTypeKind::Function { .. } | ResolvedTypeKind::Record { .. }, _) => Truth::No,
            (
                ResolvedTypeKind::Interface {
                    identity: source_identity,
                    ..
                },
                ResolvedTypeKind::Interface {
                    identity: target_identity,
                    arguments: target_arguments,
                    ..
                },
            ) => match self.instantiated_identity(sub, target_identity, model) {
                Some(instantiated) => {
                    let arguments = instantiated.arguments();
                    if arguments.len() != target_arguments.len() {
                        return Truth::Unknown;
                    }
                    all(arguments
                        .iter()
                        .zip(target_arguments.iter())
                        .map(|(source, target)| {
                            self.subtype_memoized(source, target, model, prove_unknown_top, memo)
                        }))
                }
                None => match self
                    .identity_implements(source_identity, &|identity| identity == target_identity)
                {
                    Truth::No => Truth::No,
                    _ => Truth::Unknown,
                },
            },
            _ => Truth::Unknown,
        }
    }

    /// Function subtyping preserves invocation shape, is covariant in the
    /// result, and contravariant in every accepted argument.
    fn function_subtype(
        &self,
        sub: &ResolvedType,
        sup: &ResolvedType,
        model: &SemanticModel<'_>,
        prove_unknown_top: bool,
        memo: &mut RelationMemo,
    ) -> Truth {
        let (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type: source_return,
                        positional: source_positional,
                        named: source_named,
                        optional_positional: source_optional,
                        shape: Some(source_shape),
                        ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type: target_return,
                        positional: target_positional,
                        named: target_named,
                        optional_positional: target_optional,
                        shape: Some(target_shape),
                        ..
                    },
            },
        ) = (sub, sup)
        else {
            return Truth::Unknown;
        };
        if source_positional.len().saturating_sub(*source_optional)
            > target_positional.len().saturating_sub(*target_optional)
            || source_positional.len() < target_positional.len()
            || source_shape.type_parameters.len() != target_shape.type_parameters.len()
            || source_shape
                .required_named
                .iter()
                .any(|name| !target_shape.required_named.contains(name))
            || target_named
                .iter()
                .any(|(name, _)| !source_named.iter().any(|(other, _)| name == other))
        {
            return Truth::No;
        }
        let (substitutions, mut result) = match self.function_type_parameter_relation(
            &source_shape.type_parameters,
            &target_shape.type_parameters,
            model,
            prove_unknown_top,
            memo,
        ) {
            Ok(relation) => relation,
            Err(result) => return result,
        };
        let substitute = |ty| {
            if substitutions.is_empty() {
                Cow::Borrowed(ty)
            } else {
                Cow::Owned(model.substitute(ty, &substitutions))
            }
        };
        let target_return = substitute(target_return.as_ref());
        result = all([
            result,
            self.subtype_memoized(
                source_return,
                &target_return,
                model,
                prove_unknown_top,
                memo,
            ),
        ]);
        if result == Truth::No {
            return result;
        }
        for (source, target) in source_positional
            .iter()
            .zip(target_positional.iter())
            .chain(target_named.iter().filter_map(|(name, target)| {
                source_named
                    .iter()
                    .find(|(other, _)| name == other)
                    .map(|(_, source)| (source, target))
            }))
        {
            let target = substitute(target);
            result = all([
                result,
                self.subtype_memoized(&target, source, model, prove_unknown_top, memo),
            ]);
            if result == Truth::No {
                return result;
            }
        }
        result
    }

    fn function_type_parameter_relation(
        &self,
        source_parameters: &[ResolvedType],
        target_parameters: &[ResolvedType],
        model: &SemanticModel<'_>,
        prove_unknown_top: bool,
        memo: &mut RelationMemo,
    ) -> Result<(FxHashMap<super::TypeParameterId, ResolvedType>, Truth), Truth> {
        let mut substitutions = FxHashMap::default();
        for (source_parameter, target_parameter) in
            source_parameters.iter().zip(target_parameters.iter())
        {
            let (
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { id: source_id, .. },
                },
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { id: target_id, .. },
                },
            ) = (source_parameter, target_parameter)
            else {
                return Err(Truth::Unknown);
            };
            if source_id != target_id {
                substitutions.insert(target_id.clone(), source_parameter.clone());
            }
        }
        let mut result = Truth::Yes;
        for (source_parameter, target_parameter) in
            source_parameters.iter().zip(target_parameters.iter())
        {
            let (
                ResolvedType {
                    kind:
                        ResolvedTypeKind::TypeParameter {
                            bound: source_bound,
                            ..
                        },
                },
                ResolvedType {
                    kind:
                        ResolvedTypeKind::TypeParameter {
                            bound: target_bound,
                            ..
                        },
                },
            ) = (source_parameter, target_parameter)
            else {
                return Err(Truth::Unknown);
            };
            let target_bound = if substitutions.is_empty() {
                Cow::Borrowed(target_bound.as_ref())
            } else {
                Cow::Owned(model.substitute(target_bound, &substitutions))
            };
            result = all([
                result,
                self.subtype_memoized(source_bound, &target_bound, model, prove_unknown_top, memo),
                self.subtype_memoized(&target_bound, source_bound, model, prove_unknown_top, memo),
            ]);
            if result == Truth::No {
                return Err(result);
            }
        }
        Ok((substitutions, result))
    }

    /// The analyzer's `extensionTypeErasure`; `None` when a representation
    /// type is unknown.
    fn erase(&self, ty: &ResolvedType, model: &SemanticModel<'_>) -> Option<ResolvedType> {
        // Ordinary types are already erased. Avoid rebuilding their shared graph.
        if super::find_type_including_bounds(
            ty,
            &mut |ty| {
                matches!(
                    ty,
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface {
                            extension_type: true,
                            ..
                        }
                    }
                )
            },
            &mut FxHashSet::default(),
            true,
        )
        .is_none()
        {
            return Some(ty.clone());
        }
        self.erase_memoized(
            ty,
            model,
            &mut FxHashMap::default(),
            &mut FxHashSet::default(),
        )
    }

    fn erase_memoized<'a>(
        &self,
        ty: &'a ResolvedType,
        model: &SemanticModel<'_>,
        memo: &mut FxHashMap<BorrowedIdentity<'a, ResolvedType>, Option<ResolvedType>>,
        active: &mut FxHashSet<DeclarationIdentity>,
    ) -> Option<ResolvedType> {
        let key = BorrowedIdentity::new(ty);
        if let Some(result) = memo.get(&key) {
            return result.clone();
        }
        let result = match ty {
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity,
                        arguments,
                        nullable,
                        extension_type: true,
                        ..
                    },
            } => {
                // A repeated declaration in a finite argument (E<E<int>>)
                // is not a representation cycle. Erase supplied arguments
                // before marking this declaration's representation active.
                let arguments = arguments
                    .iter()
                    .map(|argument| self.erase_memoized(argument, model, memo, active))
                    .collect::<Option<Vec<_>>>()?;
                if !active.insert(identity.clone()) {
                    return None;
                }
                let result = (|| {
                    let representation = self
                        .declaration(identity)?
                        .extension_representation
                        .as_ref()?;
                    let (parameters, _) = self.type_facts.get(identity)?.supertypes.as_ref()?;
                    let substitutions: FxHashMap<_, _> =
                        parameters.iter().cloned().zip(arguments).collect();
                    let representation = model.substitute(representation, &substitutions);
                    // This substituted graph is temporary: do not retain its
                    // addresses in the caller's memo after it is dropped.
                    let erased = self.erase_memoized(
                        &representation,
                        model,
                        &mut FxHashMap::default(),
                        active,
                    )?;
                    Some(if *nullable {
                        erased.with_nullable(true)
                    } else {
                        erased
                    })
                })();
                active.remove(identity);
                result
            }
            ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        occurrence,
                        identity,
                        arguments,
                        nullable,
                        extension_type: false,
                    },
            } => Some(ResolvedType::from_kind(ResolvedTypeKind::Interface {
                occurrence: occurrence.clone(),
                identity: identity.clone(),
                arguments: arguments
                    .iter()
                    .map(|ty| self.erase_memoized(ty, model, memo, active))
                    .collect::<Option<Vec<_>>>()?
                    .into(),
                nullable: *nullable,
                extension_type: false,
            })),
            ResolvedType {
                kind:
                    ResolvedTypeKind::TypeParameter { .. }
                    | ResolvedTypeKind::NonNullTypeParameter { .. }
                    | ResolvedTypeKind::Function { .. }
                    | ResolvedTypeKind::Record { .. },
            } => Some(self.erase_structural(ty, model, memo, active)?),
            other => return Some(other.clone()),
        };
        memo.insert(key, result.clone());
        result
    }

    fn erase_structural<'a>(
        &self,
        ty: &'a ResolvedType,
        model: &SemanticModel<'_>,
        memo: &mut FxHashMap<BorrowedIdentity<'a, ResolvedType>, Option<ResolvedType>>,
        active: &mut FxHashSet<DeclarationIdentity>,
    ) -> Option<ResolvedType> {
        match ty {
            ResolvedType {
                kind:
                    ResolvedTypeKind::TypeParameter {
                        id,
                        bound,
                        nullable,
                    },
            } => Some(ResolvedType::from_kind(ResolvedTypeKind::TypeParameter {
                id: id.clone(),
                bound: std::sync::Arc::new(self.erase_memoized(bound, model, memo, active)?),
                nullable: *nullable,
            })),
            ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { id, bound },
            } => Some(ResolvedType::from_kind(
                ResolvedTypeKind::NonNullTypeParameter {
                    id: id.clone(),
                    bound: std::sync::Arc::new(self.erase_memoized(bound, model, memo, active)?),
                },
            )),
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type,
                        positional,
                        named,
                        nullable,
                        alias,
                        optional_positional,
                        shape,
                    },
            } => Some(ResolvedType::from_kind(ResolvedTypeKind::Function {
                optional_positional: *optional_positional,
                shape: match shape {
                    Some(shape) => Some(self.erase_function_shape(shape, model, memo, active)?),
                    None => None,
                },
                alias: alias.clone(),
                return_type: std::sync::Arc::new(self.erase_memoized(
                    return_type,
                    model,
                    memo,
                    active,
                )?),
                positional: positional
                    .iter()
                    .map(|ty| self.erase_memoized(ty, model, memo, active))
                    .collect::<Option<Vec<_>>>()?
                    .into(),
                named: named
                    .iter()
                    .map(|(name, ty)| {
                        Some((name.clone(), self.erase_memoized(ty, model, memo, active)?))
                    })
                    .collect::<Option<Vec<_>>>()?
                    .into(),
                nullable: *nullable,
            })),
            ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        occurrence,
                        positional,
                        named,
                        nullable,
                    },
            } => Some(ResolvedType::from_kind(ResolvedTypeKind::Record {
                occurrence: occurrence.clone(),
                positional: positional
                    .iter()
                    .map(|ty| self.erase_memoized(ty, model, memo, active))
                    .collect::<Option<Vec<_>>>()?
                    .into(),
                named: named
                    .iter()
                    .map(|(name, ty)| {
                        Some((name.clone(), self.erase_memoized(ty, model, memo, active)?))
                    })
                    .collect::<Option<Vec<_>>>()?
                    .into(),
                nullable: *nullable,
            })),
            _ => None,
        }
    }

    fn erase_function_shape<'a>(
        &self,
        shape: &'a std::sync::Arc<super::FunctionTypeShape>,
        model: &SemanticModel<'_>,
        memo: &mut FxHashMap<BorrowedIdentity<'a, ResolvedType>, Option<ResolvedType>>,
        active: &mut FxHashSet<DeclarationIdentity>,
    ) -> Option<std::sync::Arc<super::FunctionTypeShape>> {
        let mut parameters = None;
        for (index, parameter) in shape.type_parameters.iter().enumerate() {
            let erased = self.erase_memoized(parameter, model, memo, active)?;
            if erased != *parameter {
                parameters.get_or_insert_with(|| shape.type_parameters.to_vec())[index] = erased;
            }
        }
        Some(match parameters {
            Some(parameters) => std::sync::Arc::new(super::FunctionTypeShape {
                required_named: shape.required_named.clone(),
                type_parameters: parameters.into(),
            }),
            None => shape.clone(),
        })
    }
}

/// `Never`, or a type parameter bounded by it.
fn is_bottom(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType {
            kind: ResolvedTypeKind::Never,
        } => true,
        ResolvedType {
            kind:
                ResolvedTypeKind::TypeParameter {
                    bound,
                    nullable: false,
                    ..
                }
                | ResolvedTypeKind::NonNullTypeParameter { bound, .. },
        } => is_bottom(bound),
        _ => false,
    }
}

/// A type parameter without an explicit bound.
fn unbounded(bound: &ResolvedType) -> bool {
    match bound {
        ResolvedType {
            kind: ResolvedTypeKind::Dynamic,
        } => true,
        ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity,
                    nullable: true,
                    ..
                },
        } => *identity == DeclarationIdentity::OBJECT,
        _ => false,
    }
}

/// The analyzer's `promoteToNonNull`.
fn promote_non_null(ty: &ResolvedType) -> ResolvedType {
    ty.non_null()
}

/// The analyzer's `isNonNullable`.
pub(super) fn non_nullable(ty: &ResolvedType) -> Truth {
    match ty {
        ResolvedType {
            kind: ResolvedTypeKind::Unknown,
        } => Truth::Unknown,
        ResolvedType {
            kind: ResolvedTypeKind::Never,
        } => Truth::Yes,
        ResolvedType {
            kind: ResolvedTypeKind::Dynamic | ResolvedTypeKind::Void | ResolvedTypeKind::Null,
        } => Truth::No,
        ty if ty.nullable() => Truth::No,
        ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { bound, .. },
        } => non_nullable(bound),
        ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    extension_type: true,
                    ..
                },
        } => Truth::Unknown,
        ty => match future_or_argument(ty) {
            Some(argument) => non_nullable(argument),
            None => Truth::Yes,
        },
    }
}

/// Whether `Null` is a subtype of `ty`.
fn accepts_null(ty: &ResolvedType) -> Truth {
    match ty {
        ResolvedType {
            kind: ResolvedTypeKind::Unknown,
        } => Truth::Unknown,
        ResolvedType {
            kind: ResolvedTypeKind::Dynamic | ResolvedTypeKind::Void | ResolvedTypeKind::Null,
        } => Truth::Yes,
        ty if ty.nullable() => Truth::Yes,
        ty => match future_or_argument(ty) {
            Some(argument) => accepts_null(argument),
            None => Truth::No,
        },
    }
}

fn future_or_argument(ty: &ResolvedType) -> Option<&ResolvedType> {
    match ty {
        ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity,
                    arguments,
                    ..
                },
        } if identity.is_sdk("dart:async", "FutureOr") => arguments.first(),
        _ => None,
    }
}

fn future_of(argument: &ResolvedType) -> ResolvedType {
    ResolvedType::new_interface(
        DeclarationIdentity::sdk("dart:async", "Future"),
        vec![argument.clone()].into(),
        false,
        false,
    )
}

fn any(results: impl IntoIterator<Item = Truth>) -> Truth {
    let mut result = Truth::No;
    for truth in results {
        match truth {
            Truth::Yes => return Truth::Yes,
            Truth::Unknown => result = Truth::Unknown,
            Truth::No => {}
        }
    }
    result
}

fn all(results: impl IntoIterator<Item = Truth>) -> Truth {
    let mut result = Truth::Yes;
    for truth in results {
        match truth {
            Truth::No => return Truth::No,
            Truth::Unknown => result = Truth::Unknown,
            Truth::Yes => {}
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::{IdentityIndex, IdentitySource};
    use std::path::Path;

    #[test]
    fn runtime_intersections_keep_open_and_generic_subtypes() {
        super::super::tests::with_model_at(
            Path::new("/project/intersections.dart"),
            "import 'dart:async'; \
             abstract interface class Right {} \
             sealed class Sealed {} \
             final class G<T> extends Sealed implements Box<T> {} \
             abstract interface class Box<T> {} \
             final class Closed {} \
             base mixin M implements Closed {} \
             typedef IntBox = Box<int>; \
             typedef IntOrFuture = FutureOr<int>; \
             typedef DoubleOrFuture = FutureOr<double>;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let ty = |name| model.resolve_type_name(&[name]).unwrap();
                for (left, right) in [
                    (ty("IntOrFuture"), model.core_type("num", false)),
                    (ty("DoubleOrFuture"), model.core_type("num", false)),
                    (model.core_type("Symbol", false), ty("Right")),
                    (ty("Sealed"), ty("IntBox")),
                    (ty("Closed"), ty("Right")),
                ] {
                    assert_ne!(
                        signatures.runtime_intersection(&left, &right, model),
                        RuntimeIntersection::Disjoint,
                        "{left:?} and {right:?}"
                    );
                }
            },
        );
    }

    #[test]
    fn runtime_intersection_subtypes_refresh_after_project_update() {
        let path = Path::new("/project/closed.dart");
        let parsed = crate::resolve::parsed_fixture(
            "final class Closed {} abstract interface class Right {}",
        );
        let sources = [crate::IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }];
        let (mut indexes, _) = crate::build_semantic_indexes(&sources, &[], None, None);
        let check = |indexes: &crate::SemanticIndexes,
                     parsed: &std::sync::Arc<crate::ParsedFile>| {
            let model = SemanticModel::new(path, parsed, &indexes.identities, Some(&indexes.types));
            let closed = model.resolve_type_name(&["Closed"]).unwrap();
            let right = model.resolve_type_name(&["Right"]).unwrap();
            indexes
                .signatures
                .runtime_intersection(&closed, &right, &model)
        };
        assert_eq!(check(&indexes, &parsed), RuntimeIntersection::Disjoint);
        let updated = crate::resolve::parsed_fixture(
            "final class Closed {} abstract interface class Right {} \
             final class Child extends Closed implements Right {}",
        );
        crate::update_semantic_indexes(
            &mut indexes,
            &[crate::IdentitySource {
                path,
                parsed: &updated,
                has_parse_errors: false,
            }],
            &[],
            None,
            None,
        );
        assert_ne!(check(&indexes, &updated), RuntimeIntersection::Disjoint);
    }

    #[test]
    fn runtime_intersections_separate_proof_from_concrete_heuristics() {
        use RuntimeIntersection::{Disjoint, PossiblyRelated, Unknown, UnrelatedConcrete};
        super::super::tests::with_model_at(
            Path::new("/project/casts.dart"),
            "abstract class Left {} abstract interface class Right {} \
             class Both implements Left, Right {} mixin M {} \
             class C extends Left with M {} class A {} class B {} \
             final class Closed {} sealed class Sealed {} \
             final class Child extends Sealed implements Right {} \
             base class Open extends Sealed {} \
             enum E { value } extension type Wrapped(int value) {} \
             typedef Ints = List<int>; typedef Strings = List<String>; \
             typedef Rec = (int, String); typedef ShortRec = (int,); \
             typedef Fn = int Function(); typedef OtherFn = String Function();",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let ty = |name| model.resolve_type_name(&[name]).unwrap();
                for (left, right, expected) in [
                    (ty("Left"), ty("Right"), PossiblyRelated),
                    (ty("Left"), ty("M"), PossiblyRelated),
                    (ty("Both"), ty("Right"), PossiblyRelated),
                    (ty("A"), ty("B"), UnrelatedConcrete),
                    (ty("Closed"), ty("Right"), Disjoint),
                    (ty("Sealed"), ty("Right"), PossiblyRelated),
                    (ty("Open"), ty("Right"), PossiblyRelated),
                    (ty("E"), ty("A"), Disjoint),
                    (ty("Ints"), ty("Strings"), UnrelatedConcrete),
                    (ty("Rec"), ty("ShortRec"), Disjoint),
                    (ty("Fn"), ty("OtherFn"), PossiblyRelated),
                    (ty("Fn"), ty("A"), PossiblyRelated),
                    (model.core_type("Object", false), ty("A"), PossiblyRelated),
                    (
                        ty("Left").with_nullable(true),
                        ty("Right").with_nullable(true),
                        PossiblyRelated,
                    ),
                    (ResolvedType::NULL, ty("Left"), Disjoint),
                    (
                        ResolvedType::NULL,
                        ty("Left").with_nullable(true),
                        PossiblyRelated,
                    ),
                    (ResolvedType::DYNAMIC, ty("Closed"), PossiblyRelated),
                    (ResolvedType::UNKNOWN, ty("Closed"), Unknown),
                    (ty("Wrapped"), model.core_type("String", false), Disjoint),
                ] {
                    assert_eq!(
                        signatures.runtime_intersection(&left, &right, model),
                        expected,
                        "{left:?} and {right:?}"
                    );
                    assert_eq!(
                        signatures.runtime_intersection(&right, &left, model),
                        expected,
                        "{right:?} and {left:?}"
                    );
                }
                assert_eq!(
                    signatures.runtime_unrelated(&ty("Left"), &ty("Right"), model),
                    Truth::Yes
                );
            },
        );
    }

    #[test]
    fn future_or_top_subtyping_preserves_inherited_signatures() {
        let source = "import 'dart:async';
            abstract class Base { FutureOr<void> onLoad(); }
            mixin M on Base {}
            class Child extends Base with M {
              @override
              Future<void> onLoad() async {}
            }";
        let parsed = crate::resolve::parsed_fixture(source);
        let path = Path::new("/project/future_or.dart");
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
        let signatures = SignatureIndex::from_program(parsed.program(), &model);
        let base = ResolvedType::interface_identity(model.resolve_name("Base").unwrap());
        let future_or_void = signatures
            .instantiated_override_return(&base, "onLoad", false, &model)
            .unwrap();
        let future_void = future_of(&ResolvedType::VOID);
        let int = model.core_type("int", false);
        let string = model.core_type("String", false);
        let future_int = future_of(&int);
        let future_or_int = ResolvedType::from_kind(ResolvedTypeKind::Interface {
            occurrence: super::super::FunctionTypeAlias::default(),
            identity: DeclarationIdentity::sdk("dart:async", "FutureOr"),
            arguments: vec![int.clone()].into(),
            nullable: false,
            extension_type: false,
        });
        let object = model.core_type("Object", false);
        let nullable_object = model.core_type("Object", true);
        for (sub, sup, expected) in [
            (&future_or_void, &future_or_void, Truth::Yes),
            (&ResolvedType::VOID, &future_or_void, Truth::Yes),
            (&ResolvedType::DYNAMIC, &future_or_void, Truth::Yes),
            (&nullable_object, &future_or_void, Truth::Yes),
            (&future_or_void, &nullable_object, Truth::Yes),
            (&future_void, &future_or_void, Truth::Yes),
            (&future_or_void, &future_void, Truth::No),
            (&future_or_void, &object, Truth::No),
            (&future_or_void, &int, Truth::No),
            (&int, &future_or_void, Truth::Yes),
            (&future_int, &future_void, Truth::Yes),
            (&future_or_int, &future_or_int, Truth::Yes),
            (&future_int, &future_or_int, Truth::Yes),
            (&int, &future_or_int, Truth::Yes),
            (&ResolvedType::VOID, &future_or_int, Truth::No),
            (&ResolvedType::DYNAMIC, &future_or_int, Truth::No),
            (&future_void, &future_or_int, Truth::No),
            (&future_or_int, &string, Truth::No),
            (&string, &future_or_int, Truth::No),
        ] {
            assert_eq!(
                signatures.relation(sub, super::super::TypeRelation::SubtypeOf(sup), &model),
                expected,
                "{sub:?} <: {sup:?}"
            );
        }
        let child = model.resolve_name("Child").unwrap();
        let mut lookup = super::super::inherited::OverrideInferenceLookup::default();
        let (_, _, inherited_return) = signatures
            .override_inference_signature(&child, "onLoad", false, &model, &mut lookup)
            .unwrap();
        assert_eq!(inherited_return, future_or_void);
    }
}
