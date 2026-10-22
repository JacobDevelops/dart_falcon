use falcon_analyze::semantic_scope::SemanticState;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, DocName, QualifiedName, ResolvedType, RulePreparation,
    SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

pub(crate) struct BlocTypes<'a> {
    program: Program<'a>,
}

impl<'a> BlocTypes<'a> {
    pub(crate) fn new(program: Program<'a>) -> Self {
        Self { program }
    }

    pub(crate) fn is_api_type(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
        name: &str,
    ) -> bool {
        let ResolvedType::Interface { identity, .. } = ty else {
            return false;
        };
        is_api_identity(identity, model, name)
    }

    pub(crate) fn is_api_syntax(
        &self,
        ty: DartType<'_>,
        model: &SemanticModel<'_>,
        name: &str,
    ) -> bool {
        let resolved = model.resolve_type(ty);
        self.is_resolved_api_syntax(ty, &resolved, model, name)
    }

    fn is_resolved_api_syntax(
        &self,
        ty: DartType<'_>,
        resolved: &ResolvedType,
        model: &SemanticModel<'_>,
        name: &str,
    ) -> bool {
        if self.is_api_type(resolved, model, name) {
            return true;
        }
        // Only absent external declarations use the import fallback; a local declaration wins.
        if !matches!(resolved, ResolvedType::Unknown) {
            return false;
        }
        let DartType::Named(named) = ty else {
            return false;
        };
        let Some(qualified) = QualifiedName::from_identifiers(named.segments()) else {
            return false;
        };
        if qualified.name != name || unresolved_is_shadowed(model, qualified) {
            return false;
        }
        self.program.imports().any(|import| {
            let uri = import.uri().value();
            (uri.starts_with("package:bloc/") || uri.starts_with("package:flutter_bloc/"))
                && import_exposes(import, qualified)
        })
    }

