use super::{
    Arc, AstChildren, ClassMember, ClassRestriction, ConstConstructorBody, ConstructorFacts,
    DartType, DeclarationFacts, DeclarationIdentity, EffectiveMembers, Expr, ExtensionFacts,
    ExtensionMemberName, ExtensionMembers, ExternalLibraries, FileId, FormalParam, FormalParamList,
    FunctionKind, FunctionTypeShape, FxHashMap, FxHashSet, HashMap, HashSet, IdentityIndex,
    InitializingFormal, LibraryId, MemberEntry, MemberFacts, MemberKind, ParamKind,
    ParameterContract, ParameterDefault, ParameterFacts, ParameterOwner, PathBuf, Program,
    QualifiedName, ResolvedSignature, ResolvedType, SemanticModel, SignatureIndex, SignatureMap,
    StaticConstFacts, SuperFormal, TopLevelDecl, Truth, TypeFacts, TypeIndex, TypeKind, TypeParam,
    TypeParameterId, TypeParameterScope, VarKeyword, constructed_identity, default_syntax,
    empty_parameter_contracts, expression_segments, find_type_including_bounds, index_fixpoint,
    inherited, instantiate_parameter_bound, refine_inferred_type, resolve_call_constructor,
    setter_parameter_contracts,
};
use crate::resolve::semantic::ResolvedTypeKind;

type IndexedProgram<'a> = (Option<FileId>, &'a Arc<crate::ParsedFile>, bool);

pub(super) enum ConstructorInferenceEntry<'a> {
    Alias {
        identity: DeclarationIdentity,
        name: &'a str,
    },
    Declaration {
        identity: DeclarationIdentity,
        superclass: Option<ResolvedType>,
        constructor_name: &'a str,
        params: FormalParamList<'a>,
        super_constructor: &'a str,
    },
}

impl SignatureIndex {
    pub(crate) fn flow_writes(&self, file: FileId) -> Option<&Arc<super::flow_writes::FlowWrites>> {
        self.flow_writes.get(&Some(file))
    }

    pub(crate) fn file_body_is_current(
        &self,
        file: FileId,
        parsed: &Arc<crate::ParsedFile>,
    ) -> bool {
        self.flow_writes(file)
            .is_some_and(|writes| writes.source_revision.ptr_eq(&Arc::downgrade(parsed)))
    }

    /// Replace body-local flow facts while retaining unchanged declaration indexes.
    pub fn refresh_file_body(&mut self, file: FileId, parsed: &Arc<crate::ParsedFile>) {
        self.flow_writes.insert(
            Some(file),
            Arc::new(super::flow_writes::FlowWrites::from_parsed(parsed)),
        );
    }

    pub(super) fn member_entry(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&MemberEntry> {
        index_fixpoint::record_inference_read(index_fixpoint::InferenceInputRef::Member(
            identity, name,
        ));
        self.member_entries.get(identity)?.get(name)
    }

    #[must_use]
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
        let all: Vec<IndexedProgram<'_>> = files
            .iter()
            .map(|(path, parsed)| (identities.file(path), parsed, true))
            .collect();
        Self::build_programs(index, &all, identities, types)
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
        Self::build_programs(Self::default(), &all, identities, types)
    }

