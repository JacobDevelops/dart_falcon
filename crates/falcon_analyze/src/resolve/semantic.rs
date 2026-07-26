//! Canonical semantic facts shared by resolver-dependent rules.
//!
//! This is deliberately conservative: every unresolved or ambiguous identity
//! becomes `Unknown`, and consumers diagnose only from proven facts.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use falcon_syntax::ast::{
    BinaryOp, ClassMember, DartType, Expr, FormalParam, FormalParamList, FunctionBody, Program,
    TopLevelDecl, TypeParam,
};

use super::{DeclarationIdentity, IdentityIndex, SubtypeResult, TypeIndex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTruth {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeParameterId {
    pub owner: usize,
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
            other => other.clone(),
        }
    }

    pub fn interface(&self, library: &str, name: &str) -> bool {
        matches!(self, Self::Interface { identity: DeclarationIdentity::Sdk { library: found_library, name: found_name }, .. }
            if found_library == library && found_name == name)
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
    next_owner: usize,
}

impl TypeParameterScope {
    pub fn push(&mut self, params: &[TypeParam], model: &SemanticModel<'_>) {
        let owner = self.next_owner;
        self.next_owner += 1;
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

pub struct SemanticModel<'a> {
    file: &'a Path,
    identities: &'a IdentityIndex,
    types: Option<&'a TypeIndex>,
}

impl<'a> SemanticModel<'a> {
    pub fn new(
        file: &'a Path,
        identities: &'a IdentityIndex,
        types: Option<&'a TypeIndex>,
    ) -> Self {
        Self {
            file,
            identities,
            types,
        }
    }

    pub fn resolve_type(&self, ty: &DartType) -> ResolvedType {
        self.resolve_type_in(ty, &TypeParameterScope::default())
    }

    pub fn file_path(&self) -> &Path {
        self.file
    }

    pub fn extension_visible(&self, declaring_file: &Path, name: Option<&str>) -> bool {
        self.identities
            .extension_visible(self.file, declaring_file, name)
    }

    fn member_visible(&self, declaring_type: &DeclarationIdentity, name: &str) -> bool {
        !name.starts_with('_')
            || matches!(declaring_type, DeclarationIdentity::Project { library, .. }
                if self.identities.library_identity(self.file) == Some(*library))
    }

    pub fn resolve_name(&self, segments: &[String]) -> Option<DeclarationIdentity> {
        self.identities.resolve_declaration(self.file, segments)
    }

    pub fn resolve_value(&self, segments: &[String]) -> Option<DeclarationIdentity> {
        self.identities
            .resolve_value_declaration(self.file, segments)
    }

    pub fn resolve_sdk_member(&self, segments: &[String]) -> Option<DeclarationIdentity> {
        self.identities.resolve_sdk_member(self.file, segments)
    }

    pub fn resolve_imported_member(&self, segments: &[String]) -> Option<DeclarationIdentity> {
        self.identities.resolve_imported_member(self.file, segments)
    }

    pub fn resolve_type_in(&self, ty: &DartType, parameters: &TypeParameterScope) -> ResolvedType {
        match ty {
            DartType::Void { .. } => ResolvedType::Void,
            DartType::Dynamic { .. } => ResolvedType::Dynamic,
            DartType::Never { .. } => ResolvedType::Never,
            DartType::Named(named) => {
                if named.segments.len() == 1
                    && let Some(parameter) = parameters.lookup(&named.segments[0].name)
                {
                    return parameter.with_nullable(named.is_nullable);
                }
                let segments: Vec<String> = named
                    .segments
                    .iter()
                    .map(|segment| segment.name.clone())
                    .collect();
                let Some(identity) = self.identities.resolve_declaration(self.file, &segments)
                else {
                    return ResolvedType::Unknown;
                };
                if matches!(&identity, DeclarationIdentity::Sdk { library, name } if library == "dart:core" && name == "Null")
                {
                    return ResolvedType::Null;
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

    pub fn core_type(&self, name: &str, nullable: bool) -> ResolvedType {
        ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk {
                library: "dart:core".to_string(),
                name: name.to_string(),
            },
            arguments: Vec::new(),
            nullable,
            extension_type: false,
        }
    }

    pub fn is_instance_of(&self, ty: &ResolvedType, library: &str, name: &str) -> TypeTruth {
        let ResolvedType::Interface { identity, .. } = ty else {
            return if matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic) {
                TypeTruth::Unknown
            } else {
                TypeTruth::No
            };
        };
        if matches!(identity, DeclarationIdentity::Sdk { library: found_library, name: found_name }
            if found_library == library && found_name == name)
        {
            return TypeTruth::Yes;
        }
        let Some(actual_name) = subtype_name(identity) else {
            return TypeTruth::Unknown;
        };
        if matches!(identity, DeclarationIdentity::Project { .. }) && actual_name == name {
            return TypeTruth::No;
        }
        match self.types.map(|types| types.is_subtype(actual_name, name)) {
            Some(SubtypeResult::Yes) => TypeTruth::Yes,
            Some(SubtypeResult::ProvenNo) => TypeTruth::No,
            Some(SubtypeResult::Unknown) | None => TypeTruth::Unknown,
        }
    }

    pub fn is_future_like(&self, ty: &ResolvedType) -> TypeTruth {
        match ty {
            ResolvedType::Dynamic | ResolvedType::Null => TypeTruth::Yes,
            ResolvedType::Unknown => TypeTruth::Unknown,
            ResolvedType::Interface {
                extension_type: true,
                ..
            } => TypeTruth::Yes,
            ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk { library, name },
                ..
            } if library == "dart:async" && matches!(name.as_str(), "Future" | "FutureOr") => {
                TypeTruth::Yes
            }
            ResolvedType::TypeParameter { bound, .. } => self.is_future_like(bound),
            ResolvedType::Interface {
                identity: DeclarationIdentity::Project { name, .. },
                ..
            } => match self.types.map(|types| types.is_subtype(name, "Future")) {
                Some(SubtypeResult::Yes) => TypeTruth::Yes,
                Some(SubtypeResult::ProvenNo) => TypeTruth::No,
                Some(SubtypeResult::Unknown) | None => TypeTruth::Unknown,
            },
            _ => TypeTruth::No,
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
            || matches!(ty, ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library, name }, arguments, .. }
                if library == "dart:async" && matches!(name.as_str(), "Future" | "FutureOr")
                    && arguments.first().is_some_and(|argument| self.void_context(argument)))
    }

    pub fn acceptable_for_void_context(
        &self,
        expected: &ResolvedType,
        actual: &ResolvedType,
    ) -> TypeTruth {
        if matches!(actual, ResolvedType::Unknown) {
            return TypeTruth::Unknown;
        }
        if matches!(expected, ResolvedType::Void) {
            return if matches!(
                actual,
                ResolvedType::Void | ResolvedType::Null | ResolvedType::Never
            ) {
                TypeTruth::Yes
            } else {
                TypeTruth::No
            };
        }
        let ResolvedType::Interface {
            identity: DeclarationIdentity::Sdk { library, name },
            arguments,
            ..
        } = expected
        else {
            return TypeTruth::Unknown;
        };
        if library != "dart:async" || !matches!(name.as_str(), "Future" | "FutureOr") {
            return TypeTruth::Unknown;
        }
        if name == "FutureOr" && matches!(actual, ResolvedType::Dynamic) {
            return TypeTruth::Yes;
        }
        if matches!(
            actual,
            ResolvedType::Null | ResolvedType::Never | ResolvedType::Void
        ) && name == "FutureOr"
        {
            return TypeTruth::Yes;
        }
        let Some(expected_argument) = arguments.first() else {
            return TypeTruth::Unknown;
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
            return TypeTruth::No;
        };
        if actual_library != "dart:async" {
            return TypeTruth::No;
        }
        if actual_name == "Future" {
            return actual_arguments
                .first()
                .map(|argument| self.acceptable_for_void_context(expected_argument, argument))
                .unwrap_or(TypeTruth::Unknown);
        }
        if name == "FutureOr" && actual_name == "FutureOr" {
            return actual_arguments
                .first()
                .map(|argument| self.acceptable_for_void_context(expected_argument, argument))
                .unwrap_or(TypeTruth::Unknown);
        }
        TypeTruth::No
    }

    pub fn unrelated(&self, left: &ResolvedType, right: &ResolvedType) -> TypeTruth {
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
            return TypeTruth::Unknown;
        }
        if left == right {
            return TypeTruth::No;
        }
        if numeric(left) && numeric(right) {
            return TypeTruth::No;
        }
        if fixnum_and_int(left, right) || fixnum_and_int(right, left) {
            return TypeTruth::No;
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
                        return TypeTruth::Unknown;
                    }
                    return if left_arguments
                        .iter()
                        .zip(right_arguments)
                        .any(|(left, right)| self.unrelated(left, right) == TypeTruth::Yes)
                    {
                        TypeTruth::Yes
                    } else {
                        TypeTruth::No
                    };
                }
                let (Some(left_name), Some(right_name)) =
                    (subtype_name(left_id), subtype_name(right_id))
                else {
                    return TypeTruth::Unknown;
                };
                let Some(types) = self.types else {
                    return TypeTruth::Unknown;
                };
                match (
                    types.is_subtype(left_name, right_name),
                    types.is_subtype(right_name, left_name),
                ) {
                    (SubtypeResult::Yes, _) | (_, SubtypeResult::Yes) => TypeTruth::No,
                    (SubtypeResult::ProvenNo, SubtypeResult::ProvenNo) => TypeTruth::Yes,
                    _ => TypeTruth::Unknown,
                }
            }
            (ResolvedType::TypeParameter { bound, .. }, other)
            | (other, ResolvedType::TypeParameter { bound, .. }) => self.unrelated(bound, other),
            (
                ResolvedType::Function {
                    return_type: left_return,
                    positional: left_positional,
                    named: left_named,
                    ..
                },
                ResolvedType::Function {
                    return_type: right_return,
                    positional: right_positional,
                    named: right_named,
                    ..
                },
            ) => {
                if left_positional.len() != right_positional.len()
                    || left_named.iter().map(|(name, _)| name).collect::<Vec<_>>()
                        != right_named.iter().map(|(name, _)| name).collect::<Vec<_>>()
                {
                    return TypeTruth::Yes;
                }
                let pairs = left_positional.iter().zip(right_positional).chain(
                    left_named
                        .iter()
                        .zip(right_named)
                        .map(|((_, left), (_, right))| (left, right)),
                );
                if self.unrelated(left_return, right_return) == TypeTruth::Yes
                    || pairs
                        .into_iter()
                        .any(|(left, right)| self.unrelated(left, right) == TypeTruth::Yes)
                {
                    TypeTruth::Yes
                } else {
                    TypeTruth::No
                }
            }
            (
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
                    || left_named.iter().map(|(name, _)| name).collect::<Vec<_>>()
                        != right_named.iter().map(|(name, _)| name).collect::<Vec<_>>()
                {
                    return TypeTruth::Yes;
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
                    .any(|(left, right)| self.unrelated(left, right) == TypeTruth::Yes)
                {
                    TypeTruth::Yes
                } else {
                    TypeTruth::No
                }
            }
            _ => TypeTruth::Yes,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticMemberKind {
    Method,
    Getter,
    Setter,
    Operator,
    Field,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberFacts {
    pub declaring_type: DeclarationIdentity,
    pub kind: SemanticMemberKind,
    pub is_static: bool,
    pub is_abstract: bool,
    pub is_covariant: bool,
    pub has_getter: bool,
    pub has_setter: bool,
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
    declaring_file: PathBuf,
    name: Option<String>,
    on_type: ResolvedType,
    members: HashSet<String>,
}

#[derive(Debug, Clone, Copy)]
enum InstanceAccess {
    Read,
    Write,
    Invoke,
}

#[derive(Debug, Clone, Default)]
pub struct SignatureIndex {
    functions: HashMap<DeclarationIdentity, ResolvedSignature>,
    constructors: HashMap<(DeclarationIdentity, String), ResolvedSignature>,
    members: HashMap<(DeclarationIdentity, String), ResolvedSignature>,
    instance_members: HashSet<(DeclarationIdentity, String)>,
    fields: HashMap<(DeclarationIdentity, String), (Vec<TypeParameterId>, ResolvedType)>,
    instance_reads: HashMap<(DeclarationIdentity, String), (Vec<TypeParameterId>, ResolvedType)>,
    instance_writes: HashMap<(DeclarationIdentity, String), (Vec<TypeParameterId>, ResolvedType)>,
    member_facts: HashMap<(DeclarationIdentity, String), Vec<MemberFacts>>,
    supertypes: HashMap<DeclarationIdentity, (Vec<TypeParameterId>, Vec<ResolvedType>)>,
    member_supertypes: HashMap<DeclarationIdentity, Vec<ResolvedType>>,
    declarations: HashMap<DeclarationIdentity, DeclarationFacts>,
    constants: HashMap<DeclarationIdentity, Expr>,
    constant_types: HashMap<DeclarationIdentity, DeclarationIdentity>,
    extensions: Vec<ExtensionFacts>,
}

impl SignatureIndex {
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
                    if let Some(identity) =
                        model.resolve_value(std::slice::from_ref(&function.name.name))
                    {
                        self.functions.insert(
                            identity,
                            signature(
                                &parameters,
                                &[],
                                &function.type_params,
                                &function.params,
                                function.return_type.as_ref(),
                                None,
                                model,
                            ),
                        );
                    }
                }
                TopLevelDecl::Variable(variable) if variable.is_const => {
                    for declarator in &variable.declarators {
                        if let Some(initializer) = &declarator.initializer
                            && let Some(identity) =
                                model.resolve_value(std::slice::from_ref(&declarator.name.name))
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
                        declaring_file: model.file_path().to_path_buf(),
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
                let Some(identity) = model.resolve_name(std::slice::from_ref(name)) else {
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
                    for parameter in constructor
                        .params
                        .positional
                        .iter()
                        .chain(&constructor.params.optional_positional)
                        .chain(&constructor.params.named)
                        .filter(|parameter| parameter.is_super && parameter.param_type.is_none())
                    {
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
            .get_mut(&(identity.clone(), constructor_name.to_string()))
        {
            if params
                .named
                .iter()
                .any(|named| std::ptr::eq(named, parameter))
            {
                if signature.named.get(&parameter.name.name) != Some(&ty) {
                    signature
                        .named
                        .insert(parameter.name.name.clone(), ty.clone());
                    changed = true;
                }
            } else if let Some(index) = params
                .positional
                .iter()
                .chain(&params.optional_positional)
                .position(|candidate| std::ptr::eq(candidate, parameter))
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
        let Some(identity) = model.resolve_name(&[name.to_string()]) else {
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
                let segments = annotation
                    .name
                    .iter()
                    .map(|segment| segment.name.clone())
                    .collect::<Vec<_>>();
                model.resolve_imported_member(&segments)
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
                        .positional
                        .iter()
                        .chain(&constructor.params.optional_positional)
                        .chain(&constructor.params.named)
                        .map(|parameter| ParameterFacts {
                            name: parameter.name.name.clone(),
                            ty: parameter
                                .param_type
                                .as_ref()
                                .map(|ty| model.resolve_type_in(ty, &scope))
                                .or_else(|| {
                                    parameter.is_field.then(|| {
                                        let key = (identity.clone(), parameter.name.name.clone());
                                        self.member_facts
                                            .get(&key)
                                            .filter(|facts| {
                                                facts.iter().any(|fact| {
                                                    fact.kind == SemanticMemberKind::Field
                                                        && !fact.is_static
                                                })
                                            })
                                            .and_then(|_| self.fields.get(&key))
                                            .map(|(_, ty)| ty.clone())
                                            .unwrap_or(ResolvedType::Unknown)
                                    })
                                })
                                .unwrap_or(ResolvedType::Unknown),
                            is_named: constructor
                                .params
                                .named
                                .iter()
                                .any(|named| std::ptr::eq(named, parameter)),
                            is_super: parameter.is_super,
                        })
                        .collect(),
                }),
                _ => None,
            })
            .collect();
        let static_consts = members
            .iter()
            .filter_map(|member| match member {
                ClassMember::Field(field) if field.is_static && field.is_const => Some(field),
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
                    package: "test".to_string(),
                    name: name.to_string(),
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
        let mut add = |library: &str,
                       type_name: &str,
                       arity: usize,
                       member: &str,
                       positional: Vec<ResolvedType>,
                       return_type: ResolvedType| {
            let parameters: Vec<_> = (0..arity)
                .map(|ordinal| TypeParameterId {
                    owner: sdk_owner(type_name),
                    ordinal,
                    name: format!("{type_name}#{ordinal}"),
                })
                .collect();
            let key = (
                DeclarationIdentity::Sdk {
                    library: library.to_string(),
                    name: type_name.to_string(),
                },
                member.to_string(),
            );
            self.instance_members.insert(key.clone());
            self.members.insert(
                key,
                ResolvedSignature {
                    owner_parameters: parameters,
                    call_parameters: Vec::new(),
                    positional,
                    named: HashMap::new(),
                    return_type,
                },
            );
        };
        let parameter =
            |type_name: &str, _arity: usize, ordinal: usize| ResolvedType::TypeParameter {
                id: TypeParameterId {
                    owner: sdk_owner(type_name),
                    ordinal,
                    name: format!("{type_name}#{ordinal}"),
                },
                bound: Box::new(ResolvedType::Dynamic),
                nullable: false,
            };
        let object = DeclarationIdentity::Sdk {
            library: "dart:core".to_string(),
            name: "Object".to_string(),
        };
        for (name, kind, parameters) in [
            (
                "==",
                SemanticMemberKind::Operator,
                vec!["other".to_string()],
            ),
            (
                "noSuchMethod",
                SemanticMemberKind::Method,
                vec!["invocation".to_string()],
            ),
            ("hashCode", SemanticMemberKind::Getter, Vec::new()),
            ("runtimeType", SemanticMemberKind::Getter, Vec::new()),
            ("toString", SemanticMemberKind::Method, Vec::new()),
        ] {
            self.member_facts.insert(
                (object.clone(), name.to_string()),
                vec![MemberFacts {
                    declaring_type: object.clone(),
                    kind,
                    is_static: false,
                    is_abstract: false,
                    is_covariant: false,
                    has_getter: kind == SemanticMemberKind::Getter,
                    has_setter: kind == SemanticMemberKind::Setter,
                    positional_parameter_names: parameters,
                }],
            );
        }
        add(
            "dart:core",
            "List",
            1,
            "add",
            vec![parameter("List", 1, 0)],
            ResolvedType::Void,
        );
        add(
            "dart:core",
            "List",
            1,
            "[]=",
            vec![ResolvedType::Dynamic, parameter("List", 1, 0)],
            ResolvedType::Void,
        );
        add(
            "dart:core",
            "Set",
            1,
            "add",
            vec![parameter("Set", 1, 0)],
            ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core".to_string(),
                    name: "bool".to_string(),
                },
                arguments: Vec::new(),
                nullable: false,
                extension_type: false,
            },
        );
        add(
            "dart:collection",
            "Queue",
            1,
            "add",
            vec![parameter("Queue", 1, 0)],
            ResolvedType::Void,
        );
        add(
            "dart:core",
            "Map",
            2,
            "[]=",
            vec![parameter("Map", 2, 0), parameter("Map", 2, 1)],
            ResolvedType::Void,
        );
        for (library, name) in [
            ("dart:core", "List"),
            ("dart:core", "Set"),
            ("dart:collection", "Queue"),
        ] {
            let parameter = parameter(name, 1, 0);
            let identity = DeclarationIdentity::Sdk {
                library: library.to_string(),
                name: name.to_string(),
            };
            let supertype = ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core".to_string(),
                    name: "Iterable".to_string(),
                },
                arguments: vec![parameter.clone()],
                nullable: false,
                extension_type: false,
            };
            self.supertypes.insert(
                identity.clone(),
                (
                    vec![match &parameter {
                        ResolvedType::TypeParameter { id, .. } => id.clone(),
                        _ => unreachable!(),
                    }],
                    vec![supertype.clone()],
                ),
            );
            self.member_supertypes.insert(identity, vec![supertype]);
        }
        let string_to_upper_case = (
            DeclarationIdentity::Sdk {
                library: "dart:core".to_string(),
                name: "String".to_string(),
            },
            "toUpperCase".to_string(),
        );
        self.instance_members.insert(string_to_upper_case.clone());
        self.members.insert(
            string_to_upper_case,
            ResolvedSignature {
                owner_parameters: Vec::new(),
                call_parameters: Vec::new(),
                positional: Vec::new(),
                named: HashMap::new(),
                return_type: ResolvedType::Interface {
                    identity: DeclarationIdentity::Sdk {
                        library: "dart:core".to_string(),
                        name: "String".to_string(),
                    },
                    arguments: Vec::new(),
                    nullable: false,
                    extension_type: false,
                },
            },
        );
        let iterable_map = (
            DeclarationIdentity::Sdk {
                library: "dart:core".to_string(),
                name: "Iterable".to_string(),
            },
            "map".to_string(),
        );
        let map_parameter = TypeParameterId {
            owner: sdk_owner("Iterable"),
            ordinal: 1,
            name: "map#T".to_string(),
        };
        let map_type = ResolvedType::TypeParameter {
            id: map_parameter.clone(),
            bound: Box::new(ResolvedType::Dynamic),
            nullable: false,
        };
        self.instance_members.insert(iterable_map.clone());
        self.members.insert(
            iterable_map,
            ResolvedSignature {
                owner_parameters: vec![TypeParameterId {
                    owner: sdk_owner("Iterable"),
                    ordinal: 0,
                    name: "Iterable#0".to_string(),
                }],
                call_parameters: vec![map_parameter],
                positional: vec![ResolvedType::Function {
                    return_type: Box::new(map_type.clone()),
                    positional: vec![parameter("Iterable", 1, 0)],
                    named: Vec::new(),
                    nullable: false,
                }],
                named: HashMap::new(),
                return_type: ResolvedType::Interface {
                    identity: DeclarationIdentity::Sdk {
                        library: "dart:core".to_string(),
                        name: "Iterable".to_string(),
                    },
                    arguments: vec![map_type],
                    nullable: false,
                    extension_type: false,
                },
            },
        );

        let widget = DeclarationIdentity::Package {
            package: "flutter".to_string(),
            name: "Widget".to_string(),
        };
        let key = ResolvedType::Interface {
            identity: DeclarationIdentity::Package {
                package: "flutter".to_string(),
                name: "Key".to_string(),
            },
            arguments: Vec::new(),
            nullable: true,
            extension_type: false,
        };
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
        ] {
            let identity = DeclarationIdentity::Package {
                package: "flutter".to_string(),
                name: name.to_string(),
            };
            let supertype = ResolvedType::Interface {
                identity: widget.clone(),
                arguments: Vec::new(),
                nullable: false,
                extension_type: false,
            };
            self.supertypes
                .insert(identity.clone(), (Vec::new(), vec![supertype.clone()]));
            self.member_supertypes
                .insert(identity.clone(), vec![supertype]);
            self.constructors.insert(
                (identity, "new".to_string()),
                ResolvedSignature {
                    owner_parameters: Vec::new(),
                    call_parameters: Vec::new(),
                    positional: Vec::new(),
                    named: HashMap::from([("key".to_string(), key.clone())]),
                    return_type: ResolvedType::Unknown,
                },
            );
        }
        self.constructors.insert(
            (widget, "new".to_string()),
            ResolvedSignature {
                owner_parameters: Vec::new(),
                call_parameters: Vec::new(),
                positional: Vec::new(),
                named: HashMap::from([("key".to_string(), key)]),
                return_type: ResolvedType::Unknown,
            },
        );
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
        let Some(identity) = model.resolve_name(&[name.to_string()]) else {
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
        for (field_name, field_type) in &field_types {
            self.fields.insert(
                (identity.clone(), field_name.clone()),
                (owner_parameters.clone(), field_type.clone()),
            );
        }
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
        for member in members {
            match member {
                ClassMember::Method(method) => {
                    self.member_facts
                        .entry((identity.clone(), method.name.name.clone()))
                        .or_default()
                        .push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind: SemanticMemberKind::Method,
                            is_static: method.is_static,
                            is_abstract: method.is_abstract,
                            is_covariant: false,
                            has_getter: false,
                            has_setter: false,
                            positional_parameter_names: positional_parameter_names(&method.params),
                        });
                    scope.push(&method.type_params, model);
                    let key = (identity.clone(), method.name.name.clone());
                    if !method.is_static {
                        self.instance_members.insert(key.clone());
                    }
                    self.members.insert(
                        key,
                        signature(
                            &scope,
                            &owner_parameters,
                            &method.type_params,
                            &method.params,
                            method.return_type.as_ref(),
                            None,
                            model,
                        ),
                    );
                    scope.pop();
                }
                ClassMember::Constructor(constructor) => {
                    self.constructors.insert(
                        (
                            identity.clone(),
                            constructor
                                .constructor_name
                                .as_ref()
                                .map(|name| name.name.clone())
                                .unwrap_or_else(|| "new".to_string()),
                        ),
                        signature(
                            &scope,
                            &owner_parameters,
                            &[],
                            &constructor.params,
                            None,
                            Some(&field_types),
                            model,
                        ),
                    );
                }
                ClassMember::Field(field) => {
                    for declarator in &field.declarators {
                        self.member_facts
                            .entry((identity.clone(), declarator.name.name.clone()))
                            .or_default()
                            .push(MemberFacts {
                                declaring_type: identity.clone(),
                                kind: SemanticMemberKind::Field,
                                is_static: field.is_static,
                                is_abstract: field.is_abstract,
                                is_covariant: field.is_covariant,
                                has_getter: true,
                                has_setter: !field.is_final && !field.is_const,
                                positional_parameter_names: Vec::new(),
                            });
                    }
                    let field_type = field
                        .field_type
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Unknown);
                    for declarator in &field.declarators {
                        let key = (identity.clone(), declarator.name.name.clone());
                        let indexed_type = (owner_parameters.clone(), field_type.clone());
                        self.fields.insert(key.clone(), indexed_type.clone());
                        if !field.is_static {
                            self.instance_reads
                                .insert(key.clone(), indexed_type.clone());
                            if !field.is_final && !field.is_const {
                                self.instance_writes.insert(key, indexed_type);
                            }
                        }
                    }
                }
                ClassMember::Getter(getter) => {
                    self.member_facts
                        .entry((identity.clone(), getter.name.name.clone()))
                        .or_default()
                        .push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind: SemanticMemberKind::Getter,
                            is_static: getter.is_static,
                            is_abstract: getter.is_abstract,
                            is_covariant: false,
                            has_getter: true,
                            has_setter: false,
                            positional_parameter_names: Vec::new(),
                        });
                    let return_type = getter
                        .return_type
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Unknown);
                    let key = (identity.clone(), getter.name.name.clone());
                    let indexed_type = (owner_parameters.clone(), return_type);
                    self.fields.insert(key.clone(), indexed_type.clone());
                    if !getter.is_static {
                        self.instance_reads.insert(key, indexed_type);
                    }
                }
                ClassMember::Setter(setter) => {
                    self.member_facts
                        .entry((identity.clone(), setter.name.name.clone()))
                        .or_default()
                        .push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind: SemanticMemberKind::Setter,
                            is_static: setter.is_static,
                            is_abstract: setter.is_abstract,
                            is_covariant: false,
                            has_getter: false,
                            has_setter: true,
                            positional_parameter_names: vec![setter.param.name.clone()],
                        });
                    let parameter_type = setter
                        .param_type
                        .as_ref()
                        .map(|ty| model.resolve_type_in(ty, &scope))
                        .unwrap_or(ResolvedType::Dynamic);
                    let key = (identity.clone(), setter.name.name.clone());
                    let indexed_type = (owner_parameters.clone(), parameter_type);
                    self.fields
                        .entry(key.clone())
                        .or_insert_with(|| indexed_type.clone());
                    if !setter.is_static {
                        self.instance_writes.insert(key, indexed_type);
                    }
                }
                ClassMember::Operator(operator) => {
                    self.member_facts
                        .entry((identity.clone(), operator.op.clone()))
                        .or_default()
                        .push(MemberFacts {
                            declaring_type: identity.clone(),
                            kind: SemanticMemberKind::Operator,
                            is_static: false,
                            is_abstract: operator.body.is_none() && !operator.is_external,
                            is_covariant: false,
                            has_getter: false,
                            has_setter: false,
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
    ) -> TypeTruth {
        if matches!(ty, ResolvedType::Unknown | ResolvedType::Dynamic) {
            return TypeTruth::Unknown;
        }
        if self.instantiated_identity(ty, target, model).is_some() {
            TypeTruth::Yes
        } else {
            TypeTruth::Unknown
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
        self.constructors.get(&(identity.clone(), name.to_string()))
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
        self.members.get(&(identity.clone(), name.to_string()))
    }

    fn has_instance_declaration(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        access: InstanceAccess,
    ) -> bool {
        let key = (identity.clone(), name.to_string());
        (match access {
            InstanceAccess::Read => {
                self.instance_members.contains(&key) || self.instance_reads.contains_key(&key)
            }
            InstanceAccess::Write => self.instance_writes.contains_key(&key),
            InstanceAccess::Invoke => {
                self.instance_members.contains(&key) || self.instance_reads.contains_key(&key)
            }
        }) || self.member_facts.get(&key).is_some_and(|facts| {
            facts.iter().any(|fact| {
                !fact.is_static
                    && (match access {
                        InstanceAccess::Read => matches!(
                            fact.kind,
                            SemanticMemberKind::Method
                                | SemanticMemberKind::Getter
                                | SemanticMemberKind::Field
                        ),
                        InstanceAccess::Write => fact.has_setter,
                        InstanceAccess::Invoke => matches!(
                            fact.kind,
                            SemanticMemberKind::Method
                                | SemanticMemberKind::Getter
                                | SemanticMemberKind::Field
                                | SemanticMemberKind::Operator
                        ),
                    })
            })
        })
    }

    fn instance_member(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&ResolvedSignature> {
        let key = (identity.clone(), name.to_string());
        self.instance_members
            .contains(&key)
            .then(|| self.members.get(&key))
            .flatten()
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
        if !name.starts_with('_') || same_library(owner, identity) {
            facts.extend(
                self.member_facts
                    .get(&(identity.clone(), name.to_string()))
                    .into_iter()
                    .flatten()
                    .filter(|member| !member.is_static)
                    .cloned(),
            );
        }
        if let Some((_, supertypes)) = self.supertypes.get(identity) {
            for supertype in supertypes {
                self.collect_inherited_member_facts(owner, supertype, name, visited, facts)?;
            }
        } else if !matches!(identity, DeclarationIdentity::Sdk { library, name } if library == "dart:core" && name == "Object")
        {
            return None;
        }
        Some(())
    }

    pub fn unrelated(
        &self,
        left: &ResolvedType,
        right: &ResolvedType,
        model: &SemanticModel<'_>,
    ) -> TypeTruth {
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
        if matches!(identity, DeclarationIdentity::Sdk { library: found_library, name: found_name }
            if found_library == library && found_name == name)
        {
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

    pub fn has_applicable_extension_member(
        &self,
        receiver: &ResolvedType,
        name: &str,
        model: &SemanticModel<'_>,
    ) -> bool {
        self.extensions.iter().any(|extension| {
            extension.members.contains(name)
                && model.extension_visible(&extension.declaring_file, extension.name.as_deref())
                && match &extension.on_type {
                    ResolvedType::Interface {
                        identity: target, ..
                    } => self.is_subtype_of(receiver, target, model) == TypeTruth::Yes,
                    ResolvedType::Dynamic => true,
                    ResolvedType::Unknown => false,
                    target => self.unrelated(receiver, target, model) != TypeTruth::Yes,
                }
        })
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
        let object = model.core_type("Object", false);
        let ResolvedType::Interface { identity, .. } = &object else {
            unreachable!()
        };
        self.has_instance_declaration(identity, name, access)
            .then_some(object)
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
        if matches!(identity, DeclarationIdentity::Sdk { library, name } if library == "dart:core" && name == "Object")
        {
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
        self.member_facts
            .get(&(owner, name.to_string()))
            .map(Vec::as_slice)
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
        let (parameters, field) = self.instance_reads.get(&(identity, name.to_string()))?;
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
        let (parameters, field) = self.instance_writes.get(&(identity, name.to_string()))?;
        let substitutions = parameters.iter().cloned().zip(arguments).collect();
        Some((field.clone(), substitutions))
    }

    pub fn field(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
    ) -> Option<&(Vec<TypeParameterId>, ResolvedType)> {
        self.fields.get(&(identity.clone(), name.to_string()))
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
            let mut segments = expression_segments(callee)?;
            model.resolve_name(&segments).or_else(|| {
                segments.pop();
                model.resolve_name(&segments)
            })
        }
        _ => None,
    }
}

fn expression_segments(expression: &Expr) -> Option<Vec<String>> {
    let mut current = expression;
    let mut segments = Vec::new();
    loop {
        match current {
            Expr::Ident(identifier) => {
                segments.push(identifier.name.clone());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field { object, field, .. } => {
                segments.push(field.name.clone());
                current = object;
            }
            _ => return None,
        }
    }
}

fn annotation_targets_library(
    annotation: &falcon_syntax::ast::Annotation,
    model: &SemanticModel<'_>,
) -> bool {
    let segments = annotation
        .name
        .iter()
        .map(|segment| segment.name.clone())
        .collect::<Vec<_>>();
    if !matches!(
        model.resolve_imported_member(&segments),
        Some(DeclarationIdentity::Package { package, name })
            if package == "meta" && name == "Target"
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
            let Some(segments) = expression_name(object) else {
                return false;
            };
            matches!(
                model.resolve_imported_member(&segments),
                Some(DeclarationIdentity::Package { package, name })
                    if package == "meta" && name == "TargetKind"
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
        Expr::Call { callee, .. } => {
            let Some(mut segments) = expression_name(callee) else {
                return ResolvedType::Unknown;
            };
            let identity = model.resolve_name(&segments).or_else(|| {
                segments.pop();
                model.resolve_name(&segments)
            });
            identity.map_or(ResolvedType::Unknown, |identity| ResolvedType::Interface {
                identity,
                arguments: Vec::new(),
                nullable: false,
                extension_type: false,
            })
        }
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

fn expression_name(expression: &Expr) -> Option<Vec<String>> {
    let mut current = expression;
    let mut segments = Vec::new();
    loop {
        match current {
            Expr::Ident(identifier) => {
                segments.push(identifier.name.clone());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field { object, field, .. } => {
                segments.push(field.name.clone());
                current = object;
            }
            _ => return None,
        }
    }
}

fn positional_parameter_names(params: &FormalParamList) -> Vec<String> {
    params
        .positional
        .iter()
        .chain(&params.optional_positional)
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
    params: &FormalParamList,
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
                if param.is_field {
                    field_types
                        .and_then(|fields| fields.get(&param.name.name))
                        .cloned()
                        .unwrap_or(ResolvedType::Unknown)
                } else if param.is_super {
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
            .positional
            .iter()
            .chain(&params.optional_positional)
            .map(&parameter_type)
            .collect(),
        named: params
            .named
            .iter()
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
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
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
        if let Expr::Field { object, field, .. } = expr {
            let receiver = self.infer_with_signatures(object, model, signatures, type_parameters);
            if let Some((original, substitutions)) =
                signatures.resolved_field(&receiver, &field.name, model)
            {
                return model.substitute(&original, &substitutions);
            }
        }
        if let Expr::Call {
            callee, type_args, ..
        } = expr
        {
            match callee.as_ref() {
                Expr::Ident(identifier) if !self.is_bound(&identifier.name) => {
                    if let Some(identity) =
                        model.resolve_value(std::slice::from_ref(&identifier.name))
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
                    if let Some(identity) =
                        model.resolve_name(std::slice::from_ref(&identifier.name))
                    {
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
                Expr::Field { object, field, .. } => {
                    let receiver =
                        self.infer_with_signatures(object, model, signatures, type_parameters);
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
                        return model.substitute(&signature.return_type, &substitutions);
                    }
                }
                Expr::GenericInstantiation {
                    target,
                    type_args: instantiated,
                    ..
                } => {
                    if let Expr::Ident(identifier) = target.as_ref()
                        && let Some(identity) =
                            model.resolve_value(std::slice::from_ref(&identifier.name))
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
            Expr::As { dart_type, .. } | Expr::New { dart_type, .. } => model
                .resolve_type_in(dart_type, type_parameters)
                .with_nullable(false),
            Expr::List { type_arg, .. } => ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core".to_string(),
                    name: "List".to_string(),
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
                    library: "dart:core".to_string(),
                    name: "Set".to_string(),
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
                    library: "dart:core".to_string(),
                    name: "Map".to_string(),
                },
                arguments: type_args
                    .iter()
                    .map(|ty| model.resolve_type_in(ty, type_parameters))
                    .collect(),
                nullable: false,
                extension_type: false,
            },
            Expr::FuncExpr {
                type_params,
                params,
                body,
                ..
            } => {
                let mut function_parameters = type_parameters.clone();
                function_parameters.push(type_params, model);
                let positional = params
                    .positional
                    .iter()
                    .chain(&params.optional_positional)
                    .map(|param| {
                        param
                            .param_type
                            .as_ref()
                            .map(|ty| model.resolve_type_in(ty, &function_parameters))
                            .unwrap_or(ResolvedType::Dynamic)
                    })
                    .collect();
                let named = params
                    .named
                    .iter()
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
                    FunctionBody::Arrow(expression, _) => self.infer_with_signatures(
                        expression,
                        model,
                        signatures,
                        &function_parameters,
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
            _ => self.infer(expr, model),
        }
    }

    pub fn infer(&self, expr: &Expr, model: &SemanticModel<'_>) -> ResolvedType {
        match expr {
            Expr::IntLit { .. } => model.core_type("int", false),
            Expr::DoubleLit { .. } => model.core_type("double", false),
            Expr::StringLit(_) => model.core_type("String", false),
            Expr::BoolLit { .. } => model.core_type("bool", false),
            Expr::NullLit { .. } => ResolvedType::Null,
            Expr::Ident(identifier) => self.lookup(&identifier.name),
            Expr::As { dart_type, .. } => model.resolve_type(dart_type),
            Expr::NullAssert { operand, .. } => self.infer(operand, model).with_nullable(false),
            Expr::New { dart_type, .. } => model.resolve_type(dart_type).with_nullable(false),
            Expr::List { type_arg, .. } => ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core".to_string(),
                    name: "List".to_string(),
                },
                arguments: vec![
                    type_arg
                        .as_ref()
                        .map(|ty| model.resolve_type(ty))
                        .unwrap_or(ResolvedType::Dynamic),
                ],
                nullable: false,
                extension_type: false,
            },
            Expr::Set { type_arg, .. } => ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core".to_string(),
                    name: "Set".to_string(),
                },
                arguments: vec![
                    type_arg
                        .as_ref()
                        .map(|ty| model.resolve_type(ty))
                        .unwrap_or(ResolvedType::Dynamic),
                ],
                nullable: false,
                extension_type: false,
            },
            Expr::Map { type_args, .. } => ResolvedType::Interface {
                identity: DeclarationIdentity::Sdk {
                    library: "dart:core".to_string(),
                    name: "Map".to_string(),
                },
                arguments: type_args.iter().map(|ty| model.resolve_type(ty)).collect(),
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
                _ => {
                    let left = self.infer(left, model);
                    let right = self.infer(right, model);
                    if left == right {
                        left
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
                let then_type = self.infer(then_expr, model);
                let else_type = self.infer(else_expr, model);
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

fn sdk_owner(type_name: &str) -> usize {
    match type_name {
        "Iterable" => usize::MAX - 1,
        "List" => usize::MAX - 2,
        "Set" => usize::MAX - 3,
        "Queue" => usize::MAX - 4,
        "Map" => usize::MAX - 5,
        _ => usize::MAX - 100,
    }
}

fn subtype_name(identity: &DeclarationIdentity) -> Option<&str> {
    match identity {
        DeclarationIdentity::Project { name, .. } | DeclarationIdentity::Sdk { name, .. } => {
            Some(name)
        }
        DeclarationIdentity::Package { .. } => None,
    }
}

fn numeric(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library, name }, .. }
        if library == "dart:core" && matches!(name.as_str(), "int" | "double" | "num"))
}

fn fixnum_and_int(fixnum: &ResolvedType, int: &ResolvedType) -> bool {
    matches!(fixnum, ResolvedType::Interface { identity: DeclarationIdentity::Package { package, name }, .. }
        if package == "fixnum" && matches!(name.as_str(), "Int32" | "Int64"))
        && matches!(int, ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library, name }, .. }
            if library == "dart:core" && name == "int")
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
    fn resolves_canonical_generic_and_special_types() {
        with_model(
            "import 'dart:async'; class Box<T> {} void f(Box<String?> box, Future<void> future) {}",
            |program, model| {
                let TopLevelDecl::Function(function) = &program.declarations[1] else {
                    panic!()
                };
                let box_type =
                    model.resolve_type(function.params.positional[0].param_type.as_ref().unwrap());
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

                let future =
                    model.resolve_type(function.params.positional[1].param_type.as_ref().unwrap());
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
                    .params
                    .positional
                    .iter()
                    .map(|param| model.resolve_type(param.param_type.as_ref().unwrap()))
                    .collect();
                assert_eq!(model.unrelated(&types[0], &types[1]), TypeTruth::No);
                assert_eq!(model.unrelated(&types[0], &types[2]), TypeTruth::Yes);
                assert_eq!(model.unrelated(&types[0], &types[3]), TypeTruth::Unknown);
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
                    .function(&model.resolve_value(&["compare".to_string()]).unwrap())
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
                .function(&model.resolve_value(&["accept".to_string()]).unwrap())
                .unwrap();
            assert!(contains_parameter(&signature.positional[0]));
            let substitutions =
                HashMap::from([(signature.call_parameters[0].clone(), ResolvedType::Void)]);
            let expected = model.substitute(&signature.positional[0], &substitutions);
            assert_eq!(expected, ResolvedType::Void);
            assert_eq!(
                model.acceptable_for_void_context(&expected, &model.core_type("int", false)),
                TypeTruth::No
            );
            assert_eq!(
                model.acceptable_for_void_context(&expected, &ResolvedType::Null),
                TypeTruth::Yes
            );
        });
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
                    environment.bind_params(&function.params, model, &type_parameters);

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
    fn implementation_lookup_prefers_later_mixins_and_excludes_interfaces() {
        with_model(
            "class Base { num value() => 0; num field = 0; } mixin First<T> { T value() => throw UnimplementedError(); T field; } mixin Second<T> { T value() => throw UnimplementedError(); T field; } abstract class Contract { bool value(); bool field; int only(); } class Child extends Base with First<String>, Second<int> implements Contract {} class InterfaceOnly implements Contract {}",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let child_identity = model.resolve_name(&["Child".to_string()]).unwrap();
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
                    model.resolve_name(&["Second".to_string()])
                );

                let contract = model.resolve_name(&["Contract".to_string()]).unwrap();
                let interface_only = ResolvedType::Interface {
                    identity: model.resolve_name(&["InterfaceOnly".to_string()]).unwrap(),
                    arguments: Vec::new(),
                    nullable: false,
                    extension_type: false,
                };
                assert_eq!(
                    signatures.is_subtype_of(&interface_only, &contract, model),
                    TypeTruth::Yes
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
                    identity: model.resolve_name(&["Child".to_string()]).unwrap(),
                    arguments: Vec::new(),
                    nullable: false,
                    extension_type: false,
                };
                let base = model.resolve_name(&["Base".to_string()]).unwrap();
                let mixin = model.resolve_name(&["StaticMembers".to_string()]).unwrap();

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
        with_model(
            "class Value { String get value => ''; set value(num next) {} }",
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let identity = model.resolve_name(&["Value".to_string()]).unwrap();

                assert!(
                    signatures
                        .field(&identity, "value")
                        .unwrap()
                        .1
                        .interface("dart:core", "String")
                );
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
                    identity: model.resolve_name(&["Child".to_string()]).unwrap(),
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
                let base = model.resolve_name(&["Base".to_string()]).unwrap();
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

                let child = model.resolve_name(&["Child".to_string()]).unwrap();
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
                    identity: model.resolve_name(&["Child".to_string()]).unwrap(),
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
                    identity: model.resolve_name(&["Child".to_string()]).unwrap(),
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
                    identity: model.resolve_name(&["Child".to_string()]).unwrap(),
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
                        identity: model.resolve_name(&[class_name.to_string()]).unwrap(),
                        arguments: Vec::new(),
                        nullable: false,
                        extension_type: false,
                    };
                    assert_eq!(
                        signatures.resolved_member_owner(&receiver, "toString", model),
                        model.resolve_name(&[owner_name.to_string()])
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
            let receiver =
                model.resolve_type(function.params.positional[0].param_type.as_ref().unwrap());
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
                    library: "dart:core".to_string(),
                    name: "Iterable".to_string(),
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
                environment.bind_params(&function.params, model, &type_parameters);
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
            identity: model.resolve_name(&["Child".to_string()]).unwrap(),
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
            identity: model.resolve_name(&["Child".to_string()]).unwrap(),
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
        let base_identity = model.resolve_name(&["Base".to_string()]).unwrap();
        let child = ResolvedType::Interface {
            identity: model.resolve_name(&["Child".to_string()]).unwrap(),
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
                let a = model.resolve_name(&["A".to_string()]).unwrap();
                let b = model.resolve_name(&["B".to_string()]).unwrap();
                let receiver = ResolvedType::Interface {
                    identity: a,
                    arguments: Vec::new(),
                    nullable: false,
                    extension_type: false,
                };

                assert_eq!(
                    signatures.is_subtype_of(&receiver, &b, model),
                    TypeTruth::Unknown
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
                let base = model.resolve_name(&["Base".to_string()]).unwrap();
                assert!(contains_parameter(
                    &signatures.declaration(&base).unwrap().constructors[0].parameters[0].ty
                ));

                for name in ["Middle", "Leaf"] {
                    let identity = model.resolve_name(&[name.to_string()]).unwrap();
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
        let leaf_identity = model.resolve_name(&["Leaf".to_string()]).unwrap();
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
        let leaf = model.resolve_name(&["A6".to_string()]).unwrap();

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