    pub(crate) fn binding_has_api_syntax(
        &self,
        expression: Expr<'a>,
        state: &SemanticState<'a>,
        names: &[&str],
    ) -> bool {
        let mut expression = expression.unparenthesized();
        while let Expr::NullAssert(assertion) = expression {
            expression = assertion.operand().unparenthesized();
        }
        let matches_type = |ty| {
            let resolved = state.model.resolve_type(ty);
            names
                .iter()
                .any(|name| self.is_resolved_api_syntax(ty, &resolved, &state.model, name))
        };
        let (name, declaration) = match expression {
            Expr::Ident(identifier) => (
                identifier.name(),
                state.environment.binding_declaration(identifier.name()),
            ),
            Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
                (field.field().name(), None)
            }
            _ => return false,
        };
        if let Some(declaration) = declaration {
            for node in expression.syntax().ancestors() {
                for parameters in node.children().filter_map(FormalParamList::cast) {
                    if let Some(parameter) = parameters
                        .params()
                        .find(|parameter| parameter.name().span() == declaration)
                    {
                        return parameter.param_type().is_some_and(matches_type);
                    }
                }
                for variable in node.children().filter_map(LocalVarDecl::cast) {
                    if variable
                        .declarators()
                        .any(|variable| variable.name().span() == declaration)
                    {
                        return variable.var_type().is_some_and(matches_type);
                    }
                }
            }
            return false;
        }
        expression
            .syntax()
            .ancestors()
            .find_map(ClassDecl::cast)
            .is_some_and(|class| {
                class.members().any(|member| {
                    let ClassMember::Field(field) = member else {
                        return false;
                    };
                    field.declarators().any(|field| field.name().name() == name)
                        && field.field_type().is_some_and(matches_type)
                })
            })
    }

    pub(crate) fn is_bloc_class(
        &self,
        class: ClassDecl<'_>,
        model: &SemanticModel<'_>,
        signatures: Option<&SignatureIndex>,
    ) -> bool {
        class.extends().is_some_and(|supertype| {
            self.is_api_syntax(supertype, model, "Bloc")
                || self.is_api_syntax(supertype, model, "Cubit")
        }) || model
            .resolve_name(class.name().name())
            .is_some_and(|identity| {
                self.is_bloc_type(
                    &ResolvedType::interface_identity(identity),
                    model,
                    signatures,
                )
            })
    }

    pub(crate) fn is_event_bloc_class(
        &self,
        class: ClassDecl<'_>,
        model: &SemanticModel<'_>,
        signatures: Option<&SignatureIndex>,
    ) -> bool {
        class
            .extends()
            .is_some_and(|supertype| self.is_api_syntax(supertype, model, "Bloc"))
            || model
                .resolve_name(class.name().name())
                .is_some_and(|identity| {
                    self.is_event_bloc_type(
                        &ResolvedType::interface_identity(identity),
                        model,
                        signatures,
                    )
                })
    }

    pub(crate) fn is_bloc_type(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
        signatures: Option<&SignatureIndex>,
    ) -> bool {
        self.is_bloc_type_at(ty, model, signatures, 0, true)
    }

    pub(crate) fn is_event_bloc_type(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
        signatures: Option<&SignatureIndex>,
    ) -> bool {
        self.is_bloc_type_at(ty, model, signatures, 0, false)
    }

    fn is_bloc_type_at(
        &self,
        ty: &ResolvedType,
        model: &SemanticModel<'_>,
        signatures: Option<&SignatureIndex>,
        depth: usize,
        allow_cubit: bool,
    ) -> bool {
        if self.is_api_type(ty, model, "Bloc")
            || (allow_cubit && self.is_api_type(ty, model, "Cubit"))
        {
            return true;
        }
        let ResolvedType::Interface { identity, .. } = ty else {
            return false;
        };
        if depth > 64 {
            return false;
        }
        if signatures.is_some_and(|signatures| {
            signatures.supertype_matches(identity, &|parent| {
                is_api_identity(parent, model, "Bloc")
                    || (allow_cubit && is_api_identity(parent, model, "Cubit"))
            }) == Truth::Yes
        }) {
            return true;
        }
        self.program.declarations().any(|declaration| {
            let TopLevelDecl::Class(class) = declaration else {
                return false;
            };
            if model.resolve_name(class.name().name()).as_ref() != Some(identity) {
                return false;
            }
            class.extends().is_some_and(|supertype| {
                self.is_api_syntax(supertype, model, "Bloc")
                    || (allow_cubit && self.is_api_syntax(supertype, model, "Cubit"))
                    || self.is_bloc_type_at(
                        &model.resolve_type(supertype),
                        model,
                        signatures,
                        depth + 1,
                        allow_cubit,
                    )
            })
        })
    }

    // An unresolved mixin may declare any member, so it conservatively blocks API recognition.
    pub(crate) fn has_local_member(
        &self,
        class: ClassDecl<'_>,
        model: &SemanticModel<'_>,
        name: &str,
        include_class: bool,
    ) -> bool {
        if include_class {
            self.local_declaration_has_member(TopLevelDecl::Class(class), model, name, 0)
        } else {
            let inherits = |ty| self.local_type_has_member(ty, model, name, 0, false);
            let mixes = |ty| self.local_type_has_member(ty, model, name, 0, true);
            class.extends().is_some_and(inherits) || class.with_clause().any(mixes)
        }
    }

    fn local_declaration_has_member(
        &self,
        declaration: TopLevelDecl<'_>,
        model: &SemanticModel<'_>,
        name: &str,
        depth: usize,
    ) -> bool {
        if depth > 64 {
            return true;
        }
        let members = match declaration {
            TopLevelDecl::Class(class) => Some(class.members()),
            TopLevelDecl::MixinClass(class) => Some(class.members()),
            TopLevelDecl::Mixin(mixin) => Some(mixin.members()),
            _ => None,
        };
        if members.is_some_and(|mut members| {
            members.any(|member| match member {
                ClassMember::Method(method) => method.name().name() == name,
                ClassMember::Getter(getter) => getter.name().name() == name,
                ClassMember::Setter(setter) => setter.name().name() == name,
                ClassMember::Field(field) => {
                    field.declarators().any(|field| field.name().name() == name)
                }
                _ => false,
            })
        }) {
            return true;
        }
        let inherits = |ty| self.local_type_has_member(ty, model, name, depth + 1, false);
        let mixes = |ty| self.local_type_has_member(ty, model, name, depth + 1, true);
        match declaration {
            TopLevelDecl::Class(class) => {
                class.extends().is_some_and(inherits) || class.with_clause().any(mixes)
            }
            TopLevelDecl::MixinClass(class) => {
                class.extends().is_some_and(inherits) || class.with_clause().any(mixes)
            }
            TopLevelDecl::ClassTypeAlias(alias) => {
                inherits(alias.superclass()) || alias.with_clause().any(mixes)
            }
            _ => false,
        }
    }

    fn local_type_has_member(
        &self,
        ty: DartType<'_>,
        model: &SemanticModel<'_>,
        name: &str,
        depth: usize,
        unresolved_is_member: bool,
    ) -> bool {
        let DartType::Named(named) = ty else {
            return false;
        };
        let Some(qualified) = QualifiedName::from_identifiers(named.segments()) else {
            return false;
        };
        let identity = model.resolve_name(qualified);
        self.program
            .declarations()
            .find_map(|declaration| {
                let candidate = match declaration {
                    TopLevelDecl::Class(class) => class.name(),
                    TopLevelDecl::MixinClass(class) => class.name(),
                    TopLevelDecl::Mixin(mixin) => mixin.name(),
                    TopLevelDecl::ClassTypeAlias(alias) => alias.name(),
                    _ => return None,
                };
                let matches = match &identity {
                    Some(identity) => {
                        model.resolve_name(candidate.name()).as_ref() == Some(identity)
                    }
                    None => qualified.prefix.is_none() && candidate.name() == qualified.name,
                };
                matches.then(|| self.local_declaration_has_member(declaration, model, name, depth))
            })
            .unwrap_or_else(|| {
                unresolved_is_member && matches!(model.resolve_type(ty), ResolvedType::Unknown)
            })
    }
}