    /// Replace facts owned by dirty project libraries, retaining all other
    /// declarations and the immutable external-only overlay. File IDs must be
    /// stable, and `dirty` must include transitive import/export dependents and
    /// both sides of any part regrouping.
    pub fn update_project(
        &mut self,
        files: &[crate::resolve::IdentitySource<'_>],
        identities: &IdentityIndex,
        types: &TypeIndex,
        dirty: &HashSet<LibraryId>,
    ) {
        if dirty.is_empty() {
            return;
        }
        self.clear_field_forwarders();
        let keep_identity = |identity: &DeclarationIdentity| {
            !identities
                .declaration_library(identity)
                .is_some_and(|library| dirty.contains(&library))
        };
        self.functions
            .local
            .retain(|identity, _| keep_identity(identity));
        self.value_types
            .local
            .retain(|identity, _| keep_identity(identity));
        self.member_entries
            .local
            .retain(|identity, _| keep_identity(identity));
        self.type_facts
            .local
            .retain(|identity, _| keep_identity(identity));
        self.constants
            .local
            .retain(|identity, _| keep_identity(identity));
        self.constant_types
            .local
            .retain(|identity, _| keep_identity(identity));
        self.function_annotations
            .local
            .retain(|identity, _| keep_identity(identity));
        self.value_getters.retain(keep_identity);
        self.cyclic_values.retain(keep_identity);
        self.blocked_field_promotions
            .local
            .retain(|library, _| !dirty.contains(library));
        let keep_file = |file: Option<FileId>| {
            !file.is_some_and(|file| dirty.contains(&identities.library(file)))
        };
        self.flow_writes.retain(|file, _| keep_file(*file));
        let keep_parameter = |parameter: &TypeParameterId| match parameter.owner {
            ParameterOwner::Source { file, .. } => keep_file(file),
            ParameterOwner::Sdk(_) => true,
        };
        self.owner_defaults
            .local
            .retain(|parameter, _| keep_parameter(parameter));
        self.bound_declared
            .local
            .retain(|parameter, _| keep_parameter(parameter));
        self.extensions.retain(|extension| {
            !extension
                .declaring_library
                .is_some_and(|library| dirty.contains(&library))
        });
        self.cyclic_supertypes = None;
        self.extension_members = None;
        let programs: Vec<_> = files
            .iter()
            .filter_map(|source| {
                let file = identities.file(source.path)?;
                dirty.contains(&identities.library(file)).then(|| {
                    (
                        source.parsed.program(),
                        SemanticModel::from_file(
                            Some(file),
                            source.parsed,
                            identities,
                            Some(types),
                        ),
                    )
                })
            })
            .collect();
        for (program, model) in &programs {
            self.add_declarations(*program, model, true);
        }
        self.finish();
        self.infer_programs(&programs);
        self.mark_library_subclasses();
        self.mark_field_forwarders(identities, Some(types));
    }

    fn build_programs(
        mut index: Self,
        all: &[IndexedProgram<'_>],
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        for (file, parsed, analyze_bodies) in all {
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

    pub(super) fn add_declarations(
        &mut self,
        program: Program<'_>,
        model: &SemanticModel<'_>,
        analyze_bodies: bool,
    ) {
        if analyze_bodies {
            self.flow_writes.entry(model.file).or_insert_with(|| {
                Arc::new(model.parsed.map_or_else(
                    || super::flow_writes::FlowWrites::program(program),
                    super::flow_writes::FlowWrites::from_parsed,
                ))
            });
        }
        for declaration in program.declarations() {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            self.add_type_kind(declaration, model);
            match declaration {
                TopLevelDecl::ClassTypeAlias(alias) => {
                    self.add_class_alias_declaration(alias, model);
                }
                TopLevelDecl::TypeAlias(alias) => {
                    self.add_type_alias_declaration(alias, model);
                }
                TopLevelDecl::Function(function)
                    if matches!(function.kind(), FunctionKind::Function(_)) =>
                {
                    self.add_function_declaration(function, model);
                }
                TopLevelDecl::Function(function)
                    if matches!(function.kind(), FunctionKind::Getter) =>
                {
                    self.add_getter_declaration(function, model);
                }
                TopLevelDecl::Variable(variable) => {
                    self.add_variable_declaration(variable, model);
                }
                TopLevelDecl::Class(class) => {
                    self.add_class_declaration(class, model);
                }
                TopLevelDecl::MixinClass(class) => {
                    self.add_mixin_class_declaration(class, model);
                }
                TopLevelDecl::Extension(extension) => {
                    self.add_extension_declaration(extension, model);
                }
                TopLevelDecl::ExtensionType(extension) => {
                    self.add_extension_type_declaration(extension, model);
                }
                TopLevelDecl::Mixin(mixin) => {
                    self.add_mixin_declaration(mixin, model);
                }
                TopLevelDecl::Enum(enumeration) => {
                    self.add_enum_declaration(enumeration, model);
                }
                _ => {}
            }
        }
        self.add_immutability(program, model);
    }

    fn add_type_kind(&mut self, declaration: TopLevelDecl<'_>, model: &SemanticModel<'_>) {
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
            let facts = self.type_facts.entry(identity).or_default();
            facts.inherited_object = Some(inherited_object);
            facts.restriction = restriction;
            facts.kind = Some(kind);
        }
    }

    fn add_class_alias_declaration(
        &mut self,
        alias: falcon_syntax::cst::ClassTypeAliasDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        self.add_declaration_facts(
            alias.name().name(),
            alias.annotations(),
            alias.type_params(),
            &EffectiveMembers::declared(AstChildren::empty()),
            (None, alias.modifiers().is_abstract),
            model,
        );
        self.add_type(
            alias.name().name(),
            alias.type_params(),
            TopLevelDecl::ClassTypeAlias(alias),
            &EffectiveMembers::declared(AstChildren::empty()),
            model,
        );
    }

    // Alias parameters drive constructor-call and tear-off inference (`alias_template`).
    fn add_type_alias_declaration(
        &mut self,
        alias: falcon_syntax::cst::TypeAliasDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        let mut parameters = TypeParameterScope::default();
        parameters.push(alias.type_params(), model);
        record_bound_declarations(&mut self.bound_declared, &parameters, alias.type_params());
        self.add_declaration_facts(
            alias.name().name(),
            alias.annotations(),
            alias.type_params(),
            &EffectiveMembers::declared(AstChildren::empty()),
            (None, false),
            model,
        );
    }

    fn add_function_declaration(
        &mut self,
        function: falcon_syntax::cst::FunctionDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
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

    fn add_getter_declaration(
        &mut self,
        function: falcon_syntax::cst::FunctionDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        if let Some(identity) = model.resolve_value(function.name().name()) {
            self.value_getters.insert(identity.clone());
            self.value_types.insert(
                identity,
                function
                    .return_type()
                    .map_or(ResolvedType::DYNAMIC, |ty| model.resolve_type(ty)),
            );
        }
    }

    fn add_variable_declaration(
        &mut self,
        variable: falcon_syntax::cst::TopLevelVarDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        for declarator in variable.declarators() {
            if !model.owns_declaration(declarator.name().name()) {
                continue;
            }
            let Some(identity) = model.resolve_value(declarator.name().name()) else {
                continue;
            };
            self.value_types.insert(
                identity.clone(),
                variable.var_type().map_or_else(
                    || {
                        if declarator.initializer().is_none() {
                            ResolvedType::DYNAMIC
                        } else {
                            ResolvedType::UNKNOWN
                        }
                    },
                    |ty| model.resolve_type(ty),
                ),
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

    fn add_class_declaration(
        &mut self,
        class: falcon_syntax::cst::ClassDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        let members = class.element_members();
        self.add_type(
            class.name().name(),
            class.type_params(),
            TopLevelDecl::Class(class),
            &members,
            model,
        );
        self.add_declaration_facts(
            class.name().name(),
            class.annotations(),
            class.type_params(),
            &members,
            (None, class.modifiers().is_abstract),
            model,
        );
    }

    fn add_mixin_class_declaration(
        &mut self,
        class: falcon_syntax::cst::MixinClassDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        let members = class.element_members();
        self.add_type(
            class.name().name(),
            class.type_params(),
            TopLevelDecl::MixinClass(class),
            &members,
            model,
        );
        self.add_declaration_facts(
            class.name().name(),
            class.annotations(),
            class.type_params(),
            &members,
            (None, class.is_abstract()),
            model,
        );
    }

    fn add_extension_declaration(
        &mut self,
        extension: falcon_syntax::cst::ExtensionDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        if let Some(name) = extension.name() {
            self.add_type(
                name.name(),
                extension.type_params(),
                TopLevelDecl::Extension(extension),
                &EffectiveMembers::declared(extension.members()),
                model,
            );
        }
        let mut parameters = TypeParameterScope::default();
        parameters.push(extension.type_params(), model);
        let owner_parameters = parameter_ids(&parameters, extension.type_params());
        let methods = self.extension_methods(extension, &mut parameters, &owner_parameters, model);
        let getters = extension
            .members()
            .filter_map(|member| match member {
                ClassMember::Getter(getter) if !getter.is_static() => Some((
                    getter.name().name().to_owned(),
                    getter.return_type().map_or(ResolvedType::DYNAMIC, |ty| {
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
                    let ResolvedType {
                        kind: ResolvedTypeKind::TypeParameter { id, bound, .. },
                    } = parameters.lookup(parameter.name().name())?
                    else {
                        return None;
                    };
                    Some((
                        id,
                        if parameter.bound().is_some() {
                            bound.as_ref().clone()
                        } else {
                            ResolvedType::DYNAMIC
                        },
                    ))
                })
                .collect(),
            methods,
            annotations: extension_annotations(extension, model),
            getters,
            members: extension
                .members()
                .filter_map(|member| match member {
                    ClassMember::Method(method) => Some(method.name().name().to_owned()),
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
                    ClassMember::Operator(operator) => Some(operator.lookup_name().to_owned()),
                    _ => None,
                })
                .collect(),
        });
    }

    fn add_extension_type_declaration(
        &mut self,
        extension: falcon_syntax::cst::ExtensionTypeDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        let members = extension.element_members();
        self.add_type(
            extension.name().name(),
            extension.type_params(),
            TopLevelDecl::ExtensionType(extension),
            &members,
            model,
        );
        self.add_declaration_facts(
            extension.name().name(),
            extension.annotations(),
            extension.type_params(),
            &members,
            (
                extension
                    .representation()
                    .and_then(falcon_syntax::cst::FormalParam::param_type),
                false,
            ),
            model,
        );
    }

    fn add_mixin_declaration(
        &mut self,
        mixin: falcon_syntax::cst::MixinDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        self.add_type(
            mixin.name().name(),
            mixin.type_params(),
            TopLevelDecl::Mixin(mixin),
            &EffectiveMembers::declared(mixin.members()),
            model,
        );
        // Mixins are never instantiated: abstract for every consumer.
        self.add_declaration_facts(
            mixin.name().name(),
            mixin.annotations(),
            mixin.type_params(),
            &EffectiveMembers::declared(mixin.members()),
            (None, true),
            model,
        );
    }

    fn add_enum_declaration(
        &mut self,
        enumeration: falcon_syntax::cst::EnumDecl<'_>,
        model: &SemanticModel<'_>,
    ) {
        let members = enumeration.element_members();
        self.add_type(
            enumeration.name().name(),
            enumeration.type_params(),
            TopLevelDecl::Enum(enumeration),
            &members,
            model,
        );
        self.add_declaration_facts(
            enumeration.name().name(),
            enumeration.annotations(),
            enumeration.type_params(),
            &members,
            (None, false),
            model,
        );
        // Every enum implicitly extends `Enum`; only meaningful
        // when `dart:core` sources describe it.
        if model.identities.has_sdk_sources()
            && let Some(identity) = model.resolve_name(enumeration.name().name())
        {
            let enum_type = model.core_type("Enum", false);
            if let Some((_, supertypes)) = self
                .type_facts
                .get_mut(&identity)
                .and_then(|facts| facts.supertypes.as_mut())
            {
                // Inherited interfaces start with Enum, then apply
                // mixins in source order and finally interfaces.
                supertypes.insert(0, enum_type.clone());
            }
            if let Some(supertypes) = self
                .type_facts
                .get_mut(&identity)
                .and_then(|facts| facts.member_supertypes.as_mut())
            {
                // Concrete lookup searches mixins in reverse order
                // before the superclass.
                let object = supertypes.len().saturating_sub(1);
                supertypes.insert(object, enum_type);
            }
        }
        if let Some(identity) = model.resolve_name(enumeration.name().name()) {
            // The language synthesizes a static const list, including
            // the enum's instantiation to bounds for generic enums.
            let enum_type =
                self.with_raw_arguments(&ResolvedType::interface_identity(identity.clone()));
            let values_type = ResolvedType::from_kind(ResolvedTypeKind::Interface {
                occurrence: super::FunctionTypeAlias::default(),
                identity: DeclarationIdentity::sdk("dart:core", "List"),
                arguments: vec![enum_type].into(),
                nullable: false,
                extension_type: false,
            });
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
            self.type_facts.entry(identity).or_default().enum_values = Some(
                enumeration
                    .variants()
                    .map(|variant| variant.name().name().to_owned())
                    .collect(),
            );
        }
    }

    fn extension_methods(
        &mut self,
        extension: falcon_syntax::cst::ExtensionDecl<'_>,
        parameters: &mut TypeParameterScope,
        owner_parameters: &[TypeParameterId],
        model: &SemanticModel<'_>,
    ) -> FxHashMap<String, ResolvedSignature> {
        extension
            .members()
            .filter_map(|member| match member {
                ClassMember::Method(method) if !method.is_static() => {
                    parameters.push(method.type_params(), model);
                    record_bound_declarations(
                        &mut self.bound_declared,
                        parameters,
                        method.type_params(),
                    );
                    let resolved = signature(
                        parameters,
                        owner_parameters,
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
                        parameters,
                        owner_parameters,
                        AstChildren::<TypeParam<'_>>::empty(),
                        Some(operator.params()),
                        operator.return_type(),
                        SignatureContext::Function,
                        model,
                    ),
                )),
                _ => None,
            })
            .collect()
    }

    fn apply_constructor_parameter_updates(
        &mut self,
        identity: &DeclarationIdentity,
        constructor_name: &str,
        params: FormalParamList<'_>,
        updates: Vec<(
            FormalParam<'_>,
            Option<ResolvedType>,
            Option<ParameterDefault>,
        )>,
    ) {
        let mut defaults_changed = false;
        for (parameter, ty, default) in updates {
            if let Some(ty) = ty {
                self.update_constructor_parameter_type(
                    identity,
                    constructor_name,
                    params,
                    parameter,
                    ty,
                );
            }
            if let Some(default) = default
                && let Some(signature) = self
                    .type_facts
                    .get_mut(identity)
                    .and_then(|facts| facts.constructors.get_mut(constructor_name))
                && !signature.defaults.contains_key(parameter.name().name())
            {
                signature
                    .defaults
                    .insert(parameter.name().name().to_owned(), default);
                defaults_changed = true;
            }
        }
        if defaults_changed {
            index_fixpoint::record_inference_write(|| {
                index_fixpoint::InferenceInput::Constructor(
                    identity.clone(),
                    constructor_name.to_owned(),
                )
            });
            index_fixpoint::record_inference_write(|| {
                index_fixpoint::InferenceInput::Constructors(identity.clone())
            });
        }
    }

    fn mixins_have_no_fields(&self, identity: &DeclarationIdentity) -> bool {
        self.type_facts.get(identity).is_some_and(|facts| {
            facts.mixin_applications.iter().all(|mixin| {
                let ResolvedType {
                    kind: ResolvedTypeKind::Interface { identity, .. },
                } = mixin
                else {
                    return false;
                };
                self.type_kind(identity).is_some()
                    && self.member_entries.get(identity).is_some_and(|entries| {
                        entries.values().all(|entry| {
                            entry.facts.iter().all(|fact| {
                                fact.is_static || !matches!(fact.kind, MemberKind::Field { .. })
                            })
                        })
                    })
            })
        })
    }

    fn publish_alias_constructors(
        &mut self,
        identity: &DeclarationIdentity,
        forwarded: Vec<(ConstructorFacts, ResolvedSignature)>,
    ) -> bool {
        let mut changed = false;
        for (facts, signature) in forwarded {
            let name = facts.name.clone();
            let mut entry_changed = false;
            let constructors = &mut self
                .type_facts
                .entry(identity.clone())
                .or_default()
                .constructors;
            if let Some(existing) = constructors.get_mut(&name) {
                for (current, candidate) in existing.positional.iter_mut().zip(signature.positional)
                {
                    entry_changed |= refine_inferred_type(current, candidate);
                }
                for (name, candidate) in signature.named {
                    if let Some(current) = existing.named.get_mut(&name) {
                        entry_changed |= refine_inferred_type(current, candidate);
                    }
                }
                for (name, default) in signature.defaults {
                    if let std::collections::hash_map::Entry::Vacant(entry) =
                        existing.defaults.entry(name)
                    {
                        entry.insert(default);
                        entry_changed = true;
                    }
                }
            } else {
                constructors.insert(name.clone(), signature);
                entry_changed = true;
            }
            if let Some(declaration) = self
                .type_facts
                .get_mut(identity)
                .and_then(|facts| facts.declaration.as_mut())
            {
                if let Some(existing) = declaration
                    .constructors
                    .iter_mut()
                    .find(|existing| existing.name == facts.name)
                {
                    for (current, candidate) in existing.parameters.iter_mut().zip(facts.parameters)
                    {
                        entry_changed |= refine_inferred_type(&mut current.ty, candidate.ty);
                    }
                } else {
                    declaration.constructors.push(facts);
                    entry_changed = true;
                }
            }
            if entry_changed {
                index_fixpoint::record_inference_write(|| {
                    index_fixpoint::InferenceInput::Constructor(identity.clone(), name)
                });
                index_fixpoint::record_inference_write(|| {
                    index_fixpoint::InferenceInput::Constructors(identity.clone())
                });
                changed = true;
            }
        }
        changed
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
            self.type_facts
                .entry(identity.clone())
                .or_default()
                .immutable = Some(immutable);
            if let Some(superclass) = superclass {
                let mut scope = TypeParameterScope::default();
                if matches!(
                    declaration,
                    TopLevelDecl::Class(_) | TopLevelDecl::MixinClass(_)
                ) {
                    scope.push(declaration.type_params(), model);
                }
                self.type_facts.entry(identity).or_default().superclass =
                    Some(model.resolve_type_in(superclass, &scope));
            }
        }
    }

    pub(super) fn constructor_inference_entries<'a>(
        program: Program<'a>,
        model: &SemanticModel<'_>,
    ) -> Vec<ConstructorInferenceEntry<'a>> {
        let mut work = Vec::new();
        for declaration in program.declarations() {
            if !model.owns_top_level_declaration(declaration) {
                continue;
            }
            if let TopLevelDecl::ClassTypeAlias(alias) = declaration {
                if let Some(identity) = model.resolve_name(alias.name().name()) {
                    work.push(ConstructorInferenceEntry::Alias {
                        identity,
                        name: alias.name().name(),
                    });
                }
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
            let mut constructors = members
                .constructors()
                .filter(|constructor| {
                    constructor
                        .params
                        .params()
                        .any(|parameter| parameter.initializing().is_some())
                })
                .peekable();
            if constructors.peek().is_none() {
                continue;
            }
            let Some(identity) = model.resolve_name(name) else {
                continue;
            };
            let mut scope = TypeParameterScope::default();
            scope.push(type_parameters, model);
            let superclass = superclass.map(|ty| model.resolve_type_in(ty, &scope));
            for mut constructor in constructors {
                work.push(ConstructorInferenceEntry::Declaration {
                    identity: identity.clone(),
                    superclass: superclass.clone(),
                    constructor_name: constructor
                        .constructor_name
                        .map_or("new", |name| name.name()),
                    params: constructor.params,
                    super_constructor: constructor
                        .initializers
                        .find_map(|initializer| match initializer {
                            falcon_syntax::cst::ConstructorInitializer::SuperCall(call) => {
                                Some(call.call_name().map_or("new", |name| name.name()))
                            }
                            _ => None,
                        })
                        .unwrap_or("new"),
                });
            }
        }

        work
    }

    pub(super) fn resolve_constructor_entry(
        &mut self,
        entry: &ConstructorInferenceEntry<'_>,
        model: &SemanticModel<'_>,
    ) {
        let (identity, superclass, constructor_name, params, super_constructor) = match entry {
            ConstructorInferenceEntry::Alias { name, .. } => {
                self.forward_alias_constructors(name, model);
                return;
            }
            ConstructorInferenceEntry::Declaration {
                identity,
                superclass,
                constructor_name,
                params,
                super_constructor,
            } => (
                identity,
                superclass,
                *constructor_name,
                params,
                *super_constructor,
            ),
        };
        let mut updates = Vec::new();
        for parameter in params.params() {
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
            updates.push((parameter, Some(ty), None));
        }
        if let Some(superclass) = superclass {
            let positional_super = params
                .positional()
                .filter(|parameter| parameter.initializing() == Some(InitializingFormal::Super));
            let named_super = params
                .named()
                .filter(|parameter| parameter.initializing() == Some(InitializingFormal::Super));
            let super_formals = positional_super
                .enumerate()
                .map(|(ordinal, parameter)| (SuperFormal::Positional(ordinal), parameter))
                .chain(
                    named_super
                        .map(|parameter| (SuperFormal::Named(parameter.name().name()), parameter)),
                );
            for (formal, parameter) in super_formals {
                let default = if parameter.default_value().is_none()
                    && matches!(
                        parameter.kind(),
                        ParamKind::OptionalPositional | ParamKind::Named { required: false }
                    ) {
                    self.super_parameter_default(superclass, super_constructor, formal)
                } else {
                    None
                };
                let ty = if parameter.param_type().is_none() {
                    self.resolved_constructor_parameter_type(
                        superclass,
                        super_constructor,
                        formal,
                        model,
                    )
                } else {
                    None
                };
                updates.push((parameter, ty, default));
            }
        }
        if index_fixpoint::inference_deferred() {
            return;
        }
        self.apply_constructor_parameter_updates(identity, constructor_name, *params, updates);
    }

    /// Materialize mixin-application constructors in the alias's parameter
    /// scope. Run alongside super-formal refinement so chained applications
    /// see both inferred parameter types and defaults from the declaring library.
    fn forward_alias_constructors(&mut self, name: &str, model: &SemanticModel<'_>) -> bool {
        let Some(identity) = model.resolve_name(name) else {
            return false;
        };
        let Some(ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity: superclass,
                    arguments,
                    ..
                },
        }) = self
            .type_facts
            .get(&identity)
            .and_then(|facts| facts.superclass.as_ref())
        else {
            return false;
        };
        index_fixpoint::record_inference_read(index_fixpoint::InferenceInputRef::Constructors(
            superclass,
        ));
        let Some(source_facts) = self
            .type_facts
            .get(superclass)
            .and_then(|facts| facts.declaration.as_ref())
        else {
            return false;
        };
        let Some(owner_parameters) = self.type_parameters(&identity) else {
            return false;
        };
        let mixins_have_no_fields = self.mixins_have_no_fields(&identity);
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
            signature.owner_parameters.clone_from(&owner_parameters);
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
        if index_fixpoint::inference_deferred() {
            return false;
        }
        self.publish_alias_constructors(&identity, forwarded)
    }

    /// Resolve the default of the super-constructor parameter before publishing.
    fn super_parameter_default(
        &self,
        superclass: &ResolvedType,
        super_constructor: &str,
        parameter: SuperFormal<'_>,
    ) -> Option<ParameterDefault> {
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    identity: super_identity,
                    ..
                },
        } = superclass
        else {
            return None;
        };
        let super_signature = self.constructor(super_identity, super_constructor)?;
        let super_name = match parameter {
            SuperFormal::Named(name) => Some(name),
            SuperFormal::Positional(ordinal) => super_signature
                .positional_parameter_names
                .get(ordinal)
                .map(String::as_str),
        };
        let default = super_name.and_then(|name| super_signature.defaults.get(name))?;
        Some(match default {
            ParameterDefault::Inherited { .. } => default.clone(),
            _ => ParameterDefault::Inherited {
                owner: super_identity.clone(),
                default: Box::new(default.clone()),
            },
        })
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
        let ResolvedType {
            kind:
                ResolvedTypeKind::Interface {
                    occurrence,
                    identity,
                    arguments,
                    ..
                },
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
            .type_facts
            .get_mut(identity)
            .map(|facts| &mut facts.constructors)
            .and_then(|constructors| constructors.get_mut(constructor_name))
        {
            if matches!(
                parameter.kind(),
                falcon_syntax::cst::ParamKind::Named { .. }
            ) {
                if let Some(current) = signature.named.get_mut(parameter_name.name()) {
                    changed |= refine_inferred_type(current, ty.clone());
                }
            } else if let Some(index) = params
                .positional()
                .position(|candidate| candidate.name().span() == parameter_name.span())
                && let Some(current) = signature.positional.get_mut(index)
            {
                changed |= refine_inferred_type(current, ty.clone());
            }
        }
        if let Some(facts) = self
            .type_facts
            .get_mut(identity)
            .and_then(|facts| facts.declaration.as_mut())
            && let Some(constructor) = facts
                .constructors
                .iter_mut()
                .find(|constructor| constructor.name == constructor_name)
            && let Some(parameter) = constructor
                .parameters
                .iter_mut()
                .find(|candidate| candidate.name == parameter_name.name())
        {
            changed |= refine_inferred_type(&mut parameter.ty, ty);
        }
        if changed {
            index_fixpoint::record_inference_write(|| {
                index_fixpoint::InferenceInput::Constructor(
                    identity.clone(),
                    constructor_name.to_owned(),
                )
            });
            index_fixpoint::record_inference_write(|| {
                index_fixpoint::InferenceInput::Constructors(identity.clone())
            });
        }
        changed
    }

    fn add_const_constructor_bodies(
        &mut self,
        identity: &DeclarationIdentity,
        members: &EffectiveMembers<'_>,
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
        self.type_facts
            .entry(identity.clone())
            .or_default()
            .const_field_initializers = fields;
        self.type_facts
            .entry(identity.clone())
            .or_default()
            .const_constructors = bodies;
    }

    fn add_declaration_facts(
        &mut self,
        name: &str,
        annotations: AstChildren<'_, falcon_syntax::cst::Annotation<'_>>,
        type_parameters: AstChildren<'_, TypeParam<'_>>,
        members: &EffectiveMembers<'_>,
        declaration_kind: (Option<DartType<'_>>, bool),
        model: &SemanticModel<'_>,
    ) {
        let (extension_representation, is_abstract) = declaration_kind;
        let Some(identity) = model.resolve_name(name) else {
            return;
        };
        self.add_const_constructor_bodies(&identity, members, model);
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
                    .map_or_else(|| "new".to_string(), |name| name.name().to_owned()),
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
                                            .map_or(ResolvedType::UNKNOWN, |(_, ty)| ty.clone())
                                    },
                                )
                            })
                            .unwrap_or(ResolvedType::UNKNOWN),
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
        let static_consts = declaration_static_consts(members, &scope, model);
        self.type_facts.entry(identity).or_default().declaration = Some(DeclarationFacts {
            annotations,
            library_target,
            constructors,
            static_consts,
            extension_representation: extension_representation
                .map(|ty| model.resolve_type_in(ty, &scope)),
            is_abstract,
            has_subclass_in_library: false,
        });
    }

