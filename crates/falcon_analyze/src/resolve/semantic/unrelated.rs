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

use rustc_hash::{FxHashMap, FxHashSet};
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};

use super::{DeclarationIdentity, MemberFacts, ResolvedType, SemanticModel, SignatureIndex, Truth};
use crate::resolve::{MemberKind, TypeKind};

const PROTOBUF_ENUM_URI: &str = "package:protobuf/src/protobuf/protobuf_enum.dart";

type RelationMemo = FxHashMap<(ResolvedType, ResolvedType), Truth>;

fn memoized_relation(
    left: &ResolvedType,
    right: &ResolvedType,
    memo: &mut RelationMemo,
    compute: impl FnOnce(&mut RelationMemo) -> Truth,
) -> Truth {
    let has_children = |ty: &ResolvedType| match ty {
        ResolvedType::Interface { arguments, .. } => !arguments.is_empty(),
        ResolvedType::Record {
            positional, named, ..
        } => !positional.is_empty() || !named.is_empty(),
        ResolvedType::TypeParameter { .. }
        | ResolvedType::NonNullTypeParameter { .. }
        | ResolvedType::Function { .. } => true,
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
    /// Erases extension types to their instantiated runtime representation.
    /// Unknown representations or cyclic declarations remain unknown.
    pub fn extension_type_erasure(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.erase(ty, model)
    }

    /// The linter's `typesAreUnrelated`: `Yes` when the types are proven
    /// unrelated, `No` when they might be related, `Unknown` when falcon cannot
    /// tell.
    pub fn types_are_unrelated(
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
        use ResolvedType as T;
        if matches!(left, T::Unknown) || matches!(right, T::Unknown) {
            return Truth::Unknown;
        }
        if is_bottom(left) || is_bottom(right) || *left == T::Dynamic || *right == T::Dynamic {
            return Truth::No;
        }
        if *left == T::Null || *right == T::Null {
            let other = if *left == T::Null { right } else { left };
            if non_nullable(other) == Truth::Yes {
                return Truth::Yes;
            }
        }
        let left = promote_non_null(left);
        let right = promote_non_null(right);
        if left == right {
            return Truth::No;
        }
        let forward = self.subtype(&left, &right, model);
        let backward = self.subtype(&right, &left, model);
        if forward == Truth::Yes || backward == Truth::Yes {
            return Truth::No;
        }
        let neither_subtype = if forward == Truth::No && backward == Truth::No {
            Truth::Yes
        } else {
            Truth::Unknown
        };
        let unrelated = match (&left, &right) {
            (T::Interface { .. }, T::Interface { .. }) => {
                self.interfaces_unrelated(&left, &right, model, memo)
            }
            (
                T::TypeParameter { bound: left, .. } | T::NonNullTypeParameter { bound: left, .. },
                T::TypeParameter { bound: right, .. }
                | T::NonNullTypeParameter { bound: right, .. },
            ) => {
                if unbounded(left) || unbounded(right) {
                    Truth::No
                } else {
                    self.erased_unrelated_memoized(left, right, model, memo)
                }
            }
            (T::Function { .. }, other) | (other, T::Function { .. }) => {
                self.unrelated_to_function(other)
            }
            (T::Record { .. }, _) | (_, T::Record { .. }) => Truth::Yes,
            _ => Truth::No,
        };
        match (neither_subtype, unrelated) {
            (_, Truth::No) => Truth::No,
            (Truth::Yes, unrelated) => unrelated,
            _ => Truth::Unknown,
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
            ResolvedType::Interface {
                identity: left_identity,
                arguments: left_arguments,
                ..
            },
            ResolvedType::Interface {
                identity: right_identity,
                arguments: right_arguments,
                ..
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
                && self.supertypes.contains_key(identity)
        }) {
            // Source-free SDK declarations retain the engine's existing
            // fallback relation; its supertype graph is not an extends table.
            // A project TypeIndex may be incomplete because of imports even
            // when this index has complete SDK edges. Use those identity
            // edges when the name-based fallback cannot prove either result.
            return match self.unrelated(left, right, model) {
                Truth::Unknown => match (
                    self.subtype(left, right, model),
                    self.subtype(right, left, model),
                ) {
                    (Truth::No, Truth::No) => Truth::Yes,
                    (Truth::Yes, _) | (_, Truth::Yes) => Truth::No,
                    _ => Truth::Unknown,
                },
                result => result,
            };
        }
        let member_supertypes = |identity| {
            self.member_supertypes
                .get(identity)
                .map(|types| match types.as_slice() {
                    [types @ .., ResolvedType::Interface { identity, .. }]
                        if *identity == DeclarationIdentity::OBJECT =>
                    {
                        types
                    }
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
        let ResolvedType::Interface { identity, .. } = superclass else {
            return Truth::Unknown;
        };
        let protobuf_enum = identity.name() == "ProtobufEnum"
            && (identity.is_package("protobuf", "ProtobufEnum")
                || model.declaration_uri(identity) == Some(PROTOBUF_ENUM_URI));
        (protobuf_enum || *identity == DeclarationIdentity::OBJECT).into()
    }

    /// The linter's `_isTypeUnrelatedToFunctionType`.
    fn unrelated_to_function(&self, ty: &ResolvedType) -> Truth {
        let ResolvedType::Interface { identity, .. } = ty else {
            return (!matches!(ty, ResolvedType::Function { .. })).into();
        };
        match self.type_kind(identity) {
            None if matches!(identity, DeclarationIdentity::Sdk { .. })
                && self.supertypes.contains_key(identity) =>
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
    pub fn subtype(
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
        use ResolvedType as T;
        match (sub, sup) {
            (T::Unknown, T::Dynamic | T::Void) if prove_unknown_top => return Truth::Yes,
            (T::Unknown, _) | (_, T::Unknown) => return Truth::Unknown,
            (_, T::Dynamic | T::Void) | (T::Never, _) => return Truth::Yes,
            (
                _,
                T::Interface {
                    identity,
                    nullable: true,
                    ..
                },
            ) if *identity == DeclarationIdentity::OBJECT => {
                return Truth::Yes;
            }
            (T::Dynamic | T::Void, _) => {
                return self.subtype_memoized(
                    &model.core_type("Object", true),
                    sup,
                    model,
                    prove_unknown_top,
                    memo,
                );
            }
            (T::Null, _) => return accepts_null(sup),
            _ => {}
        }
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
            (T::TypeParameter { id, .. }, T::TypeParameter { id: target, .. }) if id == target => {
                Truth::Yes
            }
            (
                T::NonNullTypeParameter { id, .. },
                T::TypeParameter { id: target, .. } | T::NonNullTypeParameter { id: target, .. },
            ) if id == target => Truth::Yes,
            (T::NonNullTypeParameter { bound, .. }, _) => {
                self.subtype_memoized(&bound.non_null(), &sup, model, prove_unknown_top, memo)
            }
            (T::TypeParameter { bound, .. }, _) => {
                self.subtype_memoized(bound, &sup, model, prove_unknown_top, memo)
            }
            (_, T::Interface { identity, .. }) if *identity == DeclarationIdentity::OBJECT => {
                Truth::Yes
            }
            (_, T::TypeParameter { .. } | T::NonNullTypeParameter { .. }) => Truth::No,
            (T::Function { .. }, T::Function { .. }) => {
                self.function_subtype(&sub, &sup, model, prove_unknown_top, memo)
            }
            (T::Function { .. }, T::Interface { identity, .. }) => {
                identity.is_sdk("dart:core", "Function").into()
            }
            (T::Record { .. }, T::Interface { identity, .. }) => {
                identity.is_sdk("dart:core", "Record").into()
            }
            (
                T::Record {
                    positional: sub_positional,
                    named: sub_named,
                    ..
                },
                T::Record {
                    positional: sup_positional,
                    named: sup_named,
                    ..
                },
            ) => {
                if sub_positional.len() != sup_positional.len()
                    || !sub_named
                        .iter()
                        .map(|(name, _)| name)
                        .eq(sup_named.iter().map(|(name, _)| name))
                {
                    return Truth::No;
                }
                all(sub_positional
                    .iter()
                    .zip(sup_positional.iter())
                    .chain(
                        sub_named
                            .iter()
                            .zip(sup_named.iter())
                            .map(|((_, sub), (_, sup))| (sub, sup)),
                    )
                    .map(|(sub, sup)| {
                        self.subtype_memoized(sub, sup, model, prove_unknown_top, memo)
                    }))
            }
            (T::Interface { .. }, T::Function { .. } | T::Record { .. })
            | (T::Function { .. } | T::Record { .. }, _) => Truth::No,
            (
                T::Interface {
                    identity: sub_identity,
                    ..
                },
                T::Interface {
                    identity: sup_identity,
                    arguments: sup_arguments,
                    ..
                },
            ) => match self.instantiated_identity(&sub, sup_identity, model) {
                Some(instantiated) => {
                    let arguments = instantiated.arguments();
                    if arguments.len() != sup_arguments.len() {
                        return Truth::Unknown;
                    }
                    all(arguments
                        .iter()
                        .zip(sup_arguments.iter())
                        .map(|(sub, sup)| {
                            self.subtype_memoized(sub, sup, model, prove_unknown_top, memo)
                        }))
                }
                None => match self.identity_implements(
                    sub_identity,
                    &|identity| identity == sup_identity,
                    &mut HashSet::new(),
                ) {
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
            ResolvedType::Function {
                return_type: sub_return,
                positional: sub_positional,
                named: sub_named,
                optional_positional: sub_optional,
                shape: Some(sub_shape),
                ..
            },
            ResolvedType::Function {
                return_type: sup_return,
                positional: sup_positional,
                named: sup_named,
                optional_positional: sup_optional,
                shape: Some(sup_shape),
                ..
            },
        ) = (sub, sup)
        else {
            return Truth::Unknown;
        };
        if sub_positional.len().saturating_sub(*sub_optional)
            > sup_positional.len().saturating_sub(*sup_optional)
            || sub_positional.len() < sup_positional.len()
            || sub_shape.type_parameters.len() != sup_shape.type_parameters.len()
            || sub_shape
                .required_named
                .iter()
                .any(|name| !sup_shape.required_named.contains(name))
            || sup_named
                .iter()
                .any(|(name, _)| !sub_named.iter().any(|(other, _)| name == other))
        {
            return Truth::No;
        }

        // Generic functions are compared after alpha-renaming the target's
        // binders to the source's binders; their bounds must then be equivalent.
        let mut substitutions = HashMap::new();
        for (sub, sup) in sub_shape
            .type_parameters
            .iter()
            .zip(sup_shape.type_parameters.iter())
        {
            let (
                ResolvedType::TypeParameter { id: sub_id, .. },
                ResolvedType::TypeParameter { id: sup_id, .. },
            ) = (sub, sup)
            else {
                return Truth::Unknown;
            };
            if sub_id != sup_id {
                substitutions.insert(sup_id.clone(), sub.clone());
            }
        }
        let substitute = |ty| {
            if substitutions.is_empty() {
                Cow::Borrowed(ty)
            } else {
                Cow::Owned(model.substitute(ty, &substitutions))
            }
        };
        let mut result = Truth::Yes;
        for (sub, sup) in sub_shape
            .type_parameters
            .iter()
            .zip(sup_shape.type_parameters.iter())
        {
            let (
                ResolvedType::TypeParameter {
                    bound: sub_bound, ..
                },
                ResolvedType::TypeParameter {
                    bound: sup_bound, ..
                },
            ) = (sub, sup)
            else {
                return Truth::Unknown;
            };
            let sup_bound = substitute(sup_bound.as_ref());
            result = all([
                result,
                self.subtype_memoized(sub_bound, &sup_bound, model, prove_unknown_top, memo),
                self.subtype_memoized(&sup_bound, sub_bound, model, prove_unknown_top, memo),
            ]);
            if result == Truth::No {
                return result;
            }
        }
        let sup_return = substitute(sup_return.as_ref());
        result = all([
            result,
            self.subtype_memoized(sub_return, &sup_return, model, prove_unknown_top, memo),
        ]);
        if result == Truth::No {
            return result;
        }
        for (sub, sup) in
            sub_positional
                .iter()
                .zip(sup_positional.iter())
                .chain(sup_named.iter().filter_map(|(name, sup)| {
                    sub_named
                        .iter()
                        .find(|(other, _)| name == other)
                        .map(|(_, sub)| (sub, sup))
                }))
        {
            let sup = substitute(sup);
            result = all([
                result,
                self.subtype_memoized(&sup, sub, model, prove_unknown_top, memo),
            ]);
            if result == Truth::No {
                return result;
            }
        }
        result
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
                    ResolvedType::Interface {
                        extension_type: true,
                        ..
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

    fn erase_memoized(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
        memo: &mut FxHashMap<*const ResolvedType, Option<ResolvedType>>,
        active: &mut FxHashSet<DeclarationIdentity>,
    ) -> Option<ResolvedType> {
        let key = ty as *const ResolvedType;
        if let Some(result) = memo.get(&key) {
            return result.clone();
        }
        let result = match ty {
            ResolvedType::Interface {
                identity,
                arguments,
                nullable,
                extension_type: true,
                ..
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
                    let (parameters, _) = self.supertypes.get(identity)?;
                    let substitutions: HashMap<_, _> =
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
            ResolvedType::Interface {
                occurrence,
                identity,
                arguments,
                nullable,
                extension_type: false,
            } => Some(ResolvedType::Interface {
                occurrence: occurrence.clone(),
                identity: identity.clone(),
                arguments: arguments
                    .iter()
                    .map(|ty| self.erase_memoized(ty, model, memo, active))
                    .collect::<Option<Vec<_>>>()?
                    .into(),
                nullable: *nullable,
                extension_type: false,
            }),
            ResolvedType::TypeParameter {
                id,
                bound,
                nullable,
            } => Some(ResolvedType::TypeParameter {
                id: id.clone(),
                bound: std::sync::Arc::new(self.erase_memoized(bound, model, memo, active)?),
                nullable: *nullable,
            }),
            ResolvedType::NonNullTypeParameter { id, bound } => {
                Some(ResolvedType::NonNullTypeParameter {
                    id: id.clone(),
                    bound: std::sync::Arc::new(self.erase_memoized(bound, model, memo, active)?),
                })
            }
            ResolvedType::Function {
                return_type,
                positional,
                named,
                nullable,
                alias,
                optional_positional,
                shape,
            } => Some(ResolvedType::Function {
                optional_positional: *optional_positional,
                shape: match shape {
                    Some(shape) => {
                        let mut parameters = None;
                        for (index, parameter) in shape.type_parameters.iter().enumerate() {
                            let erased = self.erase_memoized(parameter, model, memo, active)?;
                            if erased != *parameter {
                                parameters.get_or_insert_with(|| shape.type_parameters.to_vec())
                                    [index] = erased;
                            }
                        }
                        match parameters {
                            Some(parameters) => {
                                Some(std::sync::Arc::new(super::FunctionTypeShape {
                                    required_named: shape.required_named.clone(),
                                    type_parameters: parameters.into(),
                                }))
                            }
                            None => Some(shape.clone()),
                        }
                    }
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
            }),
            ResolvedType::Record {
                occurrence,
                positional,
                named,
                nullable,
            } => Some(ResolvedType::Record {
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
            }),
            other => return Some(other.clone()),
        };
        memo.insert(key, result.clone());
        result
    }
}

/// `Never`, or a type parameter bounded by it.
fn is_bottom(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Never => true,
        ResolvedType::TypeParameter {
            bound,
            nullable: false,
            ..
        }
        | ResolvedType::NonNullTypeParameter { bound, .. } => is_bottom(bound),
        _ => false,
    }
}

/// A type parameter without an explicit bound.
fn unbounded(bound: &ResolvedType) -> bool {
    match bound {
        ResolvedType::Dynamic => true,
        ResolvedType::Interface {
            identity,
            nullable: true,
            ..
        } => *identity == DeclarationIdentity::OBJECT,
        _ => false,
    }
}

/// The analyzer's `promoteToNonNull`.
fn promote_non_null(ty: &ResolvedType) -> ResolvedType {
    ty.non_null()
}

/// The analyzer's `isNonNullable`.
fn non_nullable(ty: &ResolvedType) -> Truth {
    match ty {
        ResolvedType::Unknown => Truth::Unknown,
        ResolvedType::Never => Truth::Yes,
        ResolvedType::Dynamic | ResolvedType::Void | ResolvedType::Null => Truth::No,
        ty if ty.nullable() => Truth::No,
        ResolvedType::TypeParameter { bound, .. } => non_nullable(bound),
        ty => match future_or_argument(ty) {
            Some(argument) => non_nullable(argument),
            None => Truth::Yes,
        },
    }
}

/// Whether `Null` is a subtype of `ty`.
fn accepts_null(ty: &ResolvedType) -> Truth {
    match ty {
        ResolvedType::Unknown => Truth::Unknown,
        ResolvedType::Dynamic | ResolvedType::Void | ResolvedType::Null => Truth::Yes,
        ty if ty.nullable() => Truth::Yes,
        ty => match future_or_argument(ty) {
            Some(argument) => accepts_null(argument),
            None => Truth::No,
        },
    }
}

fn future_or_argument(ty: &ResolvedType) -> Option<&ResolvedType> {
    match ty {
        ResolvedType::Interface {
            identity,
            arguments,
            ..
        } if identity.is_sdk("dart:async", "FutureOr") => arguments.first(),
        _ => None,
    }
}

fn future_of(argument: &ResolvedType) -> ResolvedType {
    ResolvedType::Interface {
        occurrence: Default::default(),
        identity: DeclarationIdentity::sdk("dart:async", "Future"),
        arguments: vec![argument.clone()].into(),
        nullable: false,
        extension_type: false,
    }
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
    fn future_or_top_subtyping_preserves_inherited_signatures() {
        let source = "import 'dart:async';
            abstract class Base { FutureOr<void> onLoad(); }
            mixin M on Base {}
            class Child extends Base with M {
              @override
              Future<void> onLoad() async {}
            }";
        let falcon_dart_parser::ParseResult { tree, errors, .. } =
            falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = std::sync::Arc::new(crate::ParsedFile {
            source: source.into(),
            tree,
        });
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
        let future_void = future_of(&ResolvedType::Void);
        let int = model.core_type("int", false);
        let string = model.core_type("String", false);
        let future_int = future_of(&int);
        let future_or_int = ResolvedType::Interface {
            occurrence: Default::default(),
            identity: DeclarationIdentity::sdk("dart:async", "FutureOr"),
            arguments: vec![int.clone()].into(),
            nullable: false,
            extension_type: false,
        };
        let object = model.core_type("Object", false);
        let nullable_object = model.core_type("Object", true);
        for (sub, sup, expected) in [
            (&future_or_void, &future_or_void, Truth::Yes),
            (&ResolvedType::Void, &future_or_void, Truth::Yes),
            (&ResolvedType::Dynamic, &future_or_void, Truth::Yes),
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
            (&ResolvedType::Void, &future_or_int, Truth::No),
            (&ResolvedType::Dynamic, &future_or_int, Truth::No),
            (&future_void, &future_or_int, Truth::No),
            (&future_or_int, &string, Truth::No),
            (&string, &future_or_int, Truth::No),
        ] {
            assert_eq!(
                signatures.subtype(sub, sup, &model),
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
