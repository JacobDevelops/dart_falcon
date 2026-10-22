use super::*;

/// Declaration spelling facts that are not part of a parameter's resolved type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParameterContract {
    pub name: String,
    pub kind: ParamKind,
    pub is_covariant: bool,
    /// Span- and trivia-independent syntax of an explicit default; absent for implicit null.
    pub default_value_code: Option<DefaultSyntaxKey>,
}

pub(super) fn empty_parameter_contracts() -> Arc<[ParameterContract]> {
    static EMPTY: std::sync::LazyLock<Arc<[ParameterContract]>> =
        std::sync::LazyLock::new(|| Arc::from([]));
    Arc::clone(&EMPTY)
}

pub(super) fn setter_parameter_contracts(
    name: &str,
    is_covariant: bool,
) -> Arc<[ParameterContract]> {
    Arc::from([ParameterContract {
        name: name.to_string(),
        kind: ParamKind::Positional,
        is_covariant,
        default_value_code: None,
    }])
}

fn empty_parameter_names() -> Arc<[String]> {
    static EMPTY: std::sync::LazyLock<Arc<[String]>> = std::sync::LazyLock::new(|| Arc::from([]));
    Arc::clone(&EMPTY)
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
    /// Named parameters in declaration order; empty when that order is unavailable.
    pub named_parameter_names: Arc<[String]>,
    /// Default-bearing parameters that are required or annotated `@required`.
    pub non_optional_defaults: HashSet<String>,
    pub named: HashMap<String, ResolvedType>,
    pub return_type: ResolvedType,
}

impl ResolvedSignature {
    pub(super) fn returning(return_type: ResolvedType) -> Self {
        Self {
            owner_parameters: Vec::new(),
            call_parameters: Vec::new(),
            shape: None,
            optional_positional: 0,
            defaults: HashMap::new(),
            positional_parameter_names: Vec::new(),
            named_parameter_names: empty_parameter_names(),
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
        type_params: AstChildren<'_, TypeParam<'_>>,
        params: FormalParamList<'_>,
        model: &SemanticModel<'_>,
    ) -> Self {
        signature(
            scope,
            &[],
            type_params,
            Some(params),
            None,
            SignatureContext::Function,
            model,
        )
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
    Declared(crate::NodeRef),
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
pub(super) enum SuperFormal<'a> {
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
    /// Whether this is a `this.x` formal or a primary constructor's `var`/`final` field.
    pub is_field: bool,
}

#[derive(Debug, Clone)]
pub struct StaticConstFacts {
    pub name: String,
    pub ty: ResolvedType,
    pub initializer: crate::NodeRef,
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
pub(super) struct ExtensionFacts {
    pub(super) declaring_library: Option<LibraryId>,
    pub(super) name: Option<String>,
    /// The declaration identity of a named extension.
    pub(super) identity: Option<DeclarationIdentity>,
    pub(super) on_type: ResolvedType,
    /// The extension's own type parameters.
    pub(super) parameters: Vec<TypeParameterId>,
    pub(super) parameter_bounds: HashMap<TypeParameterId, ResolvedType>,
    /// Signatures of the instance methods and operators, over
    /// [`Self::parameters`].
    pub(super) methods: FxHashMap<String, ResolvedSignature>,
    /// Resolved annotations of executable extension members.
    pub(super) annotations: FxHashMap<String, Vec<DeclarationIdentity>>,
    /// Return types of the instance getters, over [`Self::parameters`].
    pub(super) getters: FxHashMap<String, ResolvedType>,
    pub(super) members: FxHashSet<String>,
    /// Instance methods, getters, setters (as `name=`), and operators.
    pub(super) instance_members: FxHashSet<String>,
}

/// Owns the indexed spelling once, while borrowing its unencoded member name.
#[derive(Debug, Clone, Eq, PartialEq)]
pub(super) struct ExtensionMemberName(pub(super) String);

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

pub(super) type ExtensionMembers = [FxHashMap<ExtensionMemberName, Vec<usize>>; 2];

#[derive(Debug, Clone, Copy)]
pub(super) enum InstanceAccess {
    Read,
    Write,
    Invoke,
}

#[derive(Debug, Clone, Default)]
pub(super) struct MemberEntry {
    pub(super) signature: Option<ResolvedSignature>,
    pub(super) override_shape: Option<inherited::OverrideShape>,
    pub(super) is_instance_method: bool,
    pub(super) field: Option<(Vec<TypeParameterId>, ResolvedType)>,
    pub(super) read: Option<ResolvedType>,
    pub(super) write: Option<ResolvedType>,
    pub(super) facts: Vec<MemberFacts>,
    pub(super) promotable_field: bool,
    pub(super) implicit_getter: bool,
}

/// The parts of a `const` constructor that constant evaluation runs.
#[derive(Debug, Clone)]
pub struct ConstConstructorBody {
    pub is_primary: bool,
    pub representation: bool,
    pub params: Vec<crate::NodeRef>,
    pub initializers: Vec<crate::NodeRef>,
    pub redirect: Option<crate::NodeRef>,
}

type ExternalSignatureMap<K, V> = (
    Arc<SignatureIndex>,
    fn(&SignatureIndex) -> &SignatureMap<K, V>,
);

/// Project-owned entries with a borrowed, immutable external table. Mutations
/// always target the project table; a declaration shadows its base entry whole.
#[derive(Clone)]
pub(super) struct SignatureMap<K, V> {
    pub(super) local: FxHashMap<K, V>,
    pub(super) external: Option<ExternalSignatureMap<K, V>>,
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
    pub(super) fn get<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
    {
        self.local.get(key).or_else(|| {
            let (base, field) = self.external.as_ref()?;
            field(base).get(key)
        })
    }

    pub(super) fn contains_key<Q: ?Sized + Eq + Hash>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
    {
        self.get(key).is_some()
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
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
            $($(#[$local_doc])* pub(super) $local: $local_type,)*
            $($(#[$shared_doc])* pub(super) $shared: $shared_type,)*
            /// Only external-only snapshots may be retained here, never project facts.
            pub(super) external: Option<Arc<SignatureIndex>>,
        }

        impl SignatureIndex {
            pub(super) fn borrowing_external(external: Arc<Self>) -> Self {
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
        /// Generic owner parameters for curated identities without hierarchy facts.
        fallback_type_parameters: SignatureMap<DeclarationIdentity, Vec<TypeParameterId>>,
        owner_defaults: SignatureMap<TypeParameterId, ResolvedType>,
        /// Whether the declaration of each type parameter of an indexed type,
        /// alias, function or method states a bound; parameters of local
        /// functions are absent.
        bound_declared: SignatureMap<TypeParameterId, bool>,
        member_supertypes: SignatureMap<DeclarationIdentity, Vec<ResolvedType>>,
        declarations: SignatureMap<DeclarationIdentity, DeclarationFacts>,
        constants: SignatureMap<DeclarationIdentity, crate::NodeRef>,
        constant_types: SignatureMap<DeclarationIdentity, DeclarationIdentity>,
        enum_values: SignatureMap<DeclarationIdentity, Vec<String>>,
        /// Bodies of project `const` constructors, keyed by class and name.
        const_constructors: SignatureMap<DeclarationIdentity, FxHashMap<String, ConstConstructorBody>>,
        /// Instance field initializers of project classes with a `const`
        /// constructor, in declaration order.
        const_field_initializers: SignatureMap<DeclarationIdentity, Vec<(String, crate::NodeRef)>>,
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