    /// The linter's `_hasSubclassInDefiningCompilationUnit`: a class whose
    /// `extends` chain (not `implements`, `with`, or mixin `on` clauses)
    /// reaches a declaration of its own library marks that declaration.
    fn mark_library_subclasses(&mut self) {
        for facts in self.type_facts.local.values_mut() {
            if let Some(declaration) = &mut facts.declaration {
                declaration.has_subclass_in_library = false;
            }
        }
        let mut marked = Vec::new();
        let mut visited = HashSet::new();
        for subtype in self.type_facts.local.keys() {
            let DeclarationIdentity::Library { library, .. } = subtype else {
                continue;
            };
            visited.clear();
            visited.insert(subtype);
            let mut current = self
                .type_facts
                .get(subtype)
                .and_then(|facts| facts.superclass.as_ref());
            while let Some(ResolvedType {
                kind: ResolvedTypeKind::Interface { identity, .. },
            }) = current
            {
                if !visited.insert(identity) {
                    break;
                }
                if matches!(identity, DeclarationIdentity::Library { library: other, .. } if other == library)
                {
                    marked.push(identity.clone());
                }
                current = self
                    .type_facts
                    .get(identity)
                    .and_then(|facts| facts.superclass.as_ref());
            }
        }
        for identity in marked {
            if let Some(facts) = self
                .type_facts
                .get_mut(&identity)
                .and_then(|facts| facts.declaration.as_mut())
            {
                facts.has_subclass_in_library = true;
            }
        }
    }

