use super::*;
use crate::resolve::semantic::ResolvedTypeKind;

impl SignatureIndex {
    pub fn declaration(&self, identity: &DeclarationIdentity) -> Option<&DeclarationFacts> {
        self.type_facts.get(identity)?.declaration.as_ref()
    }

    pub fn declaration_identities(&self) -> impl Iterator<Item = &DeclarationIdentity> {
        self.type_facts
            .local
            .iter()
            .filter_map(|(identity, facts)| facts.declaration.as_ref().map(|_| identity))
    }

    /// Type of a closure parameter or return that `context` does not supply:
    /// Dart's `dynamic` without a context or under a resolved non-function
    /// one, but `Unknown` when the context exists and is unresolved.
    pub fn uncontextual_closure_type(&self, context: Option<&ResolvedType>) -> ResolvedType {
        match context {
            Some(
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown,
                }
                | ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { .. },
                }
                | ResolvedType {
                    kind: ResolvedTypeKind::NonNullTypeParameter { .. },
                }
                | ResolvedType {
                    kind: ResolvedTypeKind::Function { .. },
                },
            ) => ResolvedType::UNKNOWN,
            Some(ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            }) if identity.is_sdk("dart:async", "FutureOr")
                || (!matches!(identity, DeclarationIdentity::Sdk { .. })
                    && self.declaration(identity).is_none()) =>
            {
                ResolvedType::UNKNOWN
            }
            _ => ResolvedType::DYNAMIC,
        }
    }

    /// The number of type parameters an indexed class, mixin, enum, or
    /// extension type declares (project, loaded external, or curated fallback).
    pub fn type_parameter_count(&self, identity: &DeclarationIdentity) -> Option<usize> {
        self.type_parameters(identity)
            .map(|parameters| parameters.len())
    }

    /// The declaration kind (class, mixin, enum, ...) of the indexed type
    /// `identity`, project or external; `None` when unindexed.
    pub fn type_kind(&self, identity: &DeclarationIdentity) -> Option<TypeKind> {
        self.type_facts.get(identity)?.kind
    }

    pub fn constant_initializer(&self, identity: &DeclarationIdentity) -> Option<Expr<'_>> {
        self.constants.get(identity).map(crate::NodeRef::expr)
    }

    pub fn constant_type(&self, identity: &DeclarationIdentity) -> Option<&DeclarationIdentity> {
        self.constant_types.get(identity)
    }

    /// Whether `name` is a value of the project enum `identity`.
    pub fn is_enum_value(&self, identity: &DeclarationIdentity, name: &str) -> bool {
        self.type_facts
            .get(identity)
            .and_then(|facts| facts.enum_values.as_ref())
            .is_some_and(|values| values.iter().any(|value| value == name))
    }

    /// Whether `identity` is a project enum.
    pub fn is_enum(&self, identity: &DeclarationIdentity) -> bool {
        self.enum_value_names(identity).is_some()
    }

    /// Names of the declared values of an indexed enum, in declaration order.
    pub fn enum_value_names(&self, identity: &DeclarationIdentity) -> Option<&[String]> {
        self.type_facts.get(identity)?.enum_values.as_deref()
    }

    /// The explicit `extends` type of an indexed class or mixin class.
    pub fn superclass(&self, identity: &DeclarationIdentity) -> Option<&ResolvedType> {
        self.type_facts.get(identity)?.superclass.as_ref()
    }

    /// The body of the project `const` constructor `identity.name`.
    pub fn const_constructor(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&ConstConstructorBody> {
        self.type_facts.get(identity)?.const_constructors.get(name)
    }

    /// Instance field initializers of a project class with a `const`
    /// constructor.
    pub fn const_field_initializers(
        &self,
        identity: &DeclarationIdentity,
    ) -> impl Iterator<Item = (&str, Expr<'_>)> {
        self.type_facts
            .get(identity)
            .into_iter()
            .flat_map(|facts| &facts.const_field_initializers)
            .map(|(name, initializer)| (name.as_str(), initializer.expr()))
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

    pub(super) fn declaration_subtype(
        &self,
        ty: &ResolvedType,
        target: &DeclarationIdentity,
        model: &SemanticModel<'_>,
    ) -> Truth {
        match ty {
            ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { bound, .. },
            }
            | ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { bound, .. },
            } => self.declaration_subtype(bound, target, model),
            ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } if !self.has_cyclic_supertypes(identity) => {
                let result = self.ancestor_matches(
                    identity,
                    |identity| {
                        self.type_facts
                            .get(identity)
                            .and_then(|facts| facts.supertypes.as_ref())
                            .map(|(_, supertypes)| supertypes.as_slice())
                    },
                    &|identity| identity.matches(target),
                    |identity, supertypes| {
                        // Without SDK sources enum facts omit the implicit
                        // Enum edge, so their recorded list is incomplete.
                        self.type_kind(identity) != Some(TypeKind::Enum)
                            || supertypes
                                .iter()
                                .any(|ty| ty.interface("dart:core", "Enum"))
                    },
                );
                if result == Truth::Unknown
                    && self.instantiated_identity(ty, target, model).is_some()
                {
                    // Malformed hierarchies can contain type-parameter edges
                    // that require substitution rather than identity matching.
                    Truth::Yes
                } else {
                    result
                }
            }
            _ => Truth::Unknown,
        }
    }

    /// The static read type of a top-level variable or getter.
    pub fn value_type(&self, identity: &DeclarationIdentity) -> Option<&ResolvedType> {
        index_fixpoint::record_inference_read(index_fixpoint::InferenceInputRef::Value(identity));
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

    pub(super) fn is_cyclic_value(&self, identity: &DeclarationIdentity) -> bool {
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
        index_fixpoint::record_inference_read(index_fixpoint::InferenceInputRef::Constructor(
            identity, name,
        ));
        self.type_facts.get(identity)?.constructors.get(name)
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
                && self.declaration(identity).is_some_and(|facts| {
                    index_fixpoint::record_inference_read(
                        index_fixpoint::InferenceInputRef::Constructors(identity),
                    );
                    facts.constructors.is_empty()
                }))
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
            AstChildren::empty(),
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
        let facts = self.type_facts.get(identity)?;
        facts
            .supertypes
            .as_ref()
            .map(|(parameters, _)| parameters.as_slice())
            .or(facts.fallback_type_parameters.as_deref())
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
        ResolvedType::from_kind(ResolvedTypeKind::Interface {
            occurrence: Default::default(),
            identity,
            arguments: parameters
                .iter()
                .map(|id| {
                    ResolvedType::from_kind(ResolvedTypeKind::TypeParameter {
                        id: id.clone(),
                        bound: Arc::new(
                            self.owner_defaults
                                .get(id)
                                .cloned()
                                .unwrap_or_else(|| model.core_type("Object", true)),
                        ),
                        nullable: false,
                    })
                })
                .collect(),
            nullable: false,
            extension_type,
        })
    }

    /// The solution for `id` when only a top-type upper constraint
    /// (`candidate`) bounds it: a declared bound joins that constraint, so the
    /// solution is their greatest lower bound, the bound unless the bound is
    /// the more top type. `Object?` thus instantiates `T extends Object?` under
    /// a `dynamic` context. `None` when `candidate` is not a top type, the
    /// declaration omits the bound (which adds no constraint), or the bound
    /// depends on other `parameters` being solved.
    pub(super) fn declared_bound_meet(
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
        index_fixpoint::record_inference_read(index_fixpoint::InferenceInputRef::Constructors(
            identity,
        ));
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
    ) -> Option<(&ResolvedSignature, FxHashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity,
                    arguments,
                    ..
                },
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

    pub(super) fn instance_member(
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
        let (_, supertypes) = self.type_facts.get(identity)?.supertypes.as_ref()?;
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
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = supertype
        else {
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
            && self.type_kind(identity).is_none()
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
        if let Some((_, supertypes)) = self
            .type_facts
            .get(identity)
            .and_then(|facts| facts.supertypes.as_ref())
        {
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
        let Some((_, supertypes)) = self
            .type_facts
            .get(identity)
            .and_then(|facts| facts.supertypes.as_ref())
        else {
            return Truth::Unknown;
        };
        let mut visited = HashSet::new();
        let mut stack: Vec<&ResolvedType> = supertypes.iter().collect();
        let mut complete = true;
        let receiver = ResolvedType::interface_identity(identity.clone());
        while let Some(supertype) = stack.pop() {
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } = supertype
            else {
                complete = false;
                continue;
            };
            if *identity == DeclarationIdentity::OBJECT || !visited.insert(identity) {
                continue;
            }
            if let DeclarationIdentity::Sdk { library, name } = identity
                && self.type_kind(identity).is_none()
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
            } else if self.type_kind(identity) != Some(TypeKind::Mixin)
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
                                        ResolvedType {
                                            kind: ResolvedTypeKind::Interface { identity, .. },
                                        } => self.type_kind(&identity) != Some(TypeKind::Mixin),
                                        _ => true,
                                    })
                            })
                        })
                    })
                })
            {
                return Truth::Yes;
            }
            match self
                .type_facts
                .get(identity)
                .and_then(|facts| facts.supertypes.as_ref())
            {
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
        let facts = self.type_facts.get(identity)?;
        facts.kind?;
        let declared = facts.restriction;
        if declared != Some(Sealed) {
            return Some(declared);
        }
        if !visiting.insert(identity) {
            return None;
        }
        let (_, supertypes) = facts.supertypes.as_ref()?;
        let mut restriction_of = |ty: &'a ResolvedType| match ty {
            ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } => self.effective_restriction_inner(identity, visiting),
            _ => None,
        };
        // The trailing implicit `Object` is no declared interface.
        let mut interfaces = supertypes.iter();
        interfaces.next_back();
        for _ in 0..facts.superclass_edge_count() {
            interfaces.next()?;
        }
        let superclass = match &facts.superclass {
            Some(superclass) => restriction_of(superclass)?,
            None => None,
        };
        let mixins = facts
            .mixin_applications
            .iter()
            .map(|mixin| {
                interfaces.next()?;
                restriction_of(mixin)
            })
            .collect::<Option<Vec<_>>>()?;
        let interfaces = interfaces
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
            match self
                .type_facts
                .get(&current)
                .and_then(|facts| facts.immutable)
            {
                Some(Truth::Yes) => return Truth::Yes,
                Some(Truth::No) => {}
                Some(Truth::Unknown) => complete = false,
                None => {
                    complete = false;
                    continue;
                }
            }
            let next: Vec<&ResolvedType> = if superclass_only {
                self.superclass(&current).into_iter().collect()
            } else {
                self.type_facts
                    .get(&current)
                    .and_then(|facts| facts.supertypes.as_ref())
                    .map(|(_, supertypes)| supertypes.iter().collect())
                    .unwrap_or_default()
            };
            for supertype in next {
                match supertype {
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface { identity, .. },
                    } => stack.push(identity.clone()),
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
        for supertype in self.type_facts.get(identity)?.member_supertypes.as_ref()? {
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { identity: next, .. },
            } = supertype
            else {
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

    pub(super) fn concrete_member_in_chain(
        &self,
        owner: &DeclarationIdentity,
        identity: &DeclarationIdentity,
        name: &str,
        filter: impl Fn(&MemberFacts) -> bool + Copy,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> Option<Option<(&MemberFacts, &ResolvedType)>> {
        for supertype in self.type_facts.get(identity)?.member_supertypes.as_ref()? {
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { identity: next, .. },
            } = supertype
            else {
                return None;
            };
            if *next == DeclarationIdentity::OBJECT || !visited.insert(next.clone()) {
                continue;
            }
            // Curated SDK entries describe only some members; only SDK
            // declarations indexed from loaded sources are searched.
            if matches!(next, DeclarationIdentity::Sdk { .. }) && self.type_kind(next).is_none() {
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
            && self.type_kind(identity).is_none()
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
        let (_, supertypes) = self.type_facts.get(identity)?.supertypes.as_ref()?;
        for supertype in supertypes {
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { identity: next, .. },
            } = supertype
            else {
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
    pub(super) fn declared_member(
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
    pub(super) fn extension_access_name<'s>(
        &'s self,
        name: &'s str,
        write: bool,
    ) -> Option<&'s str> {
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
    ) -> Option<(&ExtensionFacts, FxHashMap<TypeParameterId, ResolvedType>)> {
        if matches!(
            receiver,
            ResolvedType {
                kind: ResolvedTypeKind::Dynamic
            } | ResolvedType {
                kind: ResolvedTypeKind::Unknown
            }
        ) {
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
                    (
                        _,
                        ResolvedType {
                            kind: ResolvedTypeKind::Dynamic,
                        },
                    ) => on_type != other_on_type,
                    _ => match (
                        self.resolved_subtype(on_type, other_on_type, model),
                        self.resolved_subtype(other_on_type, on_type, model),
                    ) {
                        (Truth::Yes, Truth::No) => true,
                        (Truth::Yes, Truth::Yes) => {
                            let candidate_bound =
                                self.extension_on_type_at_bounds(candidate, model);
                            let other_bound = self.extension_on_type_at_bounds(other, model);
                            self.resolved_subtype(&candidate_bound, &other_bound, model)
                                == Truth::Yes
                                && self.resolved_subtype(&other_bound, &candidate_bound, model)
                                    == Truth::No
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
    ) -> FxHashMap<TypeParameterId, ResolvedType> {
        let mut inferred =
            FxHashMap::with_capacity_and_hasher(extension.parameters.len(), Default::default());
        let mut upper = FxHashMap::default();
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
                        return ResolvedType::UNKNOWN;
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

    pub(super) fn normalized_raw_type<'t>(
        &self,
        ty: Cow<'t, ResolvedType>,
    ) -> Cow<'t, ResolvedType> {
        if let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity,
                    arguments,
                    ..
                },
        } = ty.as_ref()
            && arguments.is_empty()
            && self
                .type_facts
                .get(identity)
                .and_then(|facts| facts.supertypes.as_ref())
                .is_some_and(|(parameters, _)| !parameters.is_empty())
        {
            Cow::Owned(self.with_raw_arguments(ty.as_ref()))
        } else {
            ty
        }
    }

    /// Infer the on-type before checking both the receiver and parameter bounds.
    /// A failed constraint is rejection; missing semantic facts remain unknown.
    pub(super) fn instantiated_extension_on_type<'e>(
        &self,
        receiver: &ResolvedType,
        extension: &'e ExtensionFacts,
        model: &SemanticModel<'_>,
    ) -> (
        Cow<'e, ResolvedType>,
        Truth,
        FxHashMap<TypeParameterId, ResolvedType>,
    ) {
        let substitutions = self.infer_extension_substitutions(receiver, extension, model);
        // A missing argument fits a universal top bound, unless another
        // parameter's bound depends on its actual (still unknown) identity.
        // Proof substitutions must never escape into specificity or signatures.
        let mut proof = Cow::Borrowed(&substitutions);
        for (id, argument) in &substitutions {
            if matches!(
                argument,
                ResolvedType {
                    kind: ResolvedTypeKind::Unknown
                }
            ) && matches!(
                extension.parameter_bounds.get(id),
                Some(ResolvedType {
                    kind: ResolvedTypeKind::Dynamic
                })
            ) && !extension
                .parameter_bounds
                .iter()
                .any(|(other, bound)| other != id && mentions_any(bound, std::slice::from_ref(id)))
            {
                proof.to_mut().insert(id.clone(), ResolvedType::DYNAMIC);
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
                || matches!(
                    extension.on_type,
                    ResolvedType {
                        kind: ResolvedTypeKind::TypeParameter { .. }
                    }
                )) {
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
    pub(super) fn resolved_instance_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        if !receiver.has_instance_members() {
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
    pub(super) fn resolved_owner_through(
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
            if !receiver.has_instance_members() {
                return None;
            }
            if let ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { bound, .. },
            } = receiver
            {
                return visit(index, bound, name, access, interfaces, model, visited);
            }
            if let ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { bound, .. },
            } = receiver
            {
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
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        occurrence,
                        identity,
                        arguments,
                        ..
                    },
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
            let (parameters, supertypes) = index.type_facts.get(identity)?.supertypes.as_ref()?;
            let substitutions = index.owner_substitutions(parameters, arguments, Some(occurrence));
            let supertypes = if interfaces {
                supertypes
            } else {
                index.type_facts.get(identity)?.member_supertypes.as_ref()?
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
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = receiver
        else {
            return None;
        };
        let access = if write {
            InstanceAccess::Write
        } else {
            InstanceAccess::Read
        };
        if self.type_kind(identity) == Some(TypeKind::Mixin) {
            let facts = self.type_facts.get(identity)?;
            let (_, supertypes) = facts.supertypes.as_ref()?;
            let constraints = facts.mixin_constraints.as_ref()?;
            return constraints
                .iter()
                .find_map(|constraint| {
                    self.resolved_owner_through(constraint, name, access, false, model)
                })
                .or_else(|| {
                    self.has_instance_declaration(&DeclarationIdentity::OBJECT, name, access)
                        .then(|| model.core_type("Object", false))
                })
                .or_else(|| {
                    supertypes.iter().find_map(|constraint| {
                        self.resolved_owner_through(constraint, name, access, true, model)
                    })
                })
                .and_then(|owner| match owner {
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface { identity, .. },
                    } => Some(identity),
                    _ => None,
                });
        }
        self.type_facts
            .get(identity)?
            .member_supertypes
            .as_ref()?
            .iter()
            .find_map(|supertype| {
                match self.resolved_owner_through(supertype, name, access, false, model)? {
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface { identity, .. },
                    } => Some(identity),
                    _ => None,
                }
            })
            .or_else(|| {
                self.has_instance_declaration(&DeclarationIdentity::OBJECT, name, access)
                    .then_some(DeclarationIdentity::OBJECT)
            })
            .or_else(|| {
                self.type_facts
                    .get(identity)?
                    .supertypes
                    .as_ref()?
                    .1
                    .iter()
                    .find_map(|supertype| {
                        match self.resolved_owner_through(supertype, name, access, true, model)? {
                            ResolvedType {
                                kind: ResolvedTypeKind::Interface { identity, .. },
                            } => Some(identity),
                            _ => None,
                        }
                    })
            })
    }

    pub(super) fn resolved_access_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        match self.resolved_instance_owner(receiver, name, access, model)? {
            ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            } => Some(identity),
            _ => None,
        }
    }

    pub fn resolved_member_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<DeclarationIdentity> {
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = self.resolved_instance_owner(receiver, name, InstanceAccess::Invoke, model)?
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
        let Some(ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        }) = self.resolved_instance_owner(receiver, name, InstanceAccess::Read, model)
        else {
            return false;
        };
        if !self
            .member_entry(&identity, name)
            .is_some_and(|entry| entry.promotable_field)
        {
            return false;
        }
        let DeclarationIdentity::Library { library, .. } = identity else {
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
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = receiver
        else {
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
        self.type_facts
            .get(identity)?
            .supertypes
            .as_ref()?
            .1
            .iter()
            .find_map(|supertype| self.interface_member_facts(supertype, name, model, visited))
    }

    pub(super) fn owner_substitutions(
        &self,
        parameters: &[TypeParameterId],
        arguments: &[ResolvedType],
        occurrence: Option<&FunctionTypeAlias>,
    ) -> FxHashMap<TypeParameterId, ResolvedType> {
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
                        .unwrap_or(ResolvedType::DYNAMIC),
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
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    occurrence,
                    identity,
                    arguments,
                    ..
                },
        } = this_type
        else {
            return None;
        };
        let (parameters, supertypes) = self.type_facts.get(identity)?.supertypes.as_ref()?;
        let substitutions = self.owner_substitutions(parameters, arguments, Some(occurrence));
        let supertypes = if self.type_kind(identity) == Some(TypeKind::Mixin) {
            supertypes
        } else {
            self.type_facts.get(identity)?.member_supertypes.as_ref()?
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
    ) -> Option<(&ResolvedSignature, FxHashMap<TypeParameterId, ResolvedType>)> {
        let Some(owner) =
            self.resolved_instance_owner(receiver, name, InstanceAccess::Invoke, model)
        else {
            return self.resolved_extension_method(receiver, name, model);
        };
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    occurrence,
                    identity,
                    arguments,
                    ..
                },
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
    ) -> Option<(&ExtensionFacts, FxHashMap<TypeParameterId, ResolvedType>)> {
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
    ) -> Option<(&ResolvedSignature, FxHashMap<TypeParameterId, ResolvedType>)> {
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
    ) -> Option<(&ResolvedSignature, FxHashMap<TypeParameterId, ResolvedType>)> {
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
    ) -> Option<(ResolvedType, FxHashMap<TypeParameterId, ResolvedType>)> {
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
        FxHashMap<TypeParameterId, ResolvedType>,
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
    ) -> Option<(ResolvedType, FxHashMap<TypeParameterId, ResolvedType>)> {
        let owner = self.resolved_instance_owner(receiver, name, InstanceAccess::Read, model)?;
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    occurrence,
                    identity,
                    arguments,
                    ..
                },
        } = owner
        else {
            return None;
        };
        let field = self.member_entry(&identity, name)?.read.as_ref()?;
        let (parameters, _) = self.type_facts.get(&identity)?.supertypes.as_ref()?;
        let substitutions = self.owner_substitutions(parameters, &arguments, Some(&occurrence));
        Some((field.clone(), substitutions))
    }

    pub fn resolved_writable_field(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(ResolvedType, FxHashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    occurrence,
                    identity,
                    arguments,
                    ..
                },
        } = self.resolved_instance_owner(receiver, name, InstanceAccess::Write, model)?
        else {
            return None;
        };
        let field = self.member_entry(&identity, name)?.write.as_ref()?;
        let (parameters, _) = self.type_facts.get(&identity)?.supertypes.as_ref()?;
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
