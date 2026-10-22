use super::*;

type IndexedProgram<'a> = (Option<FileId>, &'a Arc<crate::ParsedFile>, bool);

impl SignatureIndex {
    pub(crate) fn flow_writes(&self, file: FileId) -> Option<&Arc<super::flow_writes::FlowWrites>> {
        self.flow_writes.get(&Some(file))
    }

    pub(super) fn member_entry(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&MemberEntry> {
        self.member_entries.get(identity)?.get(name)
    }

    pub fn from_program(program: Program<'_>, model: &SemanticModel<'_>) -> Self {
        let mut index = Self::default();
        index.add_sdk_signatures();
        index.add_program(program, model);
        index
    }

    /// Precompute lookup facts once declarations and supertype edges are
    /// complete. Declaration type inference does not change these facts.
    pub(super) fn finish(&mut self) {
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

    pub(super) fn extensions_with_member<'s>(
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

    pub(super) fn all_extensions(&self) -> impl Iterator<Item = &ExtensionFacts> {
        self.extensions
            .iter()
            .chain(self.external.iter().flat_map(|base| base.extensions.iter()))
    }

    pub fn from_project_files(
        files: &[(PathBuf, Arc<crate::ParsedFile>)],
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        Self::build(files, None, identities, types)
    }

    /// Index only the current project, borrowing external signatures and facts.
    /// `external` must be an external-only, non-overlay snapshot.
    pub fn with_external(
        files: &[(PathBuf, Arc<crate::ParsedFile>)],
        identities: &IdentityIndex,
        types: &TypeIndex,
        external: Arc<SignatureIndex>,
    ) -> Self {
        let index = Self::borrowing_external(external);
        let all = files
            .iter()
            .map(|(path, parsed)| (identities.file(path), parsed, true))
            .collect();
        Self::build_programs(index, all, identities, types)
    }

    /// Index project `files` and, when given, the loaded external libraries
    /// (which must also be indexed by `identities`). The curated SDK/package
    /// tables only supply declarations whose sources are not loaded.
    pub fn build(
        files: &[(PathBuf, Arc<crate::ParsedFile>)],
        external: Option<&ExternalLibraries>,
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        let all: Vec<IndexedProgram<'_>> = external
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
                            &file.parsed,
                            false,
                        )
                    })
            })
            .chain(
                files
                    .iter()
                    .map(|(path, parsed)| (identities.file(path), parsed, true)),
            )
            .collect();
        Self::build_programs(Self::default(), all, identities, types)
    }

    fn build_programs(
        mut index: Self,
        all: Vec<IndexedProgram<'_>>,
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        for (file, parsed, analyze_bodies) in &all {
            let model = SemanticModel::from_file(*file, parsed, identities, Some(types));
            index.add_declarations(parsed.program(), &model, *analyze_bodies);
        }
        tracing::debug!("signature declarations built");
        if index.external.is_none() {
            index.add_fallback_signatures();
        }
        index.finish();
        tracing::debug!("signature tables finished");
        let programs: Vec<_> = all
            .iter()
            .map(|(file, parsed, _)| {
                (
                    parsed.program(),
                    SemanticModel::from_file(*file, parsed, identities, Some(types)),
                )
            })
            .collect();
        index.infer_programs(&programs);
        tracing::debug!("signature inference finished");
        index.mark_library_subclasses();
        index.mark_field_forwarders(identities, Some(types));
        index
    }

    /// Seed the curated SDK/package tables for every declaration no indexed
    /// source supplies: without loaded sources they are the fallback; with
    /// them, a declaration's real facts always win as a whole.
    pub(super) fn add_fallback_signatures(&mut self) {
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
        fill(
            &mut self.fallback_type_parameters,
            fallback.fallback_type_parameters,
        );
        fill(&mut self.superclasses, fallback.superclasses);
        fill(&mut self.member_supertypes, fallback.member_supertypes);
        fill(&mut self.declarations, fallback.declarations);
        fill(&mut self.constants, fallback.constants);
        fill(&mut self.constant_types, fallback.constant_types);
    }

    pub(super) fn add_declarations(
        &mut self,
        program: Program<'_>,
        model: &SemanticModel<'_>,
        analyze_bodies: bool,
    ) {
        if analyze_bodies {
            self.flow_writes
                .entry(model.file)
                .or_insert_with(|| Arc::new(super::flow_writes::FlowWrites::program(program)));
        }
        for declaration in program.declarations() {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let kind = match declaration {
                TopLevelDecl::Class(class) => Some((
                    class.name(),
                    TypeKind::Class,
                    class.extends().is_none(),
                    class.modifiers().restriction,
                )),
                TopLevelDecl::ClassTypeAlias(alias) => Some((
                    alias.name(),
                    TypeKind::Class,
                    false,
                    alias.modifiers().restriction,
                )),
                TopLevelDecl::MixinClass(class) => Some((
                    class.name(),
                    TypeKind::MixinClass,
                    class.extends().is_none(),
                    class.is_base().then_some(ClassRestriction::Base),
                )),
                TopLevelDecl::Mixin(mixin) => Some((
                    mixin.name(),
                    TypeKind::Mixin,
                    mixin.on_clause().is_empty(),
                    mixin.is_base().then_some(ClassRestriction::Base),
                )),
                TopLevelDecl::Enum(enumeration) => Some((
                    enumeration.name(),
                    TypeKind::Enum,
                    !model.identities.has_sdk_sources(),
                    None,
                )),
                TopLevelDecl::ExtensionType(extension) => {
                    Some((extension.name(), TypeKind::ExtensionType, false, None))
                }
                _ => None,
            };
            if let Some((name, kind, inherited_object, restriction)) = kind
                && let Some(identity) = model.resolve_name(name.name())
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
                        alias.name().name(),
                        alias.annotations(),
                        alias.type_params(),
                        EffectiveMembers::declared(AstChildren::empty()),
                        (None, alias.modifiers().is_abstract),
                        model,
                    );
                    self.add_type(
                        alias.name().name(),
                        alias.type_params(),
                        std::iter::once(alias.superclass())
                            .chain(alias.with_clause())
                            .chain(alias.implements())
                            .collect(),
                        alias
                            .with_clause()
                            .rev()
                            .chain(std::iter::once(alias.superclass()))
                            .collect(),
                        EffectiveMembers::declared(AstChildren::empty()),
                        model,
                    );
                    if let Some(identity) = model.resolve_name(alias.name().name()) {
                        let mut scope = TypeParameterScope::default();
                        scope.push(alias.type_params(), model);
                        self.superclasses.insert(
                            identity.clone(),
                            model.resolve_type_in(alias.superclass(), &scope),
                        );
                        self.mixin_applications
                            .insert(identity, 1..1 + alias.with_clause().len());
                    }
                }
                // Alias parameters are the inference variables of alias
                // constructor calls and tear-offs (`alias_template`).
                TopLevelDecl::TypeAlias(alias) => {
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(alias.type_params(), model);
                    record_bound_declarations(
                        &mut self.bound_declared,
                        &parameters,
                        alias.type_params(),
                    );
                    self.add_declaration_facts(
                        alias.name().name(),
                        alias.annotations(),
                        alias.type_params(),
                        EffectiveMembers::declared(AstChildren::empty()),
                        (None, false),
                        model,
                    );
                }
                TopLevelDecl::Function(function)
                    if matches!(function.kind(), FunctionKind::Function(_)) =>
                {
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(function.type_params(), model);
                    record_bound_declarations(
                        &mut self.bound_declared,
                        &parameters,
                        function.type_params(),
                    );
                    if let Some(identity) = model.resolve_value(function.name().name()) {
                        let annotations = resolve_annotations(function.annotations(), model);
                        if !annotations.is_empty() {
                            self.function_annotations
                                .insert(identity.clone(), annotations);
                        }
                        self.functions.insert(
                            identity,
                            signature(
                                &parameters,
                                &[],
                                function.type_params(),
                                function.params(),
                                function.return_type(),
                                SignatureContext::Function,
                                model,
                            ),
                        );
                    }
                }
                TopLevelDecl::Function(function)
                    if matches!(function.kind(), FunctionKind::Getter) =>
                {
                    if let Some(identity) = model.resolve_value(function.name().name()) {
                        self.value_getters.insert(identity.clone());
                        self.value_types.insert(
                            identity,
                            function
                                .return_type()
                                .map(|ty| model.resolve_type(ty))
                                .unwrap_or(ResolvedType::Dynamic),
                        );
                    }
                }
                TopLevelDecl::Variable(variable) => {
                    for declarator in variable.declarators() {
                        if !model.owns_declaration(declarator.name().name()) {
                            continue;
                        }
                        let Some(identity) = model.resolve_value(declarator.name().name()) else {
                            continue;
                        };
                        self.value_types.insert(
                            identity.clone(),
                            variable
                                .var_type()
                                .map(|ty| model.resolve_type(ty))
                                .unwrap_or_else(|| {
                                    if declarator.initializer().is_none() {
                                        ResolvedType::Dynamic
                                    } else {
                                        ResolvedType::Unknown
                                    }
                                }),
                        );
                        if variable.keyword() == VarKeyword::Const
                            && let Some(initializer) = declarator.initializer()
                        {
                            if let Some(constant_type) = constructed_identity(initializer, model) {
                                self.constant_types.insert(identity.clone(), constant_type);
                            }
                            self.constants
                                .insert(identity, model.node_ref(initializer.syntax()));
                        }
                    }
                }
                TopLevelDecl::Class(class) => {
                    let members = class.element_members();
                    let supertypes = class
                        .extends()
                        .into_iter()
                        .chain(class.with_clause())
                        .chain(class.implements())
                        .collect();
                    let member_supertypes =
                        class.with_clause().rev().chain(class.extends()).collect();
                    self.add_type(
                        class.name().name(),
                        class.type_params(),
                        supertypes,
                        member_supertypes,
                        members.clone(),
                        model,
                    );
                    if !class.with_clause().is_empty()
                        && let Some(identity) = model.resolve_name(class.name().name())
                    {
                        let start = usize::from(class.extends().is_some());
                        self.mixin_applications
                            .insert(identity, start..start + class.with_clause().len());
                    }
                    self.add_declaration_facts(
                        class.name().name(),
                        class.annotations(),
                        class.type_params(),
                        members.clone(),
                        (None, class.modifiers().is_abstract),
                        model,
                    );
                }
                TopLevelDecl::MixinClass(class) => {
                    let members = class.element_members();
                    let supertypes = class
                        .extends()
                        .into_iter()
                        .chain(class.with_clause())
                        .chain(class.implements())
                        .collect();
                    let member_supertypes =
                        class.with_clause().rev().chain(class.extends()).collect();
                    self.add_type(
                        class.name().name(),
                        class.type_params(),
                        supertypes,
                        member_supertypes,
                        members.clone(),
                        model,
                    );
                    self.add_declaration_facts(
                        class.name().name(),
                        class.annotations(),
                        class.type_params(),
                        members.clone(),
                        (None, class.is_abstract()),
                        model,
                    );
                }
                TopLevelDecl::Extension(extension) => {
                    if let Some(name) = extension.name() {
                        self.add_type(
                            name.name(),
                            extension.type_params(),
                            Vec::new(),
                            Vec::new(),
                            EffectiveMembers::declared(extension.members()),
                            model,
                        );
                    }
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(extension.type_params(), model);
                    let owner_parameters = parameter_ids(&parameters, extension.type_params());
                    let methods = extension
                        .members()
                        .filter_map(|member| match member {
                            ClassMember::Method(method) if !method.is_static() => {
                                parameters.push(method.type_params(), model);
                                record_bound_declarations(
                                    &mut self.bound_declared,
                                    &parameters,
                                    method.type_params(),
                                );
                                let resolved = signature(
                                    &parameters,
                                    &owner_parameters,
                                    method.type_params(),
                                    Some(method.params()),
                                    method.return_type(),
                                    SignatureContext::Function,
                                    model,
                                );
                                parameters.pop();
                                Some((method.name().name().to_owned(), resolved))
                            }
                            ClassMember::Operator(operator) => Some((
                                operator.lookup_name().to_owned(),
                                signature(
                                    &parameters,
                                    &owner_parameters,
                                    AstChildren::<TypeParam<'_>>::empty(),
                                    Some(operator.params()),
                                    operator.return_type(),
                                    SignatureContext::Function,
                                    model,
                                ),
                            )),
                            _ => None,
                        })
                        .collect();
                    let getters = extension
                        .members()
                        .filter_map(|member| match member {
                            ClassMember::Getter(getter) if !getter.is_static() => Some((
                                getter.name().name().to_owned(),
                                getter.return_type().map_or(ResolvedType::Dynamic, |ty| {
                                    model.resolve_type_in(ty, &parameters)
                                }),
                            )),
                            _ => None,
                        })
                        .collect();
                    self.extensions.push(ExtensionFacts {
                        declaring_library: model.library,
                        name: extension.name().map(|name| name.name().to_owned()),
                        identity: extension
                            .name()
                            .and_then(|name| model.resolve_extension(name.name())),
                        on_type: model.resolve_type_in(extension.on_type(), &parameters),
                        parameters: owner_parameters,
                        parameter_bounds: extension
                            .type_params()
                            .filter_map(|parameter| {
                                let ResolvedType::TypeParameter { id, bound, .. } =
                                    parameters.lookup(parameter.name().name())?
                                else {
                                    return None;
                                };
                                Some((
                                    id,
                                    if parameter.bound().is_some() {
                                        bound.as_ref().clone()
                                    } else {
                                        ResolvedType::Dynamic
                                    },
                                ))
                            })
                            .collect(),
                        methods,
                        annotations: extension
                            .members()
                            .filter_map(|member| {
                                let (name, annotations) = match member {
                                    ClassMember::Method(method) => {
                                        (method.name().name(), method.annotations())
                                    }
                                    ClassMember::Getter(getter) => {
                                        (getter.name().name(), getter.annotations())
                                    }
                                    ClassMember::Operator(operator) => {
                                        (operator.lookup_name(), operator.annotations())
                                    }
                                    _ => return None,
                                };
                                let annotations = resolve_annotations(annotations, model);
                                (!annotations.is_empty()).then(|| (name.to_owned(), annotations))
                            })
                            .collect(),
                        getters,
                        members: extension
                            .members()
                            .filter_map(|member| match member {
                                ClassMember::Method(method) => {
                                    Some(method.name().name().to_owned())
                                }
                                _ => None,
                            })
                            .collect(),
                        instance_members: extension
                            .members()
                            .filter_map(|member| match member {
                                ClassMember::Method(method) if !method.is_static() => {
                                    Some(method.name().name().to_owned())
                                }
                                ClassMember::Getter(getter) if !getter.is_static() => {
                                    Some(getter.name().name().to_owned())
                                }
                                ClassMember::Setter(setter) if !setter.is_static() => {
                                    Some(format!("{}=", setter.name().name()))
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
                        extension.name().name(),
                        extension.type_params(),
                        extension.implements().collect(),
                        Vec::new(),
                        members.clone(),
                        model,
                    );
                    self.add_declaration_facts(
                        extension.name().name(),
                        extension.annotations(),
                        extension.type_params(),
                        members.clone(),
                        (
                            extension
                                .representation()
                                .and_then(|param| param.param_type()),
                            false,
                        ),
                        model,
                    );
                }
                TopLevelDecl::Mixin(mixin) => {
                    let supertypes = mixin.on_clause().chain(mixin.implements()).collect();
                    self.add_type(
                        mixin.name().name(),
                        mixin.type_params(),
                        supertypes,
                        Vec::new(),
                        EffectiveMembers::declared(mixin.members()),
                        model,
                    );
                    if let Some(identity) = model.resolve_name(mixin.name().name()) {
                        self.mixin_constraint_counts
                            .insert(identity, mixin.on_clause().len());
                    }
                    // Mixins are never instantiated: abstract for every consumer.
                    self.add_declaration_facts(
                        mixin.name().name(),
                        mixin.annotations(),
                        mixin.type_params(),
                        EffectiveMembers::declared(mixin.members()),
                        (None, true),
                        model,
                    );
                }
                TopLevelDecl::Enum(enumeration) => {
                    let members = enumeration.element_members();
                    self.add_type(
                        enumeration.name().name(),
                        enumeration.type_params(),
                        enumeration
                            .with_clause()
                            .chain(enumeration.implements())
                            .collect(),
                        enumeration.with_clause().rev().collect(),
                        members.clone(),
                        model,
                    );
                    self.add_declaration_facts(
                        enumeration.name().name(),
                        enumeration.annotations(),
                        enumeration.type_params(),
                        members.clone(),
                        (None, false),
                        model,
                    );
                    // Every enum implicitly extends `Enum`; only meaningful
                    // when `dart:core` sources describe it.
                    if model.identities.has_sdk_sources()
                        && let Some(identity) = model.resolve_name(enumeration.name().name())
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
                    if let Some(identity) = model.resolve_name(enumeration.name().name()) {
                        if !enumeration.with_clause().is_empty() {
                            let start = usize::from(model.identities.has_sdk_sources());
                            self.mixin_applications.insert(
                                identity.clone(),
                                start..start + enumeration.with_clause().len(),
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
                                .variants()
                                .map(|variant| variant.name().name().to_owned())
                                .collect(),
                        );
                    }
                }
                _ => {}
            }
        }
        self.add_immutability(program, model);
    }

    fn add_immutability(&mut self, program: Program<'_>, model: &SemanticModel<'_>) {
        for declaration in program.declarations() {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            let (name, annotations, superclass) = match declaration {
                TopLevelDecl::Class(node) => (node.name(), node.annotations(), node.extends()),
                TopLevelDecl::MixinClass(node) => (node.name(), node.annotations(), node.extends()),
                TopLevelDecl::ClassTypeAlias(node) => {
                    // Its superclass was already resolved in the alias's
                    // generic scope while indexing the mixin application.
                    (node.name(), node.annotations(), None)
                }
                TopLevelDecl::Mixin(node) => (node.name(), node.annotations(), None),
                TopLevelDecl::Enum(node) => (node.name(), node.annotations(), None),
                TopLevelDecl::ExtensionType(node) => (node.name(), node.annotations(), None),
                _ => continue,
            };
            let Some(identity) = model.resolve_name(name.name()) else {
                continue;
            };
            let immutable = if annotations.clone().any(|annotation| {
                QualifiedName::from_identifiers(annotation.name())
                    .and_then(|name| {
                        model
                            .resolve_value(name)
                            .or_else(|| model.resolve_imported_member(name))
                    })
                    .is_some_and(|identity| identity.is_meta_immutable())
            }) {
                Truth::Yes
            } else if annotations.clone().any(|annotation| {
                // An `immutable` annotation that resolves to nothing may come
                // through an export chain falcon does not follow.
                annotation
                    .name()
                    .last()
                    .is_some_and(|last| matches!(last.name(), "immutable" | "Immutable"))
                    && QualifiedName::from_identifiers(annotation.name())
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
                if matches!(
                    declaration,
                    TopLevelDecl::Class(_) | TopLevelDecl::MixinClass(_)
                ) {
                    scope.push(declaration.type_params(), model);
                }
                self.superclasses
                    .insert(identity, model.resolve_type_in(superclass, &scope));
            }
        }
    }

    pub(super) fn resolve_constructor_formals(
        &mut self,
        program: Program<'_>,
        model: &SemanticModel<'_>,
    ) -> bool {
        enum Work<'a> {
            Alias(&'a str),
            Declaration {
                identity: DeclarationIdentity,
                superclass: Option<ResolvedType>,
                constructors: Vec<falcon_syntax::cst::ConstructorRef<'a>>,
            },
        }

        // Syntax and model ownership do not change during refinement. Cache the
        // participating constructors, but retain the full declaration-count bound
        // and source order for aliases and super-formal dependency chains.
        let mut declaration_count = 0;
        let mut work = Vec::new();
        for declaration in program.declarations() {
            declaration_count += 1;
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            if let TopLevelDecl::ClassTypeAlias(alias) = declaration {
                work.push(Work::Alias(alias.name().name()));
                continue;
            }
            let (name, type_parameters, superclass, members) = match declaration {
                TopLevelDecl::Class(class) => (
                    class.name().name(),
                    class.type_params(),
                    class.extends(),
                    class.element_members(),
                ),
                TopLevelDecl::MixinClass(class) => (
                    class.name().name(),
                    class.type_params(),
                    class.extends(),
                    class.element_members(),
                ),
                TopLevelDecl::Enum(enumeration) => (
                    enumeration.name().name(),
                    enumeration.type_params(),
                    None,
                    EffectiveMembers::declared(enumeration.members()),
                ),
                TopLevelDecl::ExtensionType(extension) => (
                    extension.name().name(),
                    extension.type_params(),
                    None,
                    extension.element_members(),
                ),
                _ => continue,
            };
            let constructors: Vec<_> = members
                .constructors()
                .filter(|constructor| {
                    constructor
                        .params
                        .params()
                        .any(|parameter| parameter.initializing().is_some())
                })
                .collect();
            if constructors.is_empty() {
                continue;
            }
            let Some(identity) = model.resolve_name(name) else {
                continue;
            };
            let mut scope = TypeParameterScope::default();
            scope.push(type_parameters, model);
            let superclass = superclass.map(|ty| model.resolve_type_in(ty, &scope));
            work.push(Work::Declaration {
                identity,
                superclass,
                constructors,
            });
        }

        let mut resolved = false;
        for _ in 0..declaration_count {
            let mut changed = false;
            for item in &work {
                let (identity, superclass, constructors) = match item {
                    Work::Alias(name) => {
                        changed |= self.forward_alias_constructors(name, model);
                        continue;
                    }
                    Work::Declaration {
                        identity,
                        superclass,
                        constructors,
                    } => (identity, superclass, constructors),
                };
                for constructor in constructors {
                    let constructor_name = constructor
                        .constructor_name
                        .map(|name| name.name())
                        .unwrap_or("new");
                    for parameter in constructor.params.params() {
                        if parameter.initializing() != Some(InitializingFormal::This)
                            || parameter.param_type().is_some()
                        {
                            continue;
                        }
                        let Some(ty) = self
                            .member_entry(identity, parameter.name().name())
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
                            identity,
                            constructor_name,
                            constructor.params,
                            parameter,
                            ty,
                        );
                    }
                    let Some(superclass) = superclass else {
                        continue;
                    };
                    let super_constructor = constructor
                        .initializers
                        .clone()
                        .find_map(|initializer| match initializer {
                            falcon_syntax::cst::ConstructorInitializer::SuperCall(call) => {
                                Some(call.call_name().map_or("new", |name| name.name()))
                            }
                            _ => None,
                        })
                        .unwrap_or("new");
                    let positional_super = constructor.params.positional().filter(|parameter| {
                        parameter.initializing() == Some(InitializingFormal::Super)
                    });
                    let named_super = constructor.params.named().filter(|parameter| {
                        parameter.initializing() == Some(InitializingFormal::Super)
                    });
                    let super_formals = positional_super
                        .enumerate()
                        .map(|(ordinal, parameter)| (SuperFormal::Positional(ordinal), parameter))
                        .chain(named_super.map(|parameter| {
                            (SuperFormal::Named(parameter.name().name()), parameter)
                        }));
                    for (formal, parameter) in super_formals {
                        if parameter.default_value().is_none()
                            && matches!(
                                parameter.kind(),
                                ParamKind::OptionalPositional
                                    | ParamKind::Named { required: false }
                            )
                        {
                            changed |= self.inherit_super_default(
                                (identity, constructor_name),
                                parameter.name().name(),
                                (superclass, super_constructor),
                                formal,
                            );
                        }
                        if parameter.param_type().is_some() {
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
                            identity,
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
        params: FormalParamList<'_>,
        parameter: FormalParam<'_>,
        ty: ResolvedType,
    ) -> bool {
        let parameter_name = parameter.name();
        let mut changed = false;
        if let Some(signature) = self
            .constructors
            .get_mut(identity)
            .and_then(|constructors| constructors.get_mut(constructor_name))
        {
            if matches!(
                parameter.kind(),
                falcon_syntax::cst::ParamKind::Named { .. }
            ) {
                if signature.named.get(parameter_name.name()) != Some(&ty) {
                    signature
                        .named
                        .insert(parameter_name.name().to_owned(), ty.clone());
                    changed = true;
                }
            } else if let Some(index) = params
                .positional()
                .position(|candidate| candidate.name().span() == parameter_name.span())
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
                .find(|candidate| candidate.name == parameter_name.name())
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
        members: EffectiveMembers<'_>,
        model: &SemanticModel<'_>,
    ) {
        let bodies: FxHashMap<String, ConstConstructorBody> = members
            .constructors()
            .filter(|constructor| constructor.is_const && !constructor.is_external)
            .map(|constructor| {
                (
                    constructor
                        .constructor_name
                        .map_or_else(|| "new".to_string(), |name| name.name().to_owned()),
                    ConstConstructorBody {
                        is_primary: constructor.is_primary,
                        representation: members.representation,
                        params: constructor
                            .params
                            .params()
                            .map(|parameter| model.node_ref(parameter.syntax()))
                            .collect(),
                        initializers: constructor
                            .initializers
                            .map(|initializer| model.node_ref(initializer.syntax()))
                            .collect(),
                        redirect: constructor
                            .redirect
                            .map(|redirect| model.node_ref(redirect.syntax())),
                    },
                )
            })
            .collect();
        if bodies.is_empty() {
            return;
        }
        let fields = members
            .declared
            .clone()
            .filter_map(|member| match member {
                ClassMember::Field(field) if !field.is_static() => Some(field),
                _ => None,
            })
            .flat_map(|field| {
                field.declarators().filter_map(|declarator| {
                    Some((
                        declarator.name().name().to_owned(),
                        model.node_ref(declarator.initializer()?.syntax()),
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
        annotations: AstChildren<'_, falcon_syntax::cst::Annotation<'_>>,
        type_parameters: AstChildren<'_, TypeParam<'_>>,
        members: EffectiveMembers<'_>,
        declaration_kind: (Option<DartType<'_>>, bool),
        model: &SemanticModel<'_>,
    ) {
        let (extension_representation, is_abstract) = declaration_kind;
        let Some(identity) = model.resolve_name(name) else {
            return;
        };
        self.add_const_constructor_bodies(&identity, members.clone(), model);
        let mut scope = TypeParameterScope::default();
        scope.push(type_parameters, model);
        let library_target = annotations
            .clone()
            .any(|annotation| annotation_targets_library(annotation, model));
        let annotations = resolve_annotations(annotations, model);
        let constructors = members
            .constructors()
            .map(|constructor| ConstructorFacts {
                name: constructor
                    .constructor_name
                    .map(|name| name.name().to_owned())
                    .unwrap_or_else(|| "new".to_string()),
                annotations: resolve_annotations(constructor.annotations, model),
                is_const: constructor.is_const,
                is_factory: constructor.is_factory,
                is_private: constructor
                    .constructor_name
                    .is_some_and(|name| name.name().starts_with('_')),
                redirect_target: constructor.redirect.map(|redirect| {
                    (
                        model.resolve_type(redirect.type_()),
                        redirect
                            .constructor_name()
                            .map_or_else(|| "new".to_string(), |name| name.name().to_owned()),
                    )
                }),
                parameters: constructor
                    .params
                    .params()
                    .map(|parameter| ParameterFacts {
                        name: parameter.name().name().to_owned(),
                        ty: parameter
                            .param_type()
                            .map(|ty| model.resolve_type_in(ty, &scope))
                            .or_else(|| {
                                (parameter.initializing() == Some(InitializingFormal::This)).then(
                                    || {
                                        self.member_entry(&identity, parameter.name().name())
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
                            parameter.kind(),
                            falcon_syntax::cst::ParamKind::Named { .. }
                        ),
                        is_super: parameter.initializing() == Some(InitializingFormal::Super),
                        is_field: parameter.initializing() == Some(InitializingFormal::This)
                            || (constructor.is_primary
                                && (parameter.is_var() || parameter.is_final())),
                    })
                    .collect(),
            })
            .collect();
        let static_consts = members
            .declared
            .clone()
            .filter_map(|member| match member {
                ClassMember::Field(field)
                    if field.is_static() && field.keyword() == VarKeyword::Const =>
                {
                    Some(field)
                }
                _ => None,
            })
            .flat_map(|field| {
                let field_scope = scope.clone();
                let declared_ty = field
                    .field_type()
                    .map(|ty| model.resolve_type_in(ty, &scope));
                let is_deprecated = field
                    .annotations()
                    .any(|annotation| model.is_deprecation(annotation));
                field.declarators().filter_map(move |declarator| {
                    declarator
                        .initializer()
                        .map(|initializer| StaticConstFacts {
                            name: declarator.name().name().to_owned(),
                            ty: declared_ty.clone().unwrap_or_else(|| {
                                infer_const_initializer_type(initializer, model, &field_scope)
                            }),
                            initializer: model.node_ref(initializer.syntax()),
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
            let DeclarationIdentity::Library { library, .. } = subtype else {
                continue;
            };
            visited.clear();
            visited.insert(subtype);
            let mut current = self.superclasses.get(subtype);
            while let Some(ResolvedType::Interface { identity, .. }) = current {
                if !visited.insert(identity) {
                    break;
                }
                if matches!(identity, DeclarationIdentity::Library { library: other, .. } if other == library)
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

    fn add_type(
        &mut self,
        name: &str,
        type_parameters: AstChildren<'_, TypeParam<'_>>,
        supertypes: Vec<DartType<'_>>,
        member_supertypes: Vec<DartType<'_>>,
        members: EffectiveMembers<'_>,
        model: &SemanticModel<'_>,
    ) {
        let type_identity = model.resolve_name(name);
        let is_extension = type_identity.is_none();
        let Some(identity) = type_identity.or_else(|| model.resolve_extension(name)) else {
            return;
        };
        let mut scope = TypeParameterScope::default();
        scope.push(type_parameters.clone(), model);
        let owner_parameters = parameter_ids(&scope, type_parameters.clone());
        record_bound_declarations(&mut self.bound_declared, &scope, type_parameters.clone());
        let bounds: HashMap<_, _> = owner_parameters
            .iter()
            .zip(type_parameters)
            .map(|(id, parameter)| {
                (
                    id.clone(),
                    parameter
                        .bound()
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
            .clone()
            .filter_map(|member| match member {
                ClassMember::Field(field) if !field.is_static() => Some(field),
                _ => None,
            })
            .flat_map(|field| {
                let field_type = field
                    .field_type()
                    .map(|ty| model.resolve_type_in(ty, &scope))
                    .unwrap_or(ResolvedType::Unknown);
                field.declarators().map(move |declarator| {
                    (declarator.name().name().to_string(), field_type.clone())
                })
            })
            .collect::<HashMap<_, _>>();
        for parameter in members.declaring_params() {
            field_types.insert(
                parameter.name().name().to_string(),
                parameter
                    .param_type()
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
        for member in members.declared.clone() {
            match member {
                ClassMember::Method(method) => {
                    let entry = entries.entry(method.name().name().to_string()).or_default();
                    scope.push(method.type_params(), model);
                    record_bound_declarations(
                        &mut self.bound_declared,
                        &scope,
                        method.type_params(),
                    );
                    entry.override_shape = Some(inherited::OverrideShape::new(
                        method.params(),
                        method.type_params(),
                        &scope,
                    ));
                    entry.is_instance_method |= !method.is_static();
                    let mut facts = MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Method,
                        is_static: method.is_static(),
                        // An `external` method has no body but is concrete.
                        is_abstract: method.is_abstract() && !method.is_external(),
                        positional_parameter_names: Vec::new(),
                        annotations: resolve_annotations(method.annotations(), model),
                        parameter_contracts: None,
                    };
                    entry.signature = Some(signature(
                        &scope,
                        &owner_parameters,
                        method.type_params(),
                        Some(method.params()),
                        method.return_type(),
                        SignatureContext::Member(&mut facts),
                        model,
                    ));
                    entry.facts.push(facts);
                    scope.pop();
                }
                ClassMember::Constructor(constructor) => {
                    self.constructors
                        .entry(identity.clone())
                        .or_default()
                        .insert(
                            constructor
                                .constructor_name()
                                .map(|name| name.name().to_string())
                                .unwrap_or_else(|| "new".to_string()),
                            signature(
                                &scope,
                                &owner_parameters,
                                AstChildren::empty(),
                                Some(constructor.params()),
                                None,
                                SignatureContext::Constructor(&field_types),
                                model,
                            ),
                        );
                }
                ClassMember::Field(field) => {
                    let field_type = field
                        .field_type()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Unknown);
                    for declarator in field.declarators() {
                        let kind = MemberKind::field(field, declarator);
                        let entry = entries
                            .entry(declarator.name().name().to_string())
                            .or_default();
                        entry.promotable_field = declarator.name().name().starts_with('_')
                            && !field.is_static()
                            && !field.is_abstract()
                            && !field.is_external()
                            && matches!(field.keyword(), VarKeyword::Final | VarKeyword::Const);
                        entry.facts.push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind,
                            is_static: field.is_static(),
                            is_abstract: field.is_abstract(),
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
                            annotations: resolve_annotations(field.annotations(), model),
                        });
                        entry.field = Some((owner_parameters.clone(), field_type.clone()));
                        if !field.is_static() {
                            entry.read = Some(field_type.clone());
                            if kind.has_setter() {
                                entry.write = Some(field_type.clone());
                            }
                        }
                    }
                }
                ClassMember::Getter(getter) => {
                    let entry = entries.entry(getter.name().name().to_string()).or_default();
                    entry.facts.push(MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Getter,
                        is_static: getter.is_static(),
                        is_abstract: getter.is_abstract(),
                        positional_parameter_names: Vec::new(),
                        parameter_contracts: Some(empty_parameter_contracts()),
                        annotations: resolve_annotations(getter.annotations(), model),
                    });
                    let return_type = getter
                        .return_type()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Dynamic);
                    entry.field = Some((owner_parameters.clone(), return_type.clone()));
                    entry.implicit_getter = getter.return_type().is_none();
                    if !getter.is_static() {
                        entry.read = Some(return_type);
                    }
                }
                ClassMember::Setter(setter) => {
                    let entry = entries.entry(setter.name().name().to_string()).or_default();
                    entry.facts.push(MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Setter,
                        is_static: setter.is_static(),
                        is_abstract: setter.is_abstract(),
                        positional_parameter_names: vec![setter.param().name().to_string()],
                        parameter_contracts: Some(setter_parameter_contracts(
                            setter.param().name(),
                            setter.is_covariant(),
                        )),
                        annotations: resolve_annotations(setter.annotations(), model),
                    });
                    let parameter_type = setter
                        .param_type()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Dynamic);
                    entry
                        .field
                        .get_or_insert_with(|| (owner_parameters.clone(), parameter_type.clone()));
                    if !setter.is_static() {
                        entry.write = Some(parameter_type);
                    }
                }
                ClassMember::Operator(operator) => {
                    let entry = entries
                        .entry(operator.lookup_name().to_owned())
                        .or_default();
                    entry.is_instance_method = true;
                    entry.override_shape = Some(inherited::OverrideShape::new(
                        operator.params(),
                        AstChildren::empty(),
                        &scope,
                    ));
                    let mut facts = MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Operator,
                        is_static: false,
                        is_abstract: operator.body().is_none() && !operator.is_external(),
                        positional_parameter_names: Vec::new(),
                        annotations: resolve_annotations(operator.annotations(), model),
                        parameter_contracts: None,
                    };
                    entry.signature = Some(signature(
                        &scope,
                        &owner_parameters,
                        AstChildren::empty(),
                        Some(operator.params()),
                        operator.return_type(),
                        SignatureContext::Member(&mut facts),
                        model,
                    ));
                    entry.facts.push(facts);
                }
                // Indexed through the constructor it completes (`element_members`).
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => {}
            }
        }
        for parameter in members.declaring_params() {
            let is_final = members.representation || parameter.is_final();
            let field_type = field_types[parameter.name().name()].clone();
            let entry = entries
                .entry(parameter.name().name().to_string())
                .or_default();
            entry.promotable_field = parameter.name().name().starts_with('_') && is_final;
            entry.facts.push(MemberFacts {
                declaring_type: identity.clone(),
                kind: MemberKind::Field {
                    writable: !is_final,
                    covariant: parameter.is_covariant(),
                },
                is_static: false,
                is_abstract: false,
                positional_parameter_names: Vec::new(),
                parameter_contracts: Some(if is_final {
                    empty_parameter_contracts()
                } else {
                    setter_parameter_contracts("value", parameter.is_covariant())
                }),
                annotations: resolve_annotations(parameter.annotations(), model),
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
                AstChildren::empty(),
                Some(primary.params()),
                None,
                SignatureContext::Constructor(&field_types),
                model,
            );
            for parameter in primary.params().named().filter(|parameter| {
                (parameter.is_final() || parameter.is_var() || members.representation)
                    && parameter.name().name().starts_with('_')
            }) {
                let name = parameter.name().name();
                let label = name.trim_start_matches('_');
                if let Some(ty) = primary_signature.named.remove(name) {
                    primary_signature.named.insert(label.to_string(), ty);
                }
                if let Some(ordered_name) =
                    Arc::make_mut(&mut primary_signature.named_parameter_names)
                        .iter_mut()
                        .find(|ordered_name| ordered_name.as_str() == name)
                {
                    label.clone_into(ordered_name);
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
                            if primary.params().named().any(|parameter| {
                                parameter.name().name() == *name
                                    && (parameter.is_final()
                                        || parameter.is_var()
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
                        .constructor_name()
                        .map_or_else(|| "new".to_string(), |name| name.name().to_string()),
                    primary_signature,
                );
        }
        if let DeclarationIdentity::Library { library, .. } = identity
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
}

fn annotation_targets_library(
    annotation: falcon_syntax::cst::Annotation<'_>,
    model: &SemanticModel<'_>,
) -> bool {
    if !matches!(
        QualifiedName::from_identifiers(annotation.name()).and_then(|name| model.resolve_imported_member(name)),
        Some(identity) if identity.is_package("meta", "Target")
    ) {
        return false;
    }
    let Some(args) = annotation.args() else {
        return false;
    };
    args.positional()
        .chain(args.named().map(|argument| argument.value()))
        .any(|expression| target_kind_library(expression, model))
}

fn target_kind_library(expression: Expr<'_>, model: &SemanticModel<'_>) -> bool {
    match expression.unparenthesized() {
        Expr::Field(field) if field.field().name() == "library" => {
            let Some(segments) = expression_segments(field.object(), false) else {
                return false;
            };
            matches!(
                QualifiedName::from_segments(&segments).and_then(|name| model.resolve_imported_member(name)),
                Some(identity) if identity.is_package("meta", "TargetKind")
            )
        }
        Expr::List(list) => list.elements().any(|element| {
            matches!(element, falcon_syntax::cst::CollectionElement::Expr(expression)
                if target_kind_library(expression, model))
        }),
        Expr::Set(set) => set.elements().any(|element| {
            matches!(element, falcon_syntax::cst::CollectionElement::Expr(expression)
                if target_kind_library(expression, model))
        }),
        _ => false,
    }
}

fn infer_const_initializer_type(
    initializer: Expr<'_>,
    model: &SemanticModel<'_>,
    scope: &TypeParameterScope,
) -> ResolvedType {
    match initializer {
        Expr::New(new) => model
            .resolve_type_in(new.dart_type(), scope)
            .with_nullable(false),
        Expr::Call(call) => resolve_call_constructor(call.callee(), model, false)
            .map_or(ResolvedType::Unknown, |(identity, _)| {
                ResolvedType::interface_identity(identity)
            }),
        Expr::Conditional(conditional) => {
            let left = infer_const_initializer_type(conditional.then_expr(), model, scope);
            let right = infer_const_initializer_type(conditional.else_expr(), model, scope);
            if left == right {
                left
            } else {
                ResolvedType::Unknown
            }
        }
        Expr::IntLit(_) => model.core_type("int", false),
        Expr::DoubleLit(_) => model.core_type("double", false),
        Expr::StringLit(_) => model.core_type("String", false),
        Expr::BoolLit(_) => model.core_type("bool", false),
        _ => ResolvedType::Unknown,
    }
}

pub(super) fn same_library(left: &DeclarationIdentity, right: &DeclarationIdentity) -> bool {
    matches!(
        (left, right),
        (
            DeclarationIdentity::Library { library: left, .. },
            DeclarationIdentity::Library { library: right, .. }
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
pub(super) fn inherits_method_types(member: ClassMember<'_>) -> bool {
    let (params, infer_return) = match member {
        ClassMember::Method(method) if !method.is_static() => {
            (method.params(), method.return_type().is_none())
        }
        ClassMember::Operator(operator) => (operator.params(), operator.return_type().is_none()),
        ClassMember::Setter(setter) => return !setter.is_static() && setter.param_type().is_none(),
        _ => return false,
    };
    infer_return
        || params.params().any(|param| {
            param.param_type().is_none()
                && param.function_params().is_none()
                && param.initializing().is_none()
        })
}

pub(super) enum SignatureContext<'a> {
    Function,
    Constructor(&'a HashMap<String, ResolvedType>),
    Member(&'a mut MemberFacts),
}

/// Resolves a callable, collecting declaration facts only when requested.
pub(super) fn signature<'a>(
    scope: &TypeParameterScope,
    owner_parameters: &[TypeParameterId],
    call_parameters: impl Iterator<Item = TypeParam<'a>> + Clone,
    params: Option<FormalParamList<'_>>,
    return_type: Option<DartType<'_>>,
    context: SignatureContext<'_>,
    model: &SemanticModel<'_>,
) -> ResolvedSignature {
    let (field_types, mut facts) = match context {
        SignatureContext::Function => (None, None),
        SignatureContext::Constructor(fields) => (Some(fields), None),
        SignatureContext::Member(facts) => (None, Some(facts)),
    };
    let parameter_type = |param: FormalParam<'_>, initializing, name: &str| {
        if param.param_type().is_some() || param.function_params().is_some() {
            model.resolve_formal_parameter_in(param, scope)
        } else if initializing == Some(InitializingFormal::This) {
            field_types
                .and_then(|fields| fields.get(name))
                .cloned()
                .unwrap_or(ResolvedType::Unknown)
        } else if initializing == Some(InitializingFormal::Super) {
            ResolvedType::Unknown
        } else {
            ResolvedType::Dynamic
        }
    };
    let mut result = ResolvedSignature::returning(ResolvedType::Dynamic);
    result.owner_parameters = owner_parameters.to_vec();
    result.call_parameters = parameter_ids(scope, call_parameters.clone());
    let mut required_named = Vec::new();
    let mut named_parameter_names = Vec::new();
    let mut contracts = Vec::new();
    for param in params.into_iter().flat_map(|params| params.params()) {
        let name = param.name().name();
        let kind = param.kind();
        let initializing = param.initializing();
        let default_value = param.default_value();
        if let Some(facts) = facts.as_deref_mut() {
            if !matches!(kind, ParamKind::Named { .. }) {
                facts.positional_parameter_names.push(name.to_string());
            }
            contracts.push(ParameterContract {
                name: name.to_string(),
                kind,
                is_covariant: param.is_covariant(),
                default_value_code: default_value.map(default_syntax::default_syntax_key),
            });
        }
        let optional = matches!(
            kind,
            ParamKind::OptionalPositional | ParamKind::Named { required: false }
        );
        let has_default = default_value.is_some() || optional;
        let ty = parameter_type(param, initializing, name);
        if let ParamKind::Named { required } = kind {
            if required {
                required_named.push(name.to_string());
            }
            result.named.insert(name.to_string(), ty);
            named_parameter_names.push(name.to_string());
        } else {
            result.optional_positional += usize::from(kind == ParamKind::OptionalPositional);
            result.positional.push(ty);
            if has_default {
                result
                    .positional_parameter_names
                    .resize_with(result.positional.len() - 1, String::new);
                result.positional_parameter_names.push(name.to_string());
            } else if !result.positional_parameter_names.is_empty() {
                result.positional_parameter_names.push(String::new());
            }
        }
        if has_default
            && (!optional
                || param.annotations().any(|annotation| {
                    annotation
                        .name()
                        .last()
                        .is_some_and(|name| name.name() == "required")
                }))
        {
            result.non_optional_defaults.insert(name.to_string());
        }
        // Super formals without a declared default inherit theirs once the super
        // constructor resolves (`resolve_constructor_formals`).
        let default = match default_value {
            Some(value) => Some(ParameterDefault::Declared(model.node_ref(value.syntax()))),
            None if optional && initializing != Some(InitializingFormal::Super) => {
                Some(ParameterDefault::Null)
            }
            None => None,
        };
        if let Some(default) = default {
            result.defaults.insert(name.to_string(), default);
        }
    }
    if !named_parameter_names.is_empty() {
        result.named_parameter_names = named_parameter_names.into();
    }
    if let Some(facts) = facts {
        facts.parameter_contracts = Some(if contracts.is_empty() {
            empty_parameter_contracts()
        } else {
            contracts.into()
        });
    }
    result.shape = FunctionTypeShape::from_scope(required_named, call_parameters, scope);
    result.return_type = return_type
        .map(|ty| model.resolve_type_in(ty, scope))
        .unwrap_or(ResolvedType::Dynamic);
    result
}

/// Whether `ty` mentions any of `parameters`.
pub(super) fn mentions_any(ty: &ResolvedType, parameters: &[TypeParameterId]) -> bool {
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

fn parameter_ids<'a>(
    scope: &TypeParameterScope,
    parameters: impl Iterator<Item = TypeParam<'a>>,
) -> Vec<TypeParameterId> {
    parameters
        .filter_map(|parameter| match scope.lookup(parameter.name().name()) {
            Some(ResolvedType::TypeParameter { id, .. }) => Some(id),
            _ => None,
        })
        .collect()
}

/// Records whether the declaration of each of `parameters` states a bound.
fn record_bound_declarations<'a>(
    declared: &mut FxHashMap<TypeParameterId, bool>,
    scope: &TypeParameterScope,
    parameters: impl Iterator<Item = TypeParam<'a>>,
) {
    for parameter in parameters {
        if let Some(ResolvedType::TypeParameter { id, .. }) = scope.lookup(parameter.name().name())
        {
            declared.insert(id, parameter.bound().is_some());
        }
    }
}

/// The external (SDK or package) identities of `annotations`.
fn resolve_annotations(
    annotations: AstChildren<'_, falcon_syntax::cst::Annotation<'_>>,
    model: &SemanticModel<'_>,
) -> Vec<DeclarationIdentity> {
    annotations
        .filter_map(|annotation| {
            model.resolve_imported_member(QualifiedName::from_identifiers(annotation.name())?)
        })
        .collect()
}