    fn add_type(
        &mut self,
        name: &str,
        type_parameters: AstChildren<'_, TypeParam<'_>>,
        declaration: TopLevelDecl<'_>,
        members: &EffectiveMembers<'_>,
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
        let bounds: FxHashMap<_, _> = owner_parameters
            .iter()
            .zip(type_parameters)
            .map(|(id, parameter)| {
                (
                    id.clone(),
                    parameter.bound().map_or(ResolvedType::DYNAMIC, |bound| {
                        model.resolve_type_in(bound, &scope)
                    }),
                )
            })
            .collect();
        for id in &owner_parameters {
            self.owner_defaults.insert(
                id.clone(),
                instantiate_parameter_bound(id, &bounds, &mut HashSet::new(), model),
            );
        }
        let field_types = Self::type_field_types(members, &scope, model);
        self.add_type_supertypes(
            declaration,
            &identity,
            is_extension,
            &owner_parameters,
            &scope,
            model,
        );
        let infer_override =
            !is_extension && !matches!(declaration, TopLevelDecl::ExtensionType(_));
        let entries = self.member_entries.entry(identity.clone()).or_default();
        TypeMemberRegistration {
            identity,
            is_extension,
            infer_override,
            scope,
            owner_parameters,
            field_types,
            model,
            entries,
            bound_declared: &mut self.bound_declared,
            type_facts: &mut self.type_facts,
            blocked_field_promotions: &mut self.blocked_field_promotions,
        }
        .register(members);
    }

