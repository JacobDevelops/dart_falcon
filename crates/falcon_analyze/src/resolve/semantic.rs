//! Canonical semantic facts shared by resolver-dependent rules.
//!
//! This is deliberately conservative: every unresolved or ambiguous identity
//! becomes `Unknown`, and consumers diagnose only from proven facts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use falcon_syntax::ast::{
    BinaryOp, ClassMember, DartType, Expr, FormalParam, FormalParamList, FunctionBody,
    InitializingFormal, Program, TopLevelDecl, TypeParam, VarKeyword,
};

use super::{
    DeclarationIdentity, FileId, IdentityIndex, LibraryId, MemberKind, NameIdentity, QualifiedName,
    Truth, TypeIndex,
};

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
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedType {
    Unknown,
    Dynamic,
    Void,
    Never,
    Null,
    Interface {
        identity: DeclarationIdentity,
        arguments: Vec<ResolvedType>,
        nullable: bool,
        extension_type: bool,
    },
    TypeParameter {
        id: TypeParameterId,
        bound: Box<ResolvedType>,
        nullable: bool,
    },
    Function {
        return_type: Box<ResolvedType>,
        positional: Vec<ResolvedType>,
        named: Vec<(String, ResolvedType)>,
        nullable: bool,
    },
    Record {
        positional: Vec<ResolvedType>,
        named: Vec<(String, ResolvedType)>,
        nullable: bool,
    },
}

impl ResolvedType {
    pub fn interface_identity(identity: DeclarationIdentity) -> Self {
        Self::Interface {
            identity,
            arguments: Vec::new(),
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
            Self::Void | Self::Never => false,
        }
    }

