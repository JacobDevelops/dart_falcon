use super::*;

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

#[derive(Debug, Clone, Default)]
pub struct TypeParameterScope {
    scopes: Vec<HashMap<Arc<str>, ResolvedType>>,
}

impl TypeParameterScope {
    pub fn push<'tree>(
        &mut self,
        params: impl Iterator<Item = TypeParam<'tree>> + Clone,
        model: &SemanticModel<'_>,
    ) {
        let owner = ParameterOwner::Source {
            file: model.file,
            offset: params.clone().next().map_or(0, |param| param.span().start),
        };
        self.scopes.push(HashMap::new());
        for (ordinal, param) in params.clone().enumerate() {
            let name: Arc<str> = param.name().name().into();
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
            let Some(ResolvedType::TypeParameter { id, .. }) = self.lookup(param.name().name())
            else {
                unreachable!("registered type parameter");
            };
            let bound = param
                .bound()
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
    pub(super) file: Option<FileId>,
    pub(super) library: Option<LibraryId>,
    parsed: Option<&'a Arc<crate::ParsedFile>>,
    pub(super) identities: &'a IdentityIndex,
    pub(super) types: Option<&'a TypeIndex>,
}

impl<'a> SemanticModel<'a> {
    pub fn new(
        file: &Path,
        parsed: &'a Arc<crate::ParsedFile>,
        identities: &'a IdentityIndex,
        types: Option<&'a TypeIndex>,
    ) -> Self {
        Self::from_file(identities.file(file), parsed, identities, types)
    }

    /// Resolve in an indexed file's instance rather than its primary path instance.
    pub(crate) fn from_file(
        file: Option<FileId>,
        parsed: &'a Arc<crate::ParsedFile>,
        identities: &'a IdentityIndex,
        types: Option<&'a TypeIndex>,
    ) -> Self {
        Self {
            file,
            library: file.map(|file| identities.library(file)),
            parsed: Some(parsed),
            identities,
            types,
        }
    }

    pub(super) fn node_ref(&self, node: falcon_syntax::cst::SyntaxNode<'_>) -> crate::NodeRef {
        let parsed = self
            .parsed
            .expect("syntax retention requires a source-backed model");
        let retained = crate::NodeRef::new(Arc::clone(parsed), node.id());
        debug_assert!(std::ptr::eq(node.tree(), &retained.file.tree));
        retained
    }

    pub(super) fn owns_declaration(&self, name: &str) -> bool {
        self.library
            .is_none_or(|library| self.identities.is_authoritative_declaration(library, name))
    }

    pub(super) fn owns_top_level_declaration(&self, declaration: TopLevelDecl<'_>) -> bool {
        let name = match declaration {
            TopLevelDecl::Class(node) => node.name(),
            TopLevelDecl::ClassTypeAlias(node) => node.name(),
            TopLevelDecl::Mixin(node) => node.name(),
            TopLevelDecl::MixinClass(node) => node.name(),
            TopLevelDecl::Enum(node) => node.name(),
            TopLevelDecl::Extension(node) => {
                return node
                    .name()
                    .is_none_or(|name| self.owns_declaration(name.name()));
            }
            TopLevelDecl::ExtensionType(node) => node.name(),
            TopLevelDecl::Function(node) => node.name(),
            TopLevelDecl::TypeAlias(node) => node.name(),
            TopLevelDecl::Variable(_) | TopLevelDecl::Error(_) => return true,
        };
        self.owns_declaration(name.name())
    }

    pub fn identities(&self) -> &'a IdentityIndex {
        self.identities
    }

    /// The `package:` URI of the library declaring an identity.
    pub fn declaration_uri(&self, identity: &DeclarationIdentity) -> Option<&'a str> {
        let DeclarationIdentity::Library { library, .. } = identity else {
            return None;
        };
        self.identities.library_uri(*library)
    }

    pub(crate) fn for_declaration(
        &self,
        identity: &DeclarationIdentity,
    ) -> Option<SemanticModel<'a>> {
        let library = self.identities.declaration_library(identity)?;
        let file = self.identities.declaration_file(library, identity.name());
        Some(Self {
            file,
            library: Some(library),
            parsed: file
                .map(|file| self.identities.parsed_file(file))
                .or(self.parsed),
            identities: self.identities,
            types: self.types,
        })
    }

    pub(crate) fn configurable_constant_path(&self, identity: &DeclarationIdentity) -> bool {
        self.library
            .zip(self.identities.declaration_library(identity))
            .is_some_and(|(source, target)| {
                self.identities.configurable_constant_path(source, target)
            })
    }

    /// Views names from a declaring library without inventing a declaration.
    pub fn for_library(&self, library: LibraryId) -> Self {
        Self {
            parsed: self.parsed,
            ..Self::in_library(self.identities, self.types, Some(library))
        }
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
            parsed: None,
            identities,
            types,
        }
    }

    pub fn resolve_type(&self, ty: DartType<'_>) -> ResolvedType {
        self.resolve_type_in(ty, &TypeParameterScope::default())
    }

    /// Resolves ordinary and old-style function formals in their lexical scope.
    pub fn resolve_formal_parameter_in(
        &self,
        param: FormalParam<'_>,
        parameters: &TypeParameterScope,
    ) -> ResolvedType {
        let mut local;
        let parameters = if param.type_params().is_empty() {
            parameters
        } else {
            local = parameters.clone();
            local.push(param.type_params(), self);
            &local
        };
        let return_type = param
            .param_type()
            .map(|ty| self.resolve_type_in(ty, parameters))
            .unwrap_or(ResolvedType::Dynamic);
        let Some(function_params) = param.function_params() else {
            return return_type;
        };
        let mut named: Vec<_> = function_params
            .named()
            .map(|param| {
                (
                    param.name().name().to_owned(),
                    self.resolve_formal_parameter_in(param, parameters),
                )
            })
            .collect();
        named.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
        ResolvedType::Function {
            shape: FunctionTypeShape::from_scope(
                function_params
                    .named()
                    .filter(|param| matches!(param.kind(), ParamKind::Named { required: true }))
                    .map(|param| param.name().name().to_owned())
                    .collect(),
                param.type_params(),
                parameters,
            ),
            optional_positional: function_params
                .positional()
                .filter(|param| param.kind() == ParamKind::OptionalPositional)
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

    pub(super) fn extension_visible(
        &self,
        declaring: Option<LibraryId>,
        name: Option<&str>,
    ) -> bool {
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

    /// Whether metadata denotes the SDK's argument-free `deprecated` constant.
    pub fn is_deprecated_constant(&self, annotation: Annotation<'_>) -> bool {
        annotation.args().is_none()
            && annotation.constructor_name().is_none()
            && QualifiedName::from_identifiers(annotation.name()).is_some_and(|name| {
                self.resolve_sdk_annotation_constant(name)
                    .is_some_and(|identity| identity.is_sdk("dart:core", "deprecated"))
            })
    }

    /// Whether metadata names the SDK's `Deprecated` type, including type aliases.
    pub fn is_deprecated_constructor(&self, annotation: Annotation<'_>) -> bool {
        let names = annotation.name();
        let is_deprecated_type = |name: Option<QualifiedName<'_>>| {
            name.is_some_and(|name| {
                self.resolve_name(name).is_some_and(|identity| {
                    self.unaliased(identity).is_sdk("dart:core", "Deprecated")
                })
            })
        };
        is_deprecated_type(QualifiedName::from_identifiers(names.clone()))
            || (annotation.constructor_name().is_none()
                && names.clone().count() > 1
                && is_deprecated_type(QualifiedName::from_identifiers(
                    names.clone().take(names.count() - 1),
                )))
    }

    /// Whether metadata denotes either resolved SDK deprecation annotation.
    pub fn is_deprecation(&self, annotation: Annotation<'_>) -> bool {
        self.is_deprecated_constant(annotation)
            || (annotation.args().is_some() && self.is_deprecated_constructor(annotation))
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

    /// Resolve an SDK annotation constant, including one const variable alias.
    pub fn resolve_sdk_annotation_constant<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        let name = name.into();
        if let Some(identity) = self.resolve_sdk_member(name) {
            return Some(identity);
        }
        let (owner, member) = match self.resolve_value(name) {
            Some(identity) => (identity, None),
            None => (
                self.unaliased(
                    self.resolve_name(name.prefix?)
                        .or_else(|| self.resolve_extension(name.prefix?))?,
                ),
                Some(name.name),
            ),
        };
        let model = self.for_declaration(&owner)?;
        let program = model.parsed?.program();
        let initializer = program.declarations().find_map(|declaration| {
            let mut declarators = match (declaration, member) {
                (TopLevelDecl::Variable(variable), None)
                    if variable.keyword() == VarKeyword::Const =>
                {
                    variable.declarators()
                }
                (declaration, Some(member)) => {
                    let (declared_name, mut members) = match declaration {
                        TopLevelDecl::Class(node) => (node.name(), node.members()),
                        TopLevelDecl::Mixin(node) => (node.name(), node.members()),
                        TopLevelDecl::MixinClass(node) => (node.name(), node.members()),
                        TopLevelDecl::Enum(node) => (node.name(), node.members()),
                        TopLevelDecl::ExtensionType(node) => (node.name(), node.members()),
                        TopLevelDecl::Extension(node) => (node.name()?, node.members()),
                        _ => return None,
                    };
                    if declared_name.name() != owner.name() {
                        return None;
                    }
                    let initializer = members.clone().find_map(|member_node| {
                        let ClassMember::Field(field) = member_node else {
                            return None;
                        };
                        if !field.is_static() || field.keyword() != VarKeyword::Const {
                            return None;
                        }
                        field.declarators().find_map(|declarator| {
                            (declarator.name().name() == member)
                                .then(|| declarator.initializer())
                                .flatten()
                        })
                    })?;
                    let name = initializer_value_name(initializer)?;
                    let root = name.prefix.unwrap_or(name.name);
                    if let TopLevelDecl::Enum(enumeration) = declaration
                        && enumeration
                            .variants()
                            .any(|variant| variant.name().name() == root)
                    {
                        return None;
                    }
                    if members.any(|member| match member {
                        ClassMember::Field(field) => field
                            .declarators()
                            .any(|declarator| declarator.name().name() == root),
                        ClassMember::Method(method) => method.name().name() == root,
                        ClassMember::Getter(getter) => getter.name().name() == root,
                        ClassMember::Setter(setter) => setter.name().name() == root,
                        _ => false,
                    }) {
                        return None;
                    }
                    return Some(initializer);
                }
                _ => return None,
            };
            declarators.find_map(|declarator| {
                (declarator.name().name() == owner.name())
                    .then(|| declarator.initializer())
                    .flatten()
            })
        })?;
        model.resolve_sdk_member(initializer_value_name(initializer)?)
    }

    pub fn resolve_imported_member<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_imported_member(self.library?, name.into())
    }

    /// Resolve imported annotations before generation when only generated parts are missing.
    pub fn resolve_annotation_before_generation<'n>(
        &self,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_annotation_before_generation(self.library?, name.into())
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
            .map(|alias| alias.type_params().len())
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

    pub fn resolve_type_in(
        &self,
        ty: DartType<'_>,
        parameters: &TypeParameterScope,
    ) -> ResolvedType {
        match ty {
            DartType::Void(_) => ResolvedType::Void,
            DartType::Dynamic(_) => ResolvedType::Dynamic,
            DartType::Named(named) => {
                let mut segments = named.segments();
                if let Some(segment) = segments.next()
                    && segments.next().is_none()
                    && let Some(parameter) = parameters.lookup(segment.name())
                {
                    return parameter.with_nullable(parameter.nullable() || named.is_nullable());
                }
                let Some(identity) = QualifiedName::from_identifiers(named.segments())
                    .and_then(|name| self.resolve_name(name))
                else {
                    return ResolvedType::Unknown;
                };
                if identity.is_sdk("dart:core", "Null") {
                    return ResolvedType::Null;
                }
                if identity.is_sdk("dart:core", "Never") {
                    return if named.is_nullable() {
                        ResolvedType::Null
                    } else {
                        ResolvedType::Never
                    };
                }
                if let Some(alias) = self.identities.type_alias(&identity) {
                    let arguments: Vec<_> = named
                        .type_args()
                        .map(|argument| self.resolve_type_in(argument, parameters))
                        .collect();
                    return self.expand_alias(
                        &identity,
                        alias,
                        &arguments,
                        named.is_nullable(),
                        Some((self.file, named.span().start)),
                    );
                }
                let extension_type = self.types.is_some_and(|types| {
                    types.type_kind_identity(&identity) == Some(super::TypeKind::ExtensionType)
                });
                ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity,
                    arguments: if named.type_args().is_empty() {
                        empty_type_arguments()
                    } else {
                        named
                            .type_args()
                            .map(|argument| self.resolve_type_in(argument, parameters))
                            .collect()
                    },
                    nullable: named.is_nullable(),
                    extension_type,
                }
            }
            DartType::Function(function) => {
                let mut function_parameters = parameters.clone();
                function_parameters.push(function.type_params(), self);
                let mut named: Vec<_> = function
                    .params()
                    .filter(|param| param.is_named())
                    .filter_map(|param| {
                        param.name().map(|name| {
                            (
                                name.name().to_owned(),
                                self.resolve_type_in(param.param_type(), &function_parameters),
                            )
                        })
                    })
                    .collect();
                named.sort_by(|left, right| left.0.cmp(&right.0));
                ResolvedType::Function {
                    shape: FunctionTypeShape::from_scope(
                        function
                            .params()
                            .filter(|param| param.is_named() && param.is_required())
                            .filter_map(|param| param.name().map(|name| name.name().to_owned()))
                            .collect(),
                        function.type_params(),
                        &function_parameters,
                    ),
                    optional_positional: function
                        .params()
                        .filter(|param| !param.is_named() && !param.is_required())
                        .count(),
                    alias: FunctionTypeAlias::default(),
                    return_type: Arc::new(
                        function
                            .return_type()
                            .map(|ty| self.resolve_type_in(ty, &function_parameters))
                            .unwrap_or(ResolvedType::Dynamic),
                    ),
                    positional: if function.params().all(|param| param.is_named()) {
                        empty_type_arguments()
                    } else {
                        function
                            .params()
                            .filter(|param| !param.is_named())
                            .map(|param| {
                                self.resolve_type_in(param.param_type(), &function_parameters)
                            })
                            .collect()
                    },
                    named: if named.is_empty() {
                        empty_named_types()
                    } else {
                        named.into()
                    },
                    nullable: function.is_nullable(),
                }
            }
            DartType::Record(record) => {
                let mut named: Vec<_> = record
                    .named()
                    .map(|field| {
                        (
                            field.name().name().to_owned(),
                            self.resolve_type_in(field.field_type(), parameters),
                        )
                    })
                    .collect();
                named.sort_by(|left, right| left.0.cmp(&right.0));
                ResolvedType::Record {
                    occurrence: Default::default(),
                    positional: if record.positional().is_empty() {
                        empty_type_arguments()
                    } else {
                        record
                            .positional()
                            .map(|field| self.resolve_type_in(field, parameters))
                            .collect()
                    },
                    named: if named.is_empty() {
                        empty_named_types()
                    } else {
                        named.into()
                    },
                    nullable: record.is_nullable(),
                }
            }
        }
    }

    /// The type a use of `alias` with `arguments` denotes: the aliased type,
    /// resolved in the alias's own library, with its type parameters replaced
    /// by the arguments (or, when omitted, by their bounds, as instantiation
    /// to bounds does), made nullable by a `?` on the use. Alias cycles
    /// (invalid Dart) resolve to `Unknown`.
    pub(super) fn expand_alias(
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
    pub(super) fn alias_template(&self, alias: &TypeAlias) -> Option<Arc<AliasTemplate>> {
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
            parsed: Some(self.identities.parsed_file(alias.file)),
            identities: self.identities,
            types: self.types,
        };
        let mut scope = TypeParameterScope::default();
        scope.push(alias.type_params(), &model);
        let aliased = model.resolve_type_in(alias.aliased(), &scope);
        let mut parameters: Vec<_> = alias
            .type_params()
            .enumerate()
            .filter_map(|(ordinal, parameter)| {
                let ResolvedType::TypeParameter { id, bound, .. } =
                    scope.lookup(parameter.name().name())?
                else {
                    return None;
                };
                let default = if parameter.bound().is_some() {
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
                    types.supertype_matches(
                        identity,
                        |ancestor| ancestor.is_sdk("dart:async", "Future"),
                        |name| name == "Future",
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

    pub(super) fn substitute_memoized<'t>(
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

    pub(super) fn structural_unrelated(&self, left: &ResolvedType, right: &ResolvedType) -> Truth {
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
                        .any(|(left, right)| self.structural_unrelated(left, right) == Truth::Yes)
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
                    types.supertype_matches(
                        left_id,
                        |identity| identity.matches(right_id),
                        |name| name == right_id.name(),
                    ),
                    types.supertype_matches(
                        right_id,
                        |identity| identity.matches(left_id),
                        |name| name == left_id.name(),
                    ),
                ) {
                    (Truth::Yes, _) | (_, Truth::Yes) => Truth::No,
                    (Truth::No, Truth::No) => Truth::Yes,
                    _ => Truth::Unknown,
                }
            }
            (ResolvedType::NonNullTypeParameter { bound, .. }, other)
            | (other, ResolvedType::NonNullTypeParameter { bound, .. }) => {
                self.structural_unrelated(&bound.non_null(), other)
            }
            (ResolvedType::TypeParameter { bound, .. }, other)
            | (other, ResolvedType::TypeParameter { bound, .. }) => {
                self.structural_unrelated(bound, other)
            }
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
                    && self.structural_unrelated(left_return, right_return) == Truth::Yes
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
                    .any(|(left, right)| self.structural_unrelated(left, right) == Truth::Yes)
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