    fn type_field_types(
        members: &EffectiveMembers<'_>,
        scope: &TypeParameterScope,
        model: &SemanticModel<'_>,
    ) -> HashMap<String, ResolvedType> {
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
                    .map_or(ResolvedType::UNKNOWN, |ty| model.resolve_type_in(ty, scope));
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
                    .map_or(ResolvedType::UNKNOWN, |ty| model.resolve_type_in(ty, scope)),
            );
        }
        field_types
    }

    fn add_type_supertypes(
        &mut self,
        declaration: TopLevelDecl<'_>,
        identity: &DeclarationIdentity,
        is_extension: bool,
        owner_parameters: &[TypeParameterId],
        scope: &TypeParameterScope,
        model: &SemanticModel<'_>,
    ) {
        let (superclass, mixins, constraints, interfaces) = match declaration {
            TopLevelDecl::Class(node) => {
                (node.extends(), node.with_clause(), None, node.implements())
            }
            TopLevelDecl::MixinClass(node) => {
                (node.extends(), node.with_clause(), None, node.implements())
            }
            TopLevelDecl::ClassTypeAlias(node) => (
                Some(node.superclass()),
                node.with_clause(),
                None,
                node.implements(),
            ),
            TopLevelDecl::Mixin(node) => (
                None,
                AstChildren::empty(),
                Some(node.on_clause()),
                node.implements(),
            ),
            TopLevelDecl::Enum(node) => (None, node.with_clause(), None, node.implements()),
            TopLevelDecl::ExtensionType(node) => {
                (None, AstChildren::empty(), None, node.implements())
            }
            _ => (None, AstChildren::empty(), None, AstChildren::empty()),
        };
        let superclass = superclass.map(|ty| model.resolve_type_in(ty, scope));
        let mixins: Vec<_> = mixins.map(|ty| model.resolve_type_in(ty, scope)).collect();
        let constraints: Option<Vec<_>> =
            constraints.map(|types| types.map(|ty| model.resolve_type_in(ty, scope)).collect());
        // Resolve each declared edge once, retaining source order for subtype
        // traversal and reverse mixin order for concrete member lookup.
        let mut resolved_supertypes: Vec<_> = superclass
            .iter()
            .chain(&mixins)
            .chain(constraints.iter().flatten())
            .cloned()
            .chain(interfaces.map(|ty| model.resolve_type_in(ty, scope)))
            .collect();
        let mut resolved_member_supertypes: Vec<_> = mixins
            .iter()
            .rev()
            .chain(superclass.iter())
            .cloned()
            .collect();
        let facts = self.type_facts.entry(identity.clone()).or_default();
        if matches!(declaration, TopLevelDecl::ClassTypeAlias(_)) {
            facts.superclass = superclass;
        }
        // Mixin classes historically expose their with-clause edges only in
        // the traversal lists, not as declaration mixin applications.
        if !matches!(declaration, TopLevelDecl::MixinClass(_)) {
            facts.mixin_applications = mixins;
        }
        facts.mixin_constraints = constraints;
        // Extension types retain only their declared superinterfaces: their
        // representation may admit null, so an implicit Object edge is unsound.
        let inherits_object = !is_extension
            && *identity != DeclarationIdentity::OBJECT
            && self.type_kind(identity) != Some(TypeKind::ExtensionType);
        if inherits_object {
            resolved_supertypes.push(model.core_type("Object", false));
        }
        self.type_facts
            .entry(identity.clone())
            .or_default()
            .supertypes = Some((owner_parameters.to_vec(), resolved_supertypes));
        if inherits_object {
            resolved_member_supertypes.push(model.core_type("Object", false));
        }
        self.type_facts
            .entry(identity.clone())
            .or_default()
            .member_supertypes = Some(resolved_member_supertypes);
    }
}