pub(crate) fn is_api_identity(
    identity: &DeclarationIdentity,
    model: &SemanticModel<'_>,
    name: &str,
) -> bool {
    identity.name() == name
        && (identity.is_package("bloc", name)
            || identity.is_package("flutter_bloc", name)
            || model.declaration_uri(identity).is_some_and(|uri| {
                uri.starts_with("package:bloc/") || uri.starts_with("package:flutter_bloc/")
            }))
}

fn unresolved_is_shadowed(model: &SemanticModel<'_>, name: QualifiedName<'_>) -> bool {
    let resolved = match name.prefix {
        Some(prefix) => model.resolve_doc_prefixed(prefix, name.name),
        None => model.resolve_library_name(name.name),
    };
    matches!(resolved, DocName::Declaration { .. } | DocName::Extension)
}

fn import_exposes(import: ImportDirective<'_>, name: QualifiedName<'_>) -> bool {
    import.as_name().map(|prefix| prefix.name()) == name.prefix
        && import.combinators().all(|combinator| match combinator {
            ImportCombinator::Show(show) => show.names().any(|entry| entry.name() == name.name),
            ImportCombinator::Hide(hide) => hide.names().all(|entry| entry.name() != name.name),
        })
}

fn immutable_annotation(
    annotation: Annotation<'_>,
    types: &BlocTypes<'_>,
    model: &SemanticModel<'_>,
) -> bool {
    let Some(name) = QualifiedName::from_identifiers(annotation.name()) else {
        return false;
    };
    if !matches!(name.name, "immutable" | "Immutable") {
        return false;
    }
    if let Some(identity) = model
        .resolve_value(name)
        .or_else(|| model.resolve_imported_member(name))
        .or_else(|| model.resolve_name(name))
    {
        return identity.is_meta_immutable()
            || model
                .declaration_uri(&identity)
                .is_some_and(|uri| uri.starts_with("package:meta/"));
    }
    if unresolved_is_shadowed(model, name) {
        return false;
    }
    types.program.imports().any(|import| {
        let uri = import.uri().value();
        (uri.starts_with("package:meta/")
            || (name.name == "immutable" && uri.starts_with("package:flutter/")))
            && import_exposes(import, name)
    })
}

#[derive(Clone, Copy)]
pub(crate) enum ModelKind {
    Event,
    State,
}
#[derive(Clone, Copy)]
pub(crate) enum ModelRequirement {
    Sealed,
    Immutable,
    Suffix,
}

pub(crate) fn prepare_models<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    kind: ModelKind,
    requirement: ModelRequirement,
) -> RulePreparation<'a> {
    let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
        return RulePreparation::Diagnostics(Vec::new());
    };
    RulePreparation::Structural(Box::new(ModelCollector {
        ctx,
        model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
        signatures,
        types: BlocTypes::new(program),
        kind,
        requirement,
        roots: Vec::new(),
        classes: Vec::new(),
    }))
}