    pub fn with_nullable(&self, nullable: bool) -> Self {
        match self {
            Self::Interface {
                identity,
                arguments,
                extension_type,
                ..
            } => Self::Interface {
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
            Self::Function {
                return_type,
                positional,
                named,
                ..
            } => Self::Function {
                return_type: return_type.clone(),
                positional: positional.clone(),
                named: named.clone(),
                nullable,
            },
            Self::Record {
                positional, named, ..
            } => Self::Record {
                positional: positional.clone(),
                named: named.clone(),
                nullable,
            },
            Self::Never if nullable => Self::Null,
            other => other.clone(),
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
    scopes: Vec<HashMap<String, ResolvedType>>,
}

impl TypeParameterScope {
    pub fn push(&mut self, params: &[TypeParam], model: &SemanticModel<'_>) {
        let owner = ParameterOwner::Source {
            file: model.file,
            offset: params.first().map_or(0, |param| param.span.start),
        };
        self.scopes.push(HashMap::new());
        for (ordinal, param) in params.iter().enumerate() {
            self.scopes
                .last_mut()
                .expect("type-parameter scope")
                .insert(
                    param.name.name.clone(),
                    ResolvedType::TypeParameter {
                        id: TypeParameterId {
                            owner,
                            ordinal,
                            name: param.name.name.clone(),
                        },
                        bound: Box::new(model.core_type("Object", true)),
                        nullable: false,
                    },
                );
        }
        for (ordinal, param) in params.iter().enumerate() {
            let id = TypeParameterId {
                owner,
                ordinal,
                name: param.name.name.clone(),
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
                    param.name.name.clone(),
                    ResolvedType::TypeParameter {
                        id,
                        bound: Box::new(bound),
                        nullable: false,
                    },
                );
        }
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
        let file = identities.file(file);
        Self {
            file,
            library: file.map(|file| identities.library(file)),
            identities,
            types,
        }
    }

    pub(crate) fn source_path(&self) -> Option<&Path> {
        self.identities.source_path(self.file?)
    }

    pub(crate) fn for_declaration(
        &self,
        identity: &DeclarationIdentity,
    ) -> Option<SemanticModel<'a>> {
        let DeclarationIdentity::Project { library, name } = identity else {
            return None;
        };
        Some(Self {
            file: self.identities.declaration_file(*library, name),
            library: Some(*library),
            identities: self.identities,
            types: self.types,
        })
    }

    pub fn resolve_type(&self, ty: &DartType) -> ResolvedType {
        self.resolve_type_in(ty, &TypeParameterScope::default())
    }

    fn extension_visible(&self, declaring: Option<LibraryId>, name: Option<&str>) -> bool {
        self.library
            .zip(declaring)
            .is_some_and(|(from, declaring)| {
                self.identities.extension_visible(from, declaring, name)
            })
    }

    fn member_visible(&self, declaring_type: &DeclarationIdentity, name: &str) -> bool {
        !name.starts_with('_')
            || matches!(declaring_type, DeclarationIdentity::Project { library, .. }
                if self.library == Some(*library))
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

    pub fn resolve_type_in(&self, ty: &DartType, parameters: &TypeParameterScope) -> ResolvedType {
        match ty {
            DartType::Void { .. } => ResolvedType::Void,
            DartType::Dynamic { .. } => ResolvedType::Dynamic,
            DartType::Named(named) => {
                if let [segment] = named.segments.as_slice()
                    && let Some(parameter) = parameters.lookup(&segment.name)
                {
                    return parameter.with_nullable(named.is_nullable);
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
                let extension_type = matches!(&identity, DeclarationIdentity::Project { name, .. }
                    if self.types.is_some_and(|types| matches!(types.type_kind(name), Some(super::TypeKind::ExtensionType))));
                ResolvedType::Interface {
                    identity,
                    arguments: named
                        .type_args
                        .iter()
                        .map(|argument| self.resolve_type_in(argument, parameters))
                        .collect(),
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
                    return_type: Box::new(
                        function
                            .return_type
                            .as_deref()
                            .map(|ty| self.resolve_type_in(ty, &function_parameters))
                            .unwrap_or(ResolvedType::Dynamic),
                    ),
                    positional: function
                        .params
                        .iter()
                        .filter(|param| !param.is_named)
                        .map(|param| self.resolve_type_in(&param.param_type, &function_parameters))
                        .collect(),
                    named,
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
                    positional: record
                        .positional
                        .iter()
                        .map(|field| self.resolve_type_in(field, parameters))
                        .collect(),
                    named,
                    nullable: record.is_nullable,
                }
            }
        }
    }

    pub fn core_type(&self, name: &'static str, nullable: bool) -> ResolvedType {
        ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk {
                library: "dart:core",
                name,
            },
            arguments: Vec::new(),
            nullable,
            extension_type: false,
        }
    }

    pub fn is_future_like(&self, ty: &ResolvedType) -> Truth {
        match ty {
            ResolvedType::Dynamic | ResolvedType::Null => Truth::Yes,
            ResolvedType::Unknown => Truth::Unknown,
            ResolvedType::Interface {
                extension_type: true,
                ..
            } => Truth::Yes,
            ResolvedType::Interface {
                identity:
                    DeclarationIdentity::Sdk {
                        library: "dart:async",
                        name: "Future" | "FutureOr",
                    },
                ..
            } => Truth::Yes,
            ResolvedType::TypeParameter { bound, .. } => self.is_future_like(bound),
            ResolvedType::Interface {
                identity: DeclarationIdentity::Project { name, .. },
                ..
            } => self
                .types
                .map_or(Truth::Unknown, |types| types.is_subtype(name, "Future")),
            _ => Truth::No,
        }
    }

    pub fn substitute(
        &self,
        ty: &ResolvedType,
        substitutions: &HashMap<TypeParameterId, ResolvedType>,
    ) -> ResolvedType {
        let _ = self;
        match ty {
            ResolvedType::TypeParameter { id, nullable, .. } => substitutions
                .get(id)
                .map(|ty| ty.with_nullable(*nullable || ty.nullable()))
                .unwrap_or_else(|| ty.clone()),
            ResolvedType::Interface {
                identity,
                arguments,
                nullable,
                extension_type,
            } => ResolvedType::Interface {
                identity: identity.clone(),
                arguments: arguments
                    .iter()
                    .map(|argument| self.substitute(argument, substitutions))
                    .collect(),
                nullable: *nullable,
                extension_type: *extension_type,
            },
            ResolvedType::Function {
                return_type,
                positional,
                named,
                nullable,
            } => ResolvedType::Function {
                return_type: Box::new(self.substitute(return_type, substitutions)),
                positional: positional
                    .iter()
                    .map(|parameter| self.substitute(parameter, substitutions))
                    .collect(),
                named: named
                    .iter()
                    .map(|(name, parameter)| {
                        (name.clone(), self.substitute(parameter, substitutions))
                    })
                    .collect(),
                nullable: *nullable,
            },
            ResolvedType::Record {
                positional,
                named,
                nullable,
            } => ResolvedType::Record {
                positional: positional
                    .iter()
                    .map(|field| self.substitute(field, substitutions))
                    .collect(),
                named: named
                    .iter()
                    .map(|(name, field)| (name.clone(), self.substitute(field, substitutions)))
                    .collect(),
                nullable: *nullable,
            },
            other => other.clone(),
        }
    }

    pub fn void_context(&self, ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::Void)
            || matches!(ty, ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library: "dart:async", name: "Future" | "FutureOr" }, arguments, .. }
                if arguments.first().is_some_and(|argument| self.void_context(argument)))
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
        if *library != "dart:async" || !matches!(*name, "Future" | "FutureOr") {
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
                        .zip(right_arguments)
                        .any(|(left, right)| self.unrelated(left, right) == Truth::Yes)
                    {
                        Truth::Yes
                    } else {
                        Truth::No
                    };
                }
                let (Some(left_name), Some(right_name)) =
                    (subtype_name(left_id), subtype_name(right_id))
                else {
                    return Truth::Unknown;
                };
                let Some(types) = self.types else {
                    return Truth::Unknown;
                };
                match (
                    types.is_subtype(left_name, right_name),
                    types.is_subtype(right_name, left_name),
                ) {
                    (Truth::Yes, _) | (_, Truth::Yes) => Truth::No,
                    (Truth::No, Truth::No) => Truth::Yes,
                    _ => Truth::Unknown,
                }
            }
            (ResolvedType::TypeParameter { bound, .. }, other)
            | (other, ResolvedType::TypeParameter { bound, .. }) => self.unrelated(bound, other),
            (
                ResolvedType::Function {
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedType::Function {
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            )
            | (
                ResolvedType::Record {
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedType::Record {
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            ) => {
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

#[derive(Debug, Clone)]
pub struct ResolvedSignature {
    pub owner_parameters: Vec<TypeParameterId>,
    pub call_parameters: Vec<TypeParameterId>,
    pub positional: Vec<ResolvedType>,
    pub named: HashMap<String, ResolvedType>,
    pub return_type: ResolvedType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstructorFacts {
    pub name: String,
    pub is_const: bool,
    pub is_factory: bool,
    pub is_private: bool,
    pub parameters: Vec<ParameterFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    on_type: ResolvedType,
    members: HashSet<String>,
    /// Instance methods, getters, setters (as `name=`), and operators.
    instance_members: HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
enum InstanceAccess {
    Read,
    Write,
    Invoke,
}

#[derive(Debug, Clone, Default)]
struct MemberEntry {
    signature: Option<ResolvedSignature>,
    is_instance_method: bool,
    field: Option<(Vec<TypeParameterId>, ResolvedType)>,
    read: Option<ResolvedType>,
    write: Option<ResolvedType>,
    facts: Vec<MemberFacts>,
}

#[derive(Debug, Clone, Default)]
pub struct SignatureIndex {
    functions: HashMap<DeclarationIdentity, ResolvedSignature>,
    constructors: HashMap<DeclarationIdentity, HashMap<String, ResolvedSignature>>,
    member_entries: HashMap<DeclarationIdentity, HashMap<String, MemberEntry>>,
    supertypes: HashMap<DeclarationIdentity, (Vec<TypeParameterId>, Vec<ResolvedType>)>,
    member_supertypes: HashMap<DeclarationIdentity, Vec<ResolvedType>>,
    declarations: HashMap<DeclarationIdentity, DeclarationFacts>,
    constants: HashMap<DeclarationIdentity, Expr>,
    constant_types: HashMap<DeclarationIdentity, DeclarationIdentity>,
    extensions: Vec<ExtensionFacts>,
}

impl SignatureIndex {
    fn member_entry(&self, identity: &DeclarationIdentity, name: &str) -> Option<&MemberEntry> {
        self.member_entries.get(identity)?.get(name)
    }

    pub fn from_program(program: &Program, model: &SemanticModel<'_>) -> Self {
        let mut index = Self::default();
        index.add_sdk_signatures();
        index.add_program(program, model);
        index
    }

    pub fn from_project_files(
        files: &[(PathBuf, &Program)],
        identities: &IdentityIndex,
        types: &TypeIndex,
    ) -> Self {
        let mut index = Self::default();
        index.add_sdk_signatures();
        for (path, program) in files {
            let model = SemanticModel::new(path, identities, Some(types));
            index.add_program(program, &model);
        }
        loop {
            let mut changed = false;
            for (path, program) in files {
                let model = SemanticModel::new(path, identities, Some(types));
                changed |= index.resolve_constructor_formals(program, &model);
            }
            if !changed {
                break;
            }
        }
        index.mark_library_subclasses();
        index
    }

    fn add_program(&mut self, program: &Program, model: &SemanticModel<'_>) {
        for declaration in &program.declarations {
            match declaration {
                TopLevelDecl::Function(function) => {
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(&function.type_params, model);
                    if let Some(identity) = model.resolve_value(&function.name.name) {
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
                TopLevelDecl::Variable(variable) if variable.keyword == VarKeyword::Const => {
                    for declarator in &variable.declarators {
                        if let Some(initializer) = &declarator.initializer
                            && let Some(identity) = model.resolve_value(&declarator.name.name)
                        {
                            if let Some(constant_type) = constructed_identity(initializer, model) {
                                self.constant_types.insert(identity.clone(), constant_type);
                            }
                            self.constants.insert(identity, initializer.clone());
                        }
                    }
                }
                TopLevelDecl::Class(class) => {
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
                        &class.members,
                        model,
                    );
                    self.add_declaration_facts(
                        &class.name.name,
                        &class.annotations,
                        &class.type_params,
                        &class.members,
                        (None, class.modifiers.is_abstract),
                        model,
                    );
                }
                TopLevelDecl::MixinClass(class) => {
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
                        &class.members,
                        model,
                    );
                    self.add_declaration_facts(
                        &class.name.name,
                        &class.annotations,
                        &class.type_params,
                        &class.members,
                        (None, class.is_abstract),
                        model,
                    );
                }
                TopLevelDecl::Extension(extension) => {
                    let mut parameters = TypeParameterScope::default();
                    parameters.push(&extension.type_params, model);
                    self.extensions.push(ExtensionFacts {
                        declaring_library: model.library,
                        name: extension.name.as_ref().map(|name| name.name.clone()),
                        on_type: model.resolve_type_in(&extension.on_type, &parameters),
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
                                ClassMember::Operator(operator) => Some(operator.op.clone()),
                                _ => None,
                            })
                            .collect(),
                    });
                }
                TopLevelDecl::ExtensionType(extension) => {
                    self.add_type(
                        &extension.name.name,
                        &extension.type_params,
                        extension.implements.iter().collect(),
                        Vec::new(),
                        &extension.members,
                        model,
                    );
                    self.add_declaration_facts(
                        &extension.name.name,
                        &extension.annotations,
                        &extension.type_params,
                        &extension.members,
                        (Some(&extension.representation.field_type), false),
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
                        &mixin.members,
                        model,
                    );
                }
                TopLevelDecl::Enum(enumeration) => {
                    self.add_type(
                        &enumeration.name.name,
                        &enumeration.type_params,
                        enumeration
                            .with_clause
                            .iter()
                            .chain(&enumeration.implements)
                            .collect(),
                        enumeration.with_clause.iter().rev().collect(),
                        &enumeration.members,
                        model,
                    );
                }
                _ => {}
            }
        }
        self.resolve_constructor_formals(program, model);
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
                let (name, type_parameters, superclass, members) = match declaration {
                    TopLevelDecl::Class(class) => (
                        &class.name.name,
                        &class.type_params,
                        class.extends.as_ref(),
                        class.members.as_slice(),
                    ),
                    TopLevelDecl::MixinClass(class) => (
                        &class.name.name,
                        &class.type_params,
                        class.extends.as_ref(),
                        class.members.as_slice(),
                    ),
                    _ => continue,
                };
                let Some(superclass) = superclass else {
                    continue;
                };
                let Some(identity) = model.resolve_name(name) else {
                    continue;
                };
                let mut scope = TypeParameterScope::default();
                scope.push(type_parameters, model);
                let superclass = model.resolve_type_in(superclass, &scope);
                for member in members {
                    let ClassMember::Constructor(constructor) = member else {
                        continue;
                    };
                    let constructor_name = constructor
                        .constructor_name
                        .as_ref()
                        .map(|name| name.name.clone())
                        .unwrap_or_else(|| "new".to_string());
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
                                    .map(|name| name.name.clone())
                                    .unwrap_or_else(|| "new".to_string()),
                            ),
                            _ => None,
                        })
                        .unwrap_or_else(|| "new".to_string());
                    for parameter in constructor.params.params.iter().filter(|parameter| {
                        parameter.initializing == Some(InitializingFormal::Super)
                            && parameter.param_type.is_none()
                    }) {
                        let Some(ty) = self.resolved_constructor_parameter_type(
                            &superclass,
                            &super_constructor,
                            &parameter.name.name,
                            model,
                        ) else {
                            continue;
                        };
                        changed |= self.update_constructor_parameter_type(
                            &identity,
                            &constructor_name,
                            &constructor.params,
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

    fn resolved_constructor_parameter_type(
        &self,
        superclass: &ResolvedType,
        constructor_name: &str,
        parameter_name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = superclass
        else {
            return None;
        };
        let facts = self
            .declaration(identity)?
            .constructors
            .iter()
            .find(|constructor| constructor.name == constructor_name)?;
        let parameter = facts
            .parameters
            .iter()
            .find(|parameter| parameter.name == parameter_name)?;
        let signature = self.constructor(identity, constructor_name)?;
        let ty = if parameter.is_named {
            signature.named.get(parameter_name)?
        } else {
            let index = facts
                .parameters
                .iter()
                .filter(|parameter| !parameter.is_named)
                .position(|parameter| parameter.name == parameter_name)?;
            signature.positional.get(index)?
        };
        let substitutions = signature
            .owner_parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
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

    fn add_declaration_facts(
        &mut self,
        name: &str,
        annotations: &[falcon_syntax::ast::Annotation],
        type_parameters: &[TypeParam],
        members: &[ClassMember],
        declaration_kind: (Option<&DartType>, bool),
        model: &SemanticModel<'_>,
    ) {
        let (extension_representation, is_abstract) = declaration_kind;
        let Some(identity) = model.resolve_name(name) else {
            return;
        };
        let mut scope = TypeParameterScope::default();
        scope.push(type_parameters, model);
        let library_target = annotations
            .iter()
            .any(|annotation| annotation_targets_library(annotation, model));
        let annotations = annotations
            .iter()
            .filter_map(|annotation| {
                model.resolve_imported_member(QualifiedName::from_identifiers(&annotation.name)?)
            })
            .collect();
        let constructors = members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Constructor(constructor) => Some(ConstructorFacts {
                    name: constructor
                        .constructor_name
                        .as_ref()
                        .map(|name| name.name.clone())
                        .unwrap_or_else(|| "new".to_string()),
                    is_const: constructor.is_const,
                    is_factory: constructor.is_factory,
                    is_private: constructor
                        .constructor_name
                        .as_ref()
                        .is_some_and(|name| name.name.starts_with('_')),
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
                                                        matches!(
                                                            fact.kind,
                                                            MemberKind::Field { .. }
                                                        ) && !fact.is_static
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
                }),
                _ => None,
            })
            .collect();
        let static_consts = members
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

    fn mark_library_subclasses(&mut self) {
        let edges = self
            .supertypes
            .iter()
            .flat_map(|(subtype, (_, supertypes))| {
                supertypes
                    .iter()
                    .filter_map(move |supertype| match supertype {
                        ResolvedType::Interface { identity, .. } => {
                            Some((subtype.clone(), identity.clone()))
                        }
                        _ => None,
                    })
            })
            .collect::<Vec<_>>();
        for (subtype, supertype) in edges {
            let same_library = matches!(
                (&subtype, &supertype),
                (
                    DeclarationIdentity::Project { library: left, .. },
                    DeclarationIdentity::Project { library: right, .. }
                ) if left == right
            );
            if same_library && let Some(facts) = self.declarations.get_mut(&supertype) {
                facts.has_subclass_in_library = true;
            }
        }
    }

    fn add_sdk_signatures(&mut self) {
        for name in ["TestOn", "Timeout", "Tags", "OnPlatform"] {
            self.declarations.insert(
                DeclarationIdentity::Package {
                    package: "test",
                    name,
                },
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
            let identity = DeclarationIdentity::Sdk {
                library,
                name: type_name,
            };
            let entry = index
                .member_entries
                .entry(identity)
                .or_default()
                .entry(member.to_string())
                .or_default();
            entry.is_instance_method = true;
            entry.signature = Some(ResolvedSignature {
                owner_parameters: parameters,
                call_parameters: Vec::new(),
                positional,
                named: HashMap::new(),
                return_type,
            });
        };
        let parameter = |type_name: &'static str, ordinal: usize| ResolvedType::TypeParameter {
            id: sdk_parameter(type_name, ordinal),
            bound: Box::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let object = DeclarationIdentity::Sdk {
            library: "dart:core",
            name: "Object",
        };
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
                    positional_parameter_names: parameters,
                });
        }
        add(
            self,
            "dart:core",
            "List",
            1,
            "add",
            vec![parameter("List", 0)],
            ResolvedType::Void,
        );
        add(
            self,
            "dart:core",
            "List",
            1,
            "[]=",
            vec![ResolvedType::Dynamic, parameter("List", 0)],
            ResolvedType::Void,
        );
        add(
            self,
            "dart:core",
            "Set",
            1,
            "add",
            vec![parameter("Set", 0)],
            ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "bool",
                },
                arguments: Vec::new(),
                nullable: false,
                extension_type: false,
            },
        );
        add(
            self,
            "dart:collection",
            "Queue",
            1,
            "add",
            vec![parameter("Queue", 0)],
            ResolvedType::Void,
        );
        add(
            self,
            "dart:core",
            "Map",
            2,
            "[]=",
            vec![parameter("Map", 0), parameter("Map", 1)],
            ResolvedType::Void,
        );
        for (library, name) in [
            ("dart:core", "List"),
            ("dart:core", "Set"),
            ("dart:collection", "Queue"),
        ] {
            let parameter_id = sdk_parameter(name, 0);
            let parameter = ResolvedType::TypeParameter {
                id: parameter_id.clone(),
                bound: Box::new(ResolvedType::Dynamic),
                nullable: false,
            };
            let identity = DeclarationIdentity::Sdk { library, name };
            let supertype = ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "Iterable",
                },
                arguments: vec![parameter.clone()],
                nullable: false,
                extension_type: false,
            };
            self.supertypes.insert(
                identity.clone(),
                (vec![parameter_id], vec![supertype.clone()]),
            );
            self.member_supertypes.insert(identity, vec![supertype]);
        }
        let string = DeclarationIdentity::Sdk {
            library: "dart:core",
            name: "String",
        };
        let entry = self
            .member_entries
            .entry(string)
            .or_default()
            .entry("toUpperCase".to_string())
            .or_default();
        entry.is_instance_method = true;
        entry.signature = Some(ResolvedSignature {
            owner_parameters: Vec::new(),
            call_parameters: Vec::new(),
            positional: Vec::new(),
            named: HashMap::new(),
            return_type: ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "String",
                },
                arguments: Vec::new(),
                nullable: false,
                extension_type: false,
            },
        });
        let iterable = DeclarationIdentity::Sdk {
            library: "dart:core",
            name: "Iterable",
        };
        let map_parameter = sdk_parameter("Iterable.map", 0);
        let map_type = ResolvedType::TypeParameter {
            id: map_parameter.clone(),
            bound: Box::new(ResolvedType::Dynamic),
            nullable: false,
        };
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
            positional: vec![ResolvedType::Function {
                return_type: Box::new(map_type.clone()),
                positional: vec![parameter("Iterable", 0)],
                named: Vec::new(),
                nullable: false,
            }],
            named: HashMap::new(),
            return_type: ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "Iterable",
                },
                arguments: vec![map_type],
                nullable: false,
                extension_type: false,
            },
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
            self.member_supertypes
                .insert(identity.clone(), vec![supertype]);
            self.constructors.entry(identity).or_default().insert(
                "new".to_string(),
                ResolvedSignature {
                    owner_parameters: Vec::new(),
                    call_parameters: Vec::new(),
                    positional: Vec::new(),
                    named: HashMap::from([("key".to_string(), key.clone())]),
                    return_type: ResolvedType::Unknown,
                },
            );
        }
        self.constructors.entry(widget).or_default().insert(
            "new".to_string(),
            ResolvedSignature {
                owner_parameters: Vec::new(),
                call_parameters: Vec::new(),
                positional: Vec::new(),
                named: HashMap::from([("key".to_string(), key)]),
                return_type: ResolvedType::Unknown,
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
                identity: DeclarationIdentity::Sdk { library, name },
                arguments,
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
                                    bound: Box::new(ResolvedType::Dynamic),
                                    nullable: false,
                                })
                                .collect()
                        } else {
                            Vec::new()
                        };
                    sdk(super_library, super_name, arguments)
                })
                .collect();
            self.supertypes
                .entry(DeclarationIdentity::Sdk { library, name })
                .or_insert((parameters, resolved));
        }
        let element = |owner: &'static str| ResolvedType::TypeParameter {
            id: sdk_parameter(owner, 0),
            bound: Box::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let listen = self
            .member_entries
            .entry(DeclarationIdentity::Sdk {
                library: "dart:async",
                name: "Stream",
            })
            .or_default()
            .entry("listen".to_string())
            .or_default();
        listen.is_instance_method = true;
        listen.signature = Some(ResolvedSignature {
            owner_parameters: vec![sdk_parameter("Stream", 0)],
            call_parameters: Vec::new(),
            positional: vec![ResolvedType::Function {
                return_type: Box::new(ResolvedType::Void),
                positional: vec![element("Stream")],
                named: Vec::new(),
                nullable: true,
            }],
            named: HashMap::new(),
            return_type: sdk("dart:async", "StreamSubscription", vec![element("Stream")]),
        });
        self.member_entries
            .entry(DeclarationIdentity::Sdk {
                library: "dart:async",
                name: "StreamController",
            })
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
            .entry(DeclarationIdentity::Sdk {
                library: "dart:io",
                name: "File",
            })
            .or_default()
            .entry("openWrite".to_string())
            .or_default();
        open_write.is_instance_method = true;
        open_write.signature = Some(ResolvedSignature {
            owner_parameters: Vec::new(),
            call_parameters: Vec::new(),
            positional: Vec::new(),
            named: HashMap::new(),
            return_type: sdk("dart:io", "IOSink", Vec::new()),
        });
    }

    fn add_type(
        &mut self,
        name: &str,
        type_parameters: &[TypeParam],
        supertypes: Vec<&DartType>,
        member_supertypes: Vec<&DartType>,
        members: &[ClassMember],
        model: &SemanticModel<'_>,
    ) {
        let Some(identity) = model.resolve_name(name) else {
            return;
        };
        let mut scope = TypeParameterScope::default();
        scope.push(type_parameters, model);
        let owner_parameters = parameter_ids(&scope, type_parameters);
        let field_types = members
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
        let mut resolved_supertypes: Vec<_> = supertypes
            .into_iter()
            .map(|supertype| model.resolve_type_in(supertype, &scope))
            .collect();
        resolved_supertypes.push(model.core_type("Object", false));
        self.supertypes.insert(
            identity.clone(),
            (owner_parameters.clone(), resolved_supertypes),
        );
        let mut resolved_member_supertypes: Vec<_> = member_supertypes
            .into_iter()
            .map(|supertype| model.resolve_type_in(supertype, &scope))
            .collect();
        resolved_member_supertypes.push(model.core_type("Object", false));
        self.member_supertypes
            .insert(identity.clone(), resolved_member_supertypes);
        let entries = self.member_entries.entry(identity.clone()).or_default();
        for member in members {
            match member {
                ClassMember::Method(method) => {
                    let entry = entries.entry(method.name.name.clone()).or_default();
                    entry.facts.push(MemberFacts {
                        declaring_type: identity.clone(),
                        kind: MemberKind::Method,
                        is_static: method.is_static,
                        is_abstract: method.is_abstract,
                        positional_parameter_names: positional_parameter_names(&method.params),
                    });
                    scope.push(&method.type_params, model);
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
                        let entry = entries.entry(declarator.name.name.clone()).or_default();
                        entry.facts.push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind: MemberKind::field(field),
                            is_static: field.is_static,
                            is_abstract: field.is_abstract,
                            positional_parameter_names: Vec::new(),
                        });
                        entry.field = Some((owner_parameters.clone(), field_type.clone()));
                        if !field.is_static {
                            entry.read = Some(field_type.clone());
                            if field.keyword == VarKeyword::Var {
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
                    });
                    let return_type = getter
                        .return_type
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Unknown);
                    entry.field = Some((owner_parameters.clone(), return_type.clone()));
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
                    entries
                        .entry(operator.op.clone())
                        .or_default()
                        .facts
                        .push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind: MemberKind::Operator,
                            is_static: false,
                            is_abstract: operator.body.is_none() && !operator.is_external,
                            positional_parameter_names: positional_parameter_names(
                                &operator.params,
                            ),
                        });
                }
                ClassMember::Error(_) => {}
            }
        }
    }

    pub fn declaration(&self, identity: &DeclarationIdentity) -> Option<&DeclarationFacts> {
        self.declarations.get(identity)
    }

    /// The number of type parameters a project-declared class, mixin, enum,
    /// or extension type declares; `None` for SDK, package, and unindexed types.
    pub fn project_type_parameter_count(&self, identity: &DeclarationIdentity) -> Option<usize> {
        if !matches!(identity, DeclarationIdentity::Project { .. }) {
            return None;
        }
        self.supertypes
            .get(identity)
            .map(|(parameters, _)| parameters.len())
    }

    pub fn constant_initializer(&self, identity: &DeclarationIdentity) -> Option<&Expr> {
        self.constants.get(identity)
    }

    pub fn constant_type(&self, identity: &DeclarationIdentity) -> Option<&DeclarationIdentity> {
        self.constant_types.get(identity)
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

    pub fn function(&self, identity: &DeclarationIdentity) -> Option<&ResolvedSignature> {
        self.functions.get(identity)
    }

    pub fn constructor(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&ResolvedSignature> {
        self.constructors.get(identity)?.get(name)
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
    ) -> Option<(ResolvedSignature, HashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = receiver
        else {
            return None;
        };
        let signature = self.constructor(identity, name)?;
        let substitutions = signature
            .owner_parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        Some((signature.clone(), substitutions))
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
        let (_, supertypes) = self.supertypes.get(identity)?;
        let mut facts = Vec::new();
        let mut visited = HashSet::new();
        for supertype in supertypes {
            self.collect_inherited_member_facts(
                identity,
                supertype,
                name,
                &mut visited,
                &mut facts,
            )?;
        }
        Some(facts)
    }

    fn collect_inherited_member_facts(
        &self,
        owner: &DeclarationIdentity,
        supertype: &ResolvedType,
        name: &str,
        visited: &mut HashSet<DeclarationIdentity>,
        facts: &mut Vec<MemberFacts>,
    ) -> Option<()> {
        let ResolvedType::Interface { identity, .. } = supertype else {
            return None;
        };
        if !visited.insert(identity.clone()) {
            return Some(());
        }
        let is_object = matches!(
            identity,
            DeclarationIdentity::Sdk {
                library: "dart:core",
                name: "Object"
            }
        );
        if let DeclarationIdentity::Sdk {
            library,
            name: type_name,
        } = identity
            && !is_object
        {
            // Only `Object` has recorded member facts; the curated supertype
            // edges say nothing about what other SDK types declare, so
            // reaching one is unknown unless its members are listed in
            // [`SDK_INTERFACE_MEMBERS`].
            let (.., members) = SDK_INTERFACE_MEMBERS
                .iter()
                .find(|entry| entry.0 == *library && entry.1 == *type_name)?;
            facts.extend(members.iter().filter(|member| member.0 == name).map(
                |&(_, kind, parameters)| {
                    MemberFacts {
                        declaring_type: identity.clone(),
                        kind,
                        is_static: false,
                        is_abstract: true,
                        positional_parameter_names: parameters
                            .iter()
                            .map(|&parameter| parameter.to_owned())
                            .collect(),
                    }
                },
            ));
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
                self.collect_inherited_member_facts(owner, supertype, name, visited, facts)?;
            }
        } else if !is_object {
            return None;
        }
        Some(())
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

    fn has_cyclic_supertypes(
        &self,
        identity: &DeclarationIdentity,
        visiting: &mut HashSet<DeclarationIdentity>,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> bool {
        if visiting.contains(identity) {
            return true;
        }
        if visited.contains(identity) {
            return false;
        }
        visiting.insert(identity.clone());
        let cyclic = self
            .supertypes
            .get(identity)
            .is_some_and(|(_, supertypes)| {
                supertypes.iter().any(|supertype| match supertype {
                    ResolvedType::Interface { identity, .. } => {
                        self.has_cyclic_supertypes(identity, visiting, visited)
                    }
                    _ => false,
                })
            });
        visiting.remove(identity);
        visited.insert(identity.clone());
        cyclic
    }

    fn instantiated_identity(
        &self,
        ty: &ResolvedType,
        target: &DeclarationIdentity,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = ty
        else {
            return None;
        };
        if self.has_cyclic_supertypes(identity, &mut HashSet::new(), &mut HashSet::new()) {
            return None;
        }
        if identity == target {
            return Some(ty.clone());
        }
        let (parameters, supertypes) = self.supertypes.get(identity)?;
        let substitutions: HashMap<_, _> = parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        supertypes.iter().find_map(|supertype| {
            let instantiated = model.substitute(supertype, &substitutions);
            self.instantiated_identity(&instantiated, target, model)
        })
    }

    pub fn instantiated_supertype(
        &self,
        ty: &ResolvedType,
        library: &str,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = ty
        else {
            return None;
        };
        if self.has_cyclic_supertypes(identity, &mut HashSet::new(), &mut HashSet::new()) {
            return None;
        }
        if identity.is_sdk(library, name) {
            return Some(ty.clone());
        }
        let (parameters, supertypes) = self.supertypes.get(identity)?;
        let substitutions: HashMap<_, _> = parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        supertypes.iter().find_map(|supertype| {
            let instantiated = model.substitute(supertype, &substitutions);
            self.instantiated_supertype(&instantiated, library, name, model)
        })
    }

    /// Three-valued "`ty` implements SDK declaration `library.name`".
    ///
    /// `Yes` when any declared supertype path reaches the target. `No` only when
    /// every reachable edge resolved to a declaration whose own supertype list is
    /// recorded (project declarations and the curated [`SDK_HIERARCHY`]);
    /// anything else — unresolved supertypes, unlisted SDK or package types,
    /// extension types, non-interface types — is `Unknown`.
    pub fn implements_sdk(&self, ty: &ResolvedType, library: &str, name: &str) -> Truth {
        match ty {
            ResolvedType::TypeParameter { bound, .. } => self.implements_sdk(bound, library, name),
            ResolvedType::Interface {
                identity,
                extension_type: false,
                ..
            } => self.identity_implements_sdk(identity, library, name, &mut HashSet::new()),
            _ => Truth::Unknown,
        }
    }

    fn identity_implements_sdk(
        &self,
        identity: &DeclarationIdentity,
        library: &str,
        name: &str,
        visited: &mut HashSet<DeclarationIdentity>,
    ) -> Truth {
        if identity.is_sdk(library, name) {
            return Truth::Yes;
        }
        if *identity == DeclarationIdentity::OBJECT || !visited.insert(identity.clone()) {
            return Truth::No;
        }
        let Some((_, supertypes)) = self.supertypes.get(identity) else {
            return Truth::Unknown;
        };
        let mut result = Truth::No;
        for supertype in supertypes {
            let found = match supertype {
                ResolvedType::Interface {
                    identity,
                    extension_type: false,
                    ..
                } => self.identity_implements_sdk(identity, library, name, visited),
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

    pub fn has_applicable_extension_member(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> bool {
        self.extensions.iter().any(|extension| {
            extension.members.contains(name)
                && model.extension_visible(extension.declaring_library, extension.name.as_deref())
                && match &extension.on_type {
                    ResolvedType::Interface {
                        identity: target, ..
                    } => self.is_subtype_of(receiver, target, model) == Truth::Yes,
                    ResolvedType::Dynamic => true,
                    ResolvedType::Unknown => false,
                    target => self.unrelated(receiver, target, model) != Truth::Yes,
                }
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
        for extension in &self.extensions {
            if !extension.instance_members.contains(name)
                || !model.extension_visible(extension.declaring_library, extension.name.as_deref())
            {
                continue;
            }
            match self.extension_applies(&receiver, &extension.on_type, model) {
                Truth::Yes => on_types.push(&extension.on_type),
                Truth::No => {}
                Truth::Unknown => return None,
            }
        }
        Some(on_types)
    }

    fn extension_applies(
        &self,
        receiver: &ResolvedType,
        on_type: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> Truth {
        match on_type {
            ResolvedType::Dynamic => Truth::Yes,
            ResolvedType::Interface { identity, .. }
                if *identity == DeclarationIdentity::OBJECT =>
            {
                Truth::Yes
            }
            ResolvedType::Interface { identity, .. } => {
                if self.is_subtype_of(receiver, identity, model) == Truth::Yes {
                    Truth::Yes
                } else if self.unrelated(receiver, &on_type.with_nullable(false), model)
                    == Truth::Yes
                {
                    Truth::No
                } else {
                    Truth::Unknown
                }
            }
            ResolvedType::TypeParameter { bound, .. } => {
                self.extension_applies(receiver, bound, model)
            }
            _ => Truth::Unknown,
        }
    }

    /// The on-type of the visible extension named `name`.
    pub fn visible_extension_on_type(
        &self,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<&ResolvedType> {
        self.extensions
            .iter()
            .find(|extension| {
                extension.name.as_deref() == Some(name)
                    && model.extension_visible(extension.declaring_library, Some(name))
            })
            .map(|extension| &extension.on_type)
    }

    fn resolved_instance_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        if let Some(owner) =
            self.resolved_instance_owner_before_object(receiver, name, access, model)
        {
            return Some(owner);
        }
        self.has_instance_declaration(&DeclarationIdentity::OBJECT, name, access)
            .then(|| model.core_type("Object", false))
    }

    fn resolved_instance_owner_before_object(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = receiver
        else {
            return None;
        };
        if matches!(
            identity,
            DeclarationIdentity::Sdk {
                library: "dart:core",
                name: "Object"
            }
        ) {
            return None;
        }
        if self.has_cyclic_supertypes(identity, &mut HashSet::new(), &mut HashSet::new()) {
            return None;
        }
        if model.member_visible(identity, name)
            && self.has_instance_declaration(identity, name, access)
        {
            return Some(receiver.clone());
        }
        let (parameters, _) = self.supertypes.get(identity)?;
        let substitutions: HashMap<_, _> = parameters
            .iter()
            .cloned()
            .zip(arguments.iter().cloned())
            .collect();
        self.member_supertypes
            .get(identity)?
            .iter()
            .find_map(|supertype| {
                let instantiated = model.substitute(supertype, &substitutions);
                self.resolved_instance_owner_before_object(&instantiated, name, access, model)
            })
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

    pub fn resolved_member(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(ResolvedSignature, HashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = self.resolved_instance_owner(receiver, name, InstanceAccess::Invoke, model)?
        else {
            return None;
        };
        let signature = self.instance_member(&identity, name)?;
        let substitutions = signature
            .owner_parameters
            .iter()
            .cloned()
            .zip(arguments)
            .collect();
        Some((signature.clone(), substitutions))
    }

    pub fn resolved_field(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(ResolvedType, HashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = self.resolved_instance_owner(receiver, name, InstanceAccess::Read, model)?
        else {
            return None;
        };
        let field = self.member_entry(&identity, name)?.read.as_ref()?;
        let (parameters, _) = self.supertypes.get(&identity)?;
        let substitutions = parameters.iter().cloned().zip(arguments).collect();
        Some((field.clone(), substitutions))
    }

    pub fn resolved_writable_field(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> Option<(ResolvedType, HashMap<TypeParameterId, ResolvedType>)> {
        let ResolvedType::Interface {
            identity,
            arguments,
            ..
        } = self.resolved_instance_owner(receiver, name, InstanceAccess::Write, model)?
        else {
            return None;
        };
        let field = self.member_entry(&identity, name)?.write.as_ref()?;
        let (parameters, _) = self.supertypes.get(&identity)?;
        let substitutions = parameters.iter().cloned().zip(arguments).collect();
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
    match expression {
        Expr::New { dart_type, .. } => match model.resolve_type(dart_type) {
            ResolvedType::Interface { identity, .. } => Some(identity),
            _ => None,
        },
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
        match current {
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
    match expression {
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
            DeclarationIdentity::Package { package: left, .. },
            DeclarationIdentity::Package { package: right, .. }
        ) if left == right
    )
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
        param
            .param_type
            .as_ref()
            .map(|ty| model.resolve_type_in(ty, scope))
            .unwrap_or_else(|| {
                if param.initializing == Some(InitializingFormal::This) {
                    field_types
                        .and_then(|fields| fields.get(&param.name.name))
                        .cloned()
                        .unwrap_or(ResolvedType::Unknown)
                } else if param.initializing == Some(InitializingFormal::Super) {
                    ResolvedType::Unknown
                } else {
                    ResolvedType::Dynamic
                }
            })
    };
    ResolvedSignature {
        owner_parameters: owner_parameters.to_vec(),
        call_parameters: parameter_ids(scope, call_parameters),
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
        return_type: return_type
            .map(|ty| model.resolve_type_in(ty, scope))
            .unwrap_or(ResolvedType::Void),
    }
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

#[derive(Debug, Clone, Default)]
pub struct TypeEnvironment {
    scopes: Vec<HashMap<String, ResolvedType>>,
}

impl TypeEnvironment {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    pub fn declare(&mut self, name: impl Into<String>, ty: ResolvedType) {
        self.scopes
            .last_mut()
            .expect("root scope")
            .insert(name.into(), ty);
    }

    pub fn assign(&mut self, name: &str, ty: ResolvedType) -> bool {
        let Some(scope) = self
            .scopes
            .iter_mut()
            .rev()
            .find(|scope| scope.contains_key(name))
        else {
            return false;
        };
        scope.insert(name.to_string(), ty);
        true
    }

    pub fn bind_params(
        &mut self,
        params: &FormalParamList,
        model: &SemanticModel<'_>,
        type_params: &TypeParameterScope,
    ) {
        for param in params.params.iter() {
            let ty = param
                .param_type
                .as_ref()
                .map(|ty| model.resolve_type_in(ty, type_params))
                .unwrap_or(ResolvedType::Dynamic);
            self.declare(param.name.name.clone(), ty);
        }
    }

    pub fn is_bound(&self, name: &str) -> bool {
        self.scopes
            .iter()
            .rev()
            .any(|scope| scope.contains_key(name))
    }

    pub fn lookup(&self, name: &str) -> ResolvedType {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
            .unwrap_or(ResolvedType::Unknown)
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

    pub(crate) fn infer_with_signatures_using_lookup(
        &self,
        expr: &Expr,
        model: &SemanticModel<'_>,
        signatures: &SignatureIndex,
        type_parameters: &TypeParameterScope,
        lookup: &impl Fn(&str) -> Option<ResolvedType>,
    ) -> ResolvedType {
        let infer = |expr| {
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
            let receiver = self.infer_with_signatures_using_lookup(
                object,
                model,
                signatures,
                type_parameters,
                lookup,
            );
            if let Some((original, substitutions)) =
                signatures.resolved_field(&receiver, &field.name, model)
            {
                let ty = model.substitute(&original, &substitutions);
                return if *is_null_safe {
                    ty.with_nullable(true)
                } else {
                    ty
                };
            }
        }
        if let Expr::Call {
            callee, type_args, ..
        } = expr
        {
            match callee.as_ref() {
                Expr::Ident(identifier)
                    if !self.is_bound(&identifier.name) && lookup(&identifier.name).is_none() =>
                {
                    if let Some(identity) = model.resolve_value(&identifier.name)
                        && let Some(signature) = signatures.function(&identity)
                    {
                        let substitutions: HashMap<_, _> = signature
                            .call_parameters
                            .iter()
                            .zip(type_args)
                            .map(|(parameter, argument)| {
                                (
                                    parameter.clone(),
                                    model.resolve_type_in(argument, type_parameters),
                                )
                            })
                            .collect();
                        return model.substitute(&signature.return_type, &substitutions);
                    }
                    if let Some(identity) = model.resolve_name(&identifier.name) {
                        return ResolvedType::Interface {
                            identity,
                            arguments: type_args
                                .iter()
                                .map(|argument| model.resolve_type_in(argument, type_parameters))
                                .collect(),
                            nullable: false,
                            extension_type: false,
                        };
                    }
                }
                Expr::Field {
                    object,
                    field,
                    is_null_safe,
                    ..
                } => {
                    if !*is_null_safe
                        && let Some(segments) = expression_segments(object, false)
                        && segments
                            .first()
                            .is_some_and(|head| !self.is_bound(head) && lookup(head).is_none())
                        && let Some(identity) = model.resolve_dotted(&segments)
                        && signatures.constructor(&identity, &field.name).is_some()
                    {
                        return ResolvedType::Interface {
                            identity,
                            arguments: type_args
                                .iter()
                                .map(|ty| model.resolve_type_in(ty, type_parameters))
                                .collect(),
                            nullable: false,
                            extension_type: false,
                        };
                    }
                    let receiver = self.infer_with_signatures_using_lookup(
                        object,
                        model,
                        signatures,
                        type_parameters,
                        lookup,
                    );
                    if let Some((signature, mut substitutions)) =
                        signatures.resolved_member(&receiver, &field.name, model)
                    {
                        substitutions.extend(signature.call_parameters.iter().zip(type_args).map(
                            |(parameter, argument)| {
                                (
                                    parameter.clone(),
                                    model.resolve_type_in(argument, type_parameters),
                                )
                            },
                        ));
                        let ty = model.substitute(&signature.return_type, &substitutions);
                        return if *is_null_safe {
                            ty.with_nullable(true)
                        } else {
                            ty
                        };
                    }
                }
                Expr::GenericInstantiation {
                    target,
                    type_args: instantiated,
                    ..
                } => {
                    if let Expr::Ident(identifier) = target.as_ref()
                        && let Some(identity) = model.resolve_value(&identifier.name)
                        && let Some(signature) = signatures.function(&identity)
                    {
                        let substitutions: HashMap<_, _> = signature
                            .call_parameters
                            .iter()
                            .zip(instantiated)
                            .map(|(parameter, argument)| {
                                (
                                    parameter.clone(),
                                    model.resolve_type_in(argument, type_parameters),
                                )
                            })
                            .collect();
                        return model.substitute(&signature.return_type, &substitutions);
                    }
                }
                _ => {}
            }
        }
        match expr {
            Expr::FuncExpr {
                type_params,
                params,
                body,
                ..
            } => {
                let mut function_parameters = type_parameters.clone();
                function_parameters.push(type_params, model);
                let positional = params
                    .positional()
                    .map(|param| {
                        param
                            .param_type
                            .as_ref()
                            .map(|ty| model.resolve_type_in(ty, &function_parameters))
                            .unwrap_or(ResolvedType::Dynamic)
                    })
                    .collect();
                let named = params
                    .named()
                    .map(|param| {
                        (
                            param.name.name.clone(),
                            param
                                .param_type
                                .as_ref()
                                .map(|ty| model.resolve_type_in(ty, &function_parameters))
                                .unwrap_or(ResolvedType::Dynamic),
                        )
                    })
                    .collect();
                let return_type = match body.as_ref() {
                    FunctionBody::Arrow(expression, _) => self.infer_with_signatures_using_lookup(
                        expression,
                        model,
                        signatures,
                        &function_parameters,
                        lookup,
                    ),
                    FunctionBody::Block(_) | FunctionBody::Native(_, _) => ResolvedType::Unknown,
                };
                ResolvedType::Function {
                    return_type: Box::new(return_type),
                    positional,
                    named,
                    nullable: false,
                }
            }
            Expr::Is { .. }
            | Expr::Unary {
                op: falcon_syntax::ast::UnaryOp::Bang,
                ..
            } => model.core_type("bool", false),
            Expr::Unary { operand, .. } | Expr::PostfixIncDec { operand, .. } => {
                let ty = infer(operand);
                if numeric(&ty) {
                    ty.with_nullable(false)
                } else {
                    ResolvedType::Unknown
                }
            }
            Expr::IntLit { .. } => model.core_type("int", false),
            Expr::DoubleLit { .. } => model.core_type("double", false),
            Expr::StringLit(_) => model.core_type("String", false),
            Expr::BoolLit { .. } => model.core_type("bool", false),
            Expr::NullLit { .. } => ResolvedType::Null,
            Expr::Ident(identifier) => {
                lookup(&identifier.name).unwrap_or_else(|| self.lookup(&identifier.name))
            }
            Expr::As { dart_type, .. } => model.resolve_type_in(dart_type, type_parameters),
            Expr::NullAssert { operand, .. } => infer(operand).with_nullable(false),
            Expr::New { dart_type, .. } => model
                .resolve_type_in(dart_type, type_parameters)
                .with_nullable(false),
            Expr::List { type_arg, .. } => ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "List",
                },
                arguments: vec![
                    type_arg
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, type_parameters))
                        .unwrap_or(ResolvedType::Dynamic),
                ],
                nullable: false,
                extension_type: false,
            },
            Expr::Set { type_arg, .. } => ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "Set",
                },
                arguments: vec![
                    type_arg
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, type_parameters))
                        .unwrap_or(ResolvedType::Dynamic),
                ],
                nullable: false,
                extension_type: false,
            },
            Expr::Map { type_args, .. } => ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "Map",
                },
                arguments: type_args
                    .iter()
                    .map(|ty| model.resolve_type_in(ty, type_parameters))
                    .collect(),
                nullable: false,
                extension_type: false,
            },
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
                BinaryOp::NullCoalesce => ResolvedType::Unknown,
                _ => {
                    let left = infer(left);
                    let right = infer(right);
                    if *op == BinaryOp::Add
                        && left.interface("dart:core", "String")
                        && !left.nullable()
                        && right.interface("dart:core", "String")
                        && !right.nullable()
                    {
                        model.core_type("String", false)
                    } else if numeric(&left) && numeric(&right) {
                        let name = if *op == BinaryOp::Div {
                            "double"
                        } else if *op == BinaryOp::IntDiv {
                            "int"
                        } else if left.interface("dart:core", "double")
                            || (left.interface("dart:core", "int")
                                && right.interface("dart:core", "double"))
                        {
                            "double"
                        } else if left.interface("dart:core", "int")
                            && right.interface("dart:core", "int")
                        {
                            "int"
                        } else {
                            "num"
                        };
                        model.core_type(name, false)
                    } else {
                        ResolvedType::Unknown
                    }
                }
            },
            Expr::Conditional {
                then_expr,
                else_expr,
                ..
            } => {
                let then_type = infer(then_expr);
                let else_type = infer(else_expr);
                if then_type == else_type {
                    then_type
                } else {
                    ResolvedType::Unknown
                }
            }
            _ => ResolvedType::Unknown,
        }
    }
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
        name: format!("{owner}#{ordinal}"),
    }
}

fn subtype_name(identity: &DeclarationIdentity) -> Option<&str> {
    match identity {
        DeclarationIdentity::Package { .. } => None,
        identity => Some(identity.name()),
    }
}

fn numeric(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk {
                library: "dart:core",
                name: "int" | "double" | "num"
            },
            ..
        }
    )
}

fn fixnum_and_int(fixnum: &ResolvedType, int: &ResolvedType) -> bool {
    matches!(
        fixnum,
        ResolvedType::Interface {
            identity: DeclarationIdentity::Package {
                package: "fixnum",
                name: "Int32" | "Int64"
            },
            ..
        }
    ) && int.interface("dart:core", "int")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use falcon_dart_parser::parse;

    use super::*;
    use crate::resolve::{IdentitySource, LibrarySource};

    fn with_model(source: &str, test: impl FnOnce(&Program, &SemanticModel<'_>)) {
        let path = PathBuf::from("/project/lib/main.dart");
        let (program, errors) = parse(source);
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
    fn expression_inference_regression_table() {
        let cases = [
            ("Box.named()", Some("Box")),
            ("box?.field", Some("int?")),
            ("box?.method()", Some("int?")),
            ("box?.fail()", Some("Null")),
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
            ("n + d", Some("num")),
            ("n * d", Some("num")),
            ("n % d", Some("num")),
            ("d + n", Some("double")),
            ("i + n", Some("num")),
            ("i + i", Some("int")),
            ("s + s", Some("String")),
            ("x ?? i", None),
            ("i / i", Some("double")),
            ("d / n", Some("double")),
            ("n / i", Some("double")),
            ("d ~/ n", Some("int")),
            ("n ~/ i", Some("int")),
            ("i ~/ d", Some("int")),
            ("box / i", None),
            ("box ~/ i", None),
            ("x / i", None),
            ("i / x", None),
            ("x ~/ i", None),
            ("missing / i", None),
        ];
        let mut source = String::from(
            "class Box { Box.named(); int field = 0; int method() => 0; Never fail() => throw 0; Box operator /(num s) => this; }",
        );
        for (index, (expression, expected)) in cases.iter().enumerate() {
            source.push_str(&format!(
                "{} f{index}(Box? box, dynamic x, int i, double d, num n, String s, bool flag) => {expression};",
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
                let (program, errors) = parse(source);
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
                    matches!(arguments.first(), Some(ResolvedType::TypeParameter { id, .. }) if id.name == "U")
                );
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
    fn inference_lookup_override_reaches_recursive_expressions() {
        with_model(
            "Object direct() => value; Object asserted() => value!; Object binary() => value + value; Object conditional() => true ? value : value; Object method() => value.toUpperCase(); Object closure() => () => value;",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.declare("value", model.core_type("int", false));
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
                        assert_eq!(
                            environment.infer_with_signatures_using_lookup(
                                expression,
                                model,
                                &signatures,
                                &scope,
                                &|_| Some(ResolvedType::Unknown),
                            ),
                            ResolvedType::Unknown,
                        );
                        assert!(
                            environment
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
                    identity: model.resolve_name("A").unwrap(),
                    arguments: vec![model.core_type("String", false)],
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
    fn implementation_lookup_prefers_later_mixins_and_excludes_interfaces() {
        with_model(
            "class Base { num value() => 0; num field = 0; } mixin First<T> { T value() => throw UnimplementedError(); T field; } mixin Second<T> { T value() => throw UnimplementedError(); T field; } abstract class Contract { bool value(); bool field; int only(); } class Child extends Base with First<String>, Second<int> implements Contract {} class InterfaceOnly implements Contract {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child_identity = model.resolve_name("Child").unwrap();
                let child = ResolvedType::Interface {
                    identity: child_identity,
                    arguments: Vec::new(),
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
                    identity: model.resolve_name("InterfaceOnly").unwrap(),
                    arguments: Vec::new(),
                    nullable: false,
                    extension_type: false,
                };
                assert_eq!(
                    signatures.is_subtype_of(&interface_only, &contract, model),
                    Truth::Yes
                );
                assert!(
                    signatures
                        .resolved_member(&interface_only, "only", model)
                        .is_none()
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
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new(),
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
    fn child_setter_does_not_hide_inherited_generic_getter() {
        with_model(
            "class Base<T> { T get value => throw UnimplementedError(); } class Child extends Base<String> { set value(num next) {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child = ResolvedType::Interface {
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new(),
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
                    identity: base.clone(),
                    arguments: Vec::new(),
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
    fn later_mixin_getter_overrides_base_field_with_owner_substitution() {
        with_model(
            "class Base { num value = 0; } mixin ReadValue<T> { T get value => throw UnimplementedError(); } class Child extends Base with ReadValue<int> {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child = ResolvedType::Interface {
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new(),
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
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new(),
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
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new(),
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
    fn object_fallback_does_not_beat_earlier_application_chain_members() {
        with_model(
            "class Base { String toString() => ''; } mixin First { String toString() => ''; } mixin Last {} class FromBase extends Base with Last {} class FromMixin extends Base with First, Last {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                for (class_name, owner_name) in [("FromBase", "Base"), ("FromMixin", "First")] {
                    let receiver = ResolvedType::Interface {
                        identity: model.resolve_name(class_name).unwrap(),
                        arguments: Vec::new(),
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
                let iterable = DeclarationIdentity::Sdk {
                    library: "dart:core",
                    name: "Iterable",
                };
                let signature = signatures.member(&iterable, "map").unwrap();
                assert_eq!(signature.call_parameters.len(), 1);
                let ResolvedType::Function { return_type, .. } = &signature.positional[0] else {
                    panic!()
                };
                assert_eq!(
                    return_type.as_ref(),
                    &ResolvedType::TypeParameter {
                        id: signature.call_parameters[0].clone(),
                        bound: Box::new(ResolvedType::Dynamic),
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
        let (root, root_errors) = parse(
            "library shared; part 'part.dart'; class Base { int _method() => 0; int _field = 0; }",
        );
        let (part, part_errors) = parse("part of shared; class Child extends Base {}");
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
        let types = TypeIndex::from_library_files([
            LibrarySource {
                program: &root,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &part,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        ]);
        let files = [(root_path, &root), (part_path.clone(), &part)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&part_path, &identities, Some(&types));
        let child = ResolvedType::Interface {
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new(),
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
        let (base, base_errors) = parse("class Base { int _method() => 0; int _field = 0; }");
        let (child_program, child_errors) =
            parse("import 'base.dart'; class Child extends Base {}");
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
        let types = TypeIndex::from_library_files([
            LibrarySource {
                program: &base,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &child_program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        ]);
        let files = [(base_path, &base), (child_path.clone(), &child_program)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&child_path, &identities, Some(&types));
        let child = ResolvedType::Interface {
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new(),
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
        let (base, base_errors) = parse(
            "import 'b.dart'; class Base { int _method() => 0; int _field = 0; } class Child extends Middle {}",
        );
        let (middle, middle_errors) = parse(
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
        let types = TypeIndex::from_library_files([
            LibrarySource {
                program: &base,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &middle,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        ]);
        let files = [(base_path.clone(), &base), (middle_path, &middle)];
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&base_path, &identities, Some(&types));
        let base_identity = model.resolve_name("Base").unwrap();
        let child = ResolvedType::Interface {
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new(),
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
    fn cyclic_supertypes_are_unknown() {
        with_model(
            "class A extends B { int field = 0; void method() {} } class B extends A {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let a = model.resolve_name("A").unwrap();
                let b = model.resolve_name("B").unwrap();
                let receiver = ResolvedType::Interface {
                    identity: a,
                    arguments: Vec::new(),
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
        let (base, base_errors) = parse("class Base<T> { final T value; Base(this.value); }");
        let (middle, middle_errors) =
            parse("import 'base.dart'; class Middle extends Base<String> { Middle(super.value); }");
        let (leaf, leaf_errors) =
            parse("import 'middle.dart'; class Leaf extends Middle { Leaf(super.value); }");
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
        let types = TypeIndex::from_library_files([
            LibrarySource {
                program: &leaf,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &middle,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &base,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        ]);
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
        let (a, a_errors) = parse(
            "import 'b.dart'; class A0<T> { final T value; A0(this.value); } class A2 extends B1 { A2(super.value); } class A4 extends B3 { A4(super.value); } class A6 extends B5 { A6(super.value); }",
        );
        let (b, b_errors) = parse(
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
        let types = TypeIndex::from_library_files([
            LibrarySource {
                program: &a,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &b,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        ]);
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

    fn contains_parameter(ty: &ResolvedType) -> bool {
        matches!(ty, ResolvedType::TypeParameter { .. })
    }
}