struct TypeMemberRegistration<'a, 's> {
    identity: DeclarationIdentity,
    is_extension: bool,
    infer_override: bool,
    scope: TypeParameterScope,
    owner_parameters: Vec<TypeParameterId>,
    field_types: HashMap<String, ResolvedType>,
    model: &'s SemanticModel<'a>,
    entries: &'s mut FxHashMap<String, MemberEntry>,
    bound_declared: &'s mut SignatureMap<TypeParameterId, bool>,
    type_facts: &'s mut SignatureMap<DeclarationIdentity, TypeFacts>,
    blocked_field_promotions: &'s mut SignatureMap<LibraryId, FxHashSet<String>>,
}

impl TypeMemberRegistration<'_, '_> {
    fn register(mut self, members: &EffectiveMembers<'_>) {
        for member in members.declared.clone() {
            match member {
                ClassMember::Method(method) => self.add_method(method),
                ClassMember::Constructor(constructor) => self.add_constructor(constructor),
                ClassMember::Field(field) => self.add_field(field),
                ClassMember::Getter(getter) => self.add_getter(getter),
                ClassMember::Setter(setter) => self.add_setter(setter),
                ClassMember::Operator(operator) => self.add_operator(operator),
                // Indexed through the constructor it completes (`element_members`).
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => {}
            }
        }
        self.add_declaring_fields(members);
        self.add_primary_constructor(members);
        self.record_blocked_promotions();
    }

