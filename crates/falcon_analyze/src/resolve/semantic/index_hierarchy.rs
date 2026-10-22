use super::*;

impl SignatureIndex {
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
    pub(super) fn has_cyclic_supertypes(&self, identity: &DeclarationIdentity) -> bool {
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

    pub(super) fn cyclic_supertype_identities(&self) -> FxHashSet<DeclarationIdentity> {
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
        self.ancestor_matches(
            identity,
            |identity| {
                self.supertypes
                    .get(identity)
                    .map(|(_, supertypes)| supertypes.as_slice())
            },
            matches,
        )
    }

    /// Whether `identity` or its transitive `extends`/`with` ancestors match.
    pub fn member_supertype_matches(
        &self,
        identity: &DeclarationIdentity,
        matches: &impl Fn(&DeclarationIdentity) -> bool,
    ) -> Truth {
        self.ancestor_matches(
            identity,
            |identity| self.member_supertypes.get(identity).map(Vec::as_slice),
            matches,
        )
    }

    fn ancestor_matches<'s>(
        &'s self,
        identity: &'s DeclarationIdentity,
        edges: impl Fn(&DeclarationIdentity) -> Option<&'s [ResolvedType]>,
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
            match edges(identity) {
                Some(supertypes) => {
                    complete &= !matches!(identity, DeclarationIdentity::Package { .. });
                    for supertype in supertypes {
                        match supertype {
                            ResolvedType::Interface { identity, .. } => stack.push(identity),
                            _ => complete = false,
                        }
                    }
                }
                None => complete &= *identity == DeclarationIdentity::OBJECT,
            }
        }
        if complete { Truth::No } else { Truth::Unknown }
    }

    /// A raw use of a generic interface (no type arguments) denotes its
    /// instantiation to bounds; any other type is returned unchanged.
    pub(super) fn with_raw_arguments(&self, ty: &ResolvedType) -> ResolvedType {
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

    pub(super) fn instantiated_matching(
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

    /// Finds an instantiated supertype by declaration, retaining inherited substitutions.
    pub fn instantiated_supertype_matching(
        &self,
        ty: &ResolvedType,
        target: impl Fn(&DeclarationIdentity) -> bool,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        self.instantiated_matching(ty, &target, model, &mut FxHashSet::default())
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
                    !matches!(identity, DeclarationIdentity::Library { .. })
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
    /// hierarchy, an unknown type argument, a library declaration without
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
                    None if matches!(identity, DeclarationIdentity::Library { .. }) => {
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
    /// recorded (indexed declarations and the curated `SDK_HIERARCHY`);
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
    pub(super) fn identity_implements(
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
}