struct ModelCollector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    types: BlocTypes<'a>,
    kind: ModelKind,
    requirement: ModelRequirement,
    roots: Vec<DeclarationIdentity>,
    classes: Vec<ClassDecl<'a>>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for ModelCollector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let TopLevelDecl::Class(class) = declaration else {
            return StructuralTraversal::SkipChildren;
        };
        self.classes.push(class);
        if matches!(self.requirement, ModelRequirement::Suffix) {
            if let Some(root) = class.extends().and_then(|ty| {
                self.suffix_root(self.model.resolve_type(ty))
            }) && !self.roots.contains(&root) {
                self.roots.push(root);
            }
            return StructuralTraversal::SkipChildren;
        }
        if let Some(supertype) = class.extends() {
            let resolved = self.model.resolve_type(supertype);
            let is_bloc = self.types.is_resolved_api_syntax(
                supertype, &resolved, &self.model, "Bloc",
            );
            let is_cubit = matches!(self.kind, ModelKind::State)
                && !is_bloc
                && self.types.is_resolved_api_syntax(
                    supertype, &resolved, &self.model, "Cubit",
                );
            let index = match (self.kind, is_bloc, is_cubit) {
                (ModelKind::Event, true, _) => Some(0),
                (ModelKind::State, true, _) => Some(1),
                (ModelKind::State, _, true) => Some(0),
                _ => None,
            };
            let Some(index) = index else {
                return StructuralTraversal::SkipChildren;
            };
            let root = if self.types.is_api_type(&resolved, &self.model, "Bloc")
                || self.types.is_api_type(&resolved, &self.model, "Cubit")
            {
                match resolved.arguments().get(index) {
                    Some(ResolvedType::Interface { identity, .. }) => Some(identity.clone()),
                    _ => None,
                }
            } else if let DartType::Named(named) = supertype {
                named.type_args().nth(index).and_then(|argument| {
                    match self.model.resolve_type(argument) {
                        ResolvedType::Interface { identity, .. } => Some(identity),
                        _ => None,
                    }
                })
            } else {
                None
            };
            if let Some(identity) = root
                && self.model.identities().is_project(&identity)
                && !self.roots.contains(&identity)
            {
                self.roots.push(identity);
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        if self.roots.is_empty() {
            return Vec::new();
        }
        let mut diagnostics = Vec::new();
        for class in &self.classes {
            let Some(identity) = self.model.resolve_name(class.name().name()) else {
                continue;
            };
            let ty = ResolvedType::interface_identity(identity);
            if !self.roots.iter().any(|root| {
                self.signatures.is_subtype_of(&ty, root, &self.model) == Truth::Yes
            }) {
                continue;
            }
            let satisfied = match self.requirement {
                ModelRequirement::Sealed => matches!(class.modifiers().restriction,
                    Some(ClassRestriction::Sealed | ClassRestriction::Final)),
                ModelRequirement::Immutable => class.annotations().any(|annotation| {
                    immutable_annotation(annotation, &self.types, &self.model)
                }),
                ModelRequirement::Suffix => class.name().name().ends_with(match self.kind {
                    ModelKind::Event => "Event",
                    ModelKind::State => "State",
                }),
            };
            if satisfied {
                continue;
            }
            let message = match (self.kind, self.requirement) {
                (ModelKind::Event, ModelRequirement::Sealed) => "Declare this Bloc event `sealed` or `final`.",
                (ModelKind::State, ModelRequirement::Sealed) => "Declare this Bloc state `sealed` or `final`.",
                (ModelKind::Event, ModelRequirement::Immutable) => "Annotate this Bloc event with `@immutable`.",
                (ModelKind::State, ModelRequirement::Immutable) => "Annotate this Bloc state with `@immutable`.",
                (ModelKind::Event, ModelRequirement::Suffix) => "End Bloc event class names with `Event`.",
                (ModelKind::State, ModelRequirement::Suffix) => "End Bloc state class names with `State`.",
            };
            diagnostics.push(self.ctx.diagnostic(message, class.name().span()));
        }
        diagnostics
    }
}
}

impl ModelCollector<'_> {
    // Follow only the superclass chain: implementing Bloc does not establish a
    // Bloc's event/state contract. Substitute each generic intermediate owner.
    fn suffix_root(&self, mut ty: ResolvedType) -> Option<DeclarationIdentity> {
        let mut substitutions = std::collections::HashMap::new();
        for _ in 0..64 {
            let ResolvedType::Interface {
                identity,
                arguments,
                ..
            } = &ty
            else {
                return None;
            };
            let index = if self.types.is_api_type(&ty, &self.model, "Bloc") {
                match self.kind {
                    ModelKind::Event => 0,
                    ModelKind::State => 1,
                }
            } else if matches!(self.kind, ModelKind::State)
                && self.types.is_api_type(&ty, &self.model, "Cubit")
            {
                0
            } else {
                let superclass = self.signatures.superclass(identity)?;
                let parameters = self.signatures.type_parameters(identity)?;
                if parameters.len() != arguments.len() {
                    return None;
                }
                substitutions.clear();
                substitutions.extend(parameters.iter().cloned().zip(arguments.iter().cloned()));
                ty = self.model.substitute(superclass, &substitutions);
                continue;
            };
            return match arguments.get(index) {
                Some(ResolvedType::Interface { identity, .. })
                    if !matches!(identity, DeclarationIdentity::Sdk { .. }) =>
                {
                    Some(identity.clone())
                }
                _ => None,
            };
        }
        None
    }
}