    fn add_method(&mut self, method: falcon_syntax::cst::MethodDecl<'_>) {
        let entry = self
            .entries
            .entry(method.name().name().to_string())
            .or_default();
        self.scope.push(method.type_params(), self.model);
        record_bound_declarations(self.bound_declared, &self.scope, method.type_params());
        entry.override_shape = Some(inherited::OverrideShape::new(
            method.params(),
            method.type_params(),
            &self.scope,
        ));
        entry.is_instance_method |= !method.is_static();
        let mut facts = MemberFacts {
            declaring_type: self.identity.clone(),
            kind: MemberKind::Method,
            is_static: method.is_static(),
            // An `external` method has no body but is concrete.
            is_abstract: method.is_abstract() && !method.is_external(),
            positional_parameter_names: Vec::new(),
            annotations: resolve_annotations(method.annotations(), self.model),
            parameter_contracts: None,
        };
        entry.signature = Some(signature(
            &self.scope,
            &self.owner_parameters,
            method.type_params(),
            Some(method.params()),
            method.return_type(),
            SignatureContext::Member(&mut facts, self.infer_override),
            self.model,
        ));
        entry.facts.push(facts);
        self.scope.pop();
    }

    fn add_constructor(&mut self, constructor: falcon_syntax::cst::ConstructorDecl<'_>) {
        self.type_facts
            .entry(self.identity.clone())
            .or_default()
            .constructors
            .insert(
                constructor
                    .constructor_name()
                    .map_or_else(|| "new".to_string(), |name| name.name().to_string()),
                signature(
                    &self.scope,
                    &self.owner_parameters,
                    AstChildren::empty(),
                    Some(constructor.params()),
                    None,
                    SignatureContext::Constructor(&self.field_types),
                    self.model,
                ),
            );
    }

    fn add_field(&mut self, field: falcon_syntax::cst::FieldDecl<'_>) {
        let field_type = field.field_type().map_or(ResolvedType::UNKNOWN, |ty| {
            self.model.resolve_type_in(ty, &self.scope)
        });
        for declarator in field.declarators() {
            let kind = MemberKind::field(field, declarator);
            let entry = self
                .entries
                .entry(declarator.name().name().to_string())
                .or_default();
            entry.promotable_field = declarator.name().name().starts_with('_')
                && !field.is_static()
                && !field.is_abstract()
                && !field.is_external()
                && matches!(field.keyword(), VarKeyword::Final | VarKeyword::Const);
            entry.facts.push(MemberFacts {
                declaring_type: self.identity.clone(),
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
                annotations: resolve_annotations(field.annotations(), self.model),
            });
            entry.field = Some((self.owner_parameters.clone(), field_type.clone()));
            if !field.is_static() {
                entry.read = Some(field_type.clone());
                if kind.has_setter() {
                    entry.write = Some(field_type.clone());
                }
            }
        }
    }

    fn add_getter(&mut self, getter: falcon_syntax::cst::GetterDecl<'_>) {
        let entry = self
            .entries
            .entry(getter.name().name().to_string())
            .or_default();
        entry.facts.push(MemberFacts {
            declaring_type: self.identity.clone(),
            kind: MemberKind::Getter,
            is_static: getter.is_static(),
            is_abstract: getter.is_abstract(),
            positional_parameter_names: Vec::new(),
            parameter_contracts: Some(empty_parameter_contracts()),
            annotations: resolve_annotations(getter.annotations(), self.model),
        });
        let return_type = getter
            .return_type()
            .map(|ty| self.model.resolve_type_in(ty, &self.scope))
            .unwrap_or(if self.infer_override && !getter.is_static() {
                ResolvedType::UNKNOWN
            } else {
                ResolvedType::DYNAMIC
            });
        entry.field = Some((self.owner_parameters.clone(), return_type.clone()));
        entry.implicit_getter = getter.return_type().is_none();
        if !getter.is_static() {
            entry.read = Some(return_type);
        }
    }

    fn add_setter(&mut self, setter: falcon_syntax::cst::SetterDecl<'_>) {
        let entry = self
            .entries
            .entry(setter.name().name().to_string())
            .or_default();
        entry.facts.push(MemberFacts {
            declaring_type: self.identity.clone(),
            kind: MemberKind::Setter,
            is_static: setter.is_static(),
            is_abstract: setter.is_abstract(),
            positional_parameter_names: vec![setter.param().name().to_string()],
            parameter_contracts: Some(setter_parameter_contracts(
                setter.param().name(),
                setter.is_covariant(),
            )),
            annotations: resolve_annotations(setter.annotations(), self.model),
        });
        let parameter_type = setter
            .param_type()
            .map(|ty| self.model.resolve_type_in(ty, &self.scope))
            .unwrap_or(if setter.is_static() || !self.infer_override {
                ResolvedType::DYNAMIC
            } else {
                ResolvedType::UNKNOWN
            });
        entry.field.get_or_insert_with(|| {
            let read_type = if setter.param_type().is_none() {
                ResolvedType::DYNAMIC
            } else {
                parameter_type.clone()
            };
            (self.owner_parameters.clone(), read_type)
        });
        if !setter.is_static() {
            entry.write = Some(parameter_type);
        }
    }

    fn add_operator(&mut self, operator: falcon_syntax::cst::OperatorDecl<'_>) {
        let entry = self
            .entries
            .entry(operator.lookup_name().to_owned())
            .or_default();
        entry.is_instance_method = true;
        entry.override_shape = Some(inherited::OverrideShape::new(
            operator.params(),
            AstChildren::empty(),
            &self.scope,
        ));
        let mut facts = MemberFacts {
            declaring_type: self.identity.clone(),
            kind: MemberKind::Operator,
            is_static: false,
            is_abstract: operator.body().is_none() && !operator.is_external(),
            positional_parameter_names: Vec::new(),
            annotations: resolve_annotations(operator.annotations(), self.model),
            parameter_contracts: None,
        };
        entry.signature = Some(signature(
            &self.scope,
            &self.owner_parameters,
            AstChildren::empty(),
            Some(operator.params()),
            operator.return_type(),
            SignatureContext::Member(&mut facts, self.infer_override),
            self.model,
        ));
        entry.facts.push(facts);
    }

    fn add_declaring_fields(&mut self, members: &EffectiveMembers<'_>) {
        for parameter in members.declaring_params() {
            let is_final = members.representation || parameter.is_final();
            let field_type = self.field_types[parameter.name().name()].clone();
            let entry = self
                .entries
                .entry(parameter.name().name().to_string())
                .or_default();
            entry.promotable_field = parameter.name().name().starts_with('_') && is_final;
            entry.facts.push(MemberFacts {
                declaring_type: self.identity.clone(),
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
                annotations: resolve_annotations(parameter.annotations(), self.model),
            });
            entry.field = Some((self.owner_parameters.clone(), field_type.clone()));
            entry.read = Some(field_type.clone());
            if !is_final {
                entry.write = Some(field_type);
            }
        }
    }

    fn add_primary_constructor(&mut self, members: &EffectiveMembers<'_>) {
        if let Some(primary) = members.primary {
            let mut primary_signature = signature(
                &self.scope,
                &self.owner_parameters,
                AstChildren::empty(),
                Some(primary.params()),
                None,
                SignatureContext::Constructor(&self.field_types),
                self.model,
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
            self.type_facts
                .entry(self.identity.clone())
                .or_default()
                .constructors
                .insert(
                    primary
                        .constructor_name()
                        .map_or_else(|| "new".to_string(), |name| name.name().to_string()),
                    primary_signature,
                );
        }
    }

    fn record_blocked_promotions(self) {
        if let DeclarationIdentity::Library { library, .. } = self.identity
            && !self.is_extension
        {
            for (name, entry) in self.entries {
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
        .chain(args.named().map(falcon_syntax::cst::NamedArg::value))
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
            .map_or(ResolvedType::UNKNOWN, |(identity, _)| {
                ResolvedType::interface_identity(identity)
            }),
        Expr::Conditional(conditional) => {
            let left = infer_const_initializer_type(conditional.then_expr(), model, scope);
            let right = infer_const_initializer_type(conditional.else_expr(), model, scope);
            if left == right {
                left
            } else {
                ResolvedType::UNKNOWN
            }
        }
        Expr::IntLit(_) => model.core_type("int", false),
        Expr::DoubleLit(_) => model.core_type("double", false),
        Expr::StringLit(_) => model.core_type("String", false),
        Expr::BoolLit(_) => model.core_type("bool", false),
        _ => ResolvedType::UNKNOWN,
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
    Member(&'a mut MemberFacts, bool),
}

fn extension_annotations(
    extension: falcon_syntax::cst::ExtensionDecl<'_>,
    model: &SemanticModel<'_>,
) -> FxHashMap<String, Vec<DeclarationIdentity>> {
    extension
        .members()
        .filter_map(|member| {
            let (name, annotations) = match member {
                ClassMember::Method(method) => (method.name().name(), method.annotations()),
                ClassMember::Getter(getter) => (getter.name().name(), getter.annotations()),
                ClassMember::Operator(operator) => (operator.lookup_name(), operator.annotations()),
                _ => return None,
            };
            let annotations = resolve_annotations(annotations, model);
            (!annotations.is_empty()).then(|| (name.to_owned(), annotations))
        })
        .collect()
}

fn declaration_static_consts(
    members: &EffectiveMembers<'_>,
    scope: &TypeParameterScope,
    model: &SemanticModel<'_>,
) -> Vec<StaticConstFacts> {
    members
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
                .map(|ty| model.resolve_type_in(ty, scope));
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
        .collect()
}

fn parameter_requires_default(param: FormalParam<'_>, optional: bool) -> bool {
    !optional
        || param.annotations().any(|annotation| {
            annotation
                .name()
                .last()
                .is_some_and(|name| name.name() == "required")
        })
}

fn signature_parameter_type(
    param: FormalParam<'_>,
    initializing: Option<InitializingFormal>,
    name: &str,
    field_types: Option<&HashMap<String, ResolvedType>>,
    infer_override: bool,
    scope: &TypeParameterScope,
    model: &SemanticModel<'_>,
) -> ResolvedType {
    if param.param_type().is_some() || param.function_params().is_some() {
        model.resolve_formal_parameter_in(param, scope)
    } else if initializing == Some(InitializingFormal::This) {
        field_types
            .and_then(|fields| fields.get(name))
            .cloned()
            .unwrap_or(ResolvedType::UNKNOWN)
    } else if initializing == Some(InitializingFormal::Super) || infer_override {
        ResolvedType::UNKNOWN
    } else {
        ResolvedType::DYNAMIC
    }
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
    let (field_types, mut facts, infer_override) = match context {
        SignatureContext::Function => (None, None, false),
        SignatureContext::Constructor(fields) => (Some(fields), None, false),
        SignatureContext::Member(facts, infer_override) => (None, Some(facts), infer_override),
    };
    let infer_override = infer_override && facts.as_ref().is_some_and(|facts| !facts.is_static);
    let mut result = ResolvedSignature::returning(ResolvedType::DYNAMIC);
    result.owner_parameters = owner_parameters.to_vec();
    result.call_parameters = parameter_ids(scope, call_parameters.clone());
    let mut required_named = Vec::new();
    let mut named_parameter_names = Vec::new();
    let mut contracts = Vec::new();
    for param in params
        .into_iter()
        .flat_map(falcon_syntax::cst::FormalParamList::params)
    {
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
        let ty = signature_parameter_type(
            param,
            initializing,
            name,
            field_types,
            infer_override,
            scope,
            model,
        );
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
        if has_default && parameter_requires_default(param, optional) {
            result.non_optional_defaults.insert(name.to_string());
        }
        // Super formals without a declared default inherit theirs once the super
        // constructor resolves (`resolve_constructor_entry`).
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
        .unwrap_or(if infer_override {
            ResolvedType::UNKNOWN
        } else {
            ResolvedType::DYNAMIC
        });
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
                            ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { id, .. } |
                ResolvedTypeKind::NonNullTypeParameter { id, .. } } if parameters.contains(id)
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
            Some(ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { id, .. },
            }) => Some(id),
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
        if let Some(ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { id, .. },
        }) = scope.lookup(parameter.name().name())
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
