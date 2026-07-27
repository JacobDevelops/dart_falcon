use std::borrow::Cow;
use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ResolvedType, SemanticModel, SignatureIndex, TypeEnvironment, TypeParameterId,
    TypeParameterScope,
};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_pattern, walk_top_level_decl};

pub(crate) trait SemanticRuleVisitor {
    fn visit_expr(&mut self, _node: &Expr, _state: &SemanticState<'_>) {}

    fn visit_pattern(
        &mut self,
        _node: &Pattern,
        _matched: Option<&ResolvedType>,
        _state: &SemanticState<'_>,
    ) {
    }
}

pub(crate) struct SemanticState<'a> {
    pub(crate) model: SemanticModel<'a>,
    /// Borrowed from the driver's project-wide index; owned only for the
    /// single-file fallback built when the driver has none.
    pub(crate) signatures: Cow<'a, SignatureIndex>,
    pub(crate) environment: TypeEnvironment,
    pub(crate) type_parameters: TypeParameterScope,
    expected: Vec<ResolvedType>,
    return_type: Vec<ResolvedType>,
    matched: Vec<ResolvedType>,
    enclosing_type: Option<ResolvedType>,
    superclass: Option<ResolvedType>,
}

impl<'a> SemanticState<'a> {
    pub(crate) fn new(program: &Program, ctx: &'a AnalyzeContext<'a>) -> Option<Self> {
        let identities = ctx.identities?;
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let signatures = ctx
            .signatures
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(SignatureIndex::from_program(program, &model)));
        Some(Self {
            model,
            signatures,
            environment: TypeEnvironment::new(),
            type_parameters: TypeParameterScope::default(),
            expected: Vec::new(),
            return_type: Vec::new(),
            matched: Vec::new(),
            enclosing_type: None,
            superclass: None,
        })
    }

    pub(crate) fn infer(&self, expression: &Expr) -> ResolvedType {
        self.environment.infer_with_signatures(
            expression,
            &self.model,
            &self.signatures,
            &self.type_parameters,
        )
    }

    pub(crate) fn expected(&self) -> Option<&ResolvedType> {
        self.expected.last()
    }
}

pub(crate) fn visit_program<V: SemanticRuleVisitor>(
    visitor: &mut V,
    program: &Program,
    state: SemanticState<'_>,
) {
    let mut walker = ScopedWalker { visitor, state };
    walker.visit_program(program);
}

pub(crate) fn resolved_super_param_type(
    param: &FormalParam,
    superclass: &ResolvedType,
    constructor_name: &str,
    signatures: &SignatureIndex,
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
    let facts = signatures
        .declaration(identity)?
        .constructors
        .iter()
        .find(|constructor| constructor.name == constructor_name)?;
    let parameter = facts
        .parameters
        .iter()
        .find(|parameter| parameter.name == param.name.name)?;
    let signature = signatures.constructor(identity, constructor_name)?;
    let ty = if parameter.is_named {
        signature.named.get(&param.name.name)?
    } else {
        let index = facts
            .parameters
            .iter()
            .filter(|parameter| !parameter.is_named)
            .position(|parameter| parameter.name == param.name.name)?;
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

pub(crate) fn resolved_param_type(
    param: &FormalParam,
    model: &SemanticModel<'_>,
    type_parameters: &TypeParameterScope,
) -> ResolvedType {
    if !param.type_params.is_empty() {
        return ResolvedType::Unknown;
    }
    let return_type = param
        .param_type
        .as_ref()
        .map(|ty| model.resolve_type_in(ty, type_parameters))
        .unwrap_or(ResolvedType::Dynamic);
    let Some(function_params) = &param.function_params else {
        return return_type;
    };
    ResolvedType::Function {
        return_type: Box::new(return_type),
        positional: function_params
            .positional
            .iter()
            .chain(&function_params.optional_positional)
            .map(|param| resolved_param_type(param, model, type_parameters))
            .collect(),
        named: function_params
            .named
            .iter()
            .map(|param| {
                (
                    param.name.name.clone(),
                    resolved_param_type(param, model, type_parameters),
                )
            })
            .collect(),
        nullable: false,
    }
}

struct ScopedWalker<'a, 'model, V> {
    visitor: &'a mut V,
    state: SemanticState<'model>,
}

impl<V: SemanticRuleVisitor> ScopedWalker<'_, '_, V> {
    fn with_fresh_environment(&mut self, f: impl FnOnce(&mut Self)) {
        let saved = std::mem::replace(&mut self.state.environment, TypeEnvironment::new());
        f(self);
        self.state.environment = saved;
    }

    fn with_scope(&mut self, f: impl FnOnce(&mut Self)) {
        self.state.environment.push_scope();
        f(self);
        self.state.environment.pop_scope();
    }

    fn with_expected(&mut self, expected: ResolvedType, f: impl FnOnce(&mut Self)) {
        self.state.expected.push(expected);
        f(self);
        self.state.expected.pop();
    }

    fn visit_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(block) => {
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
            }
            FunctionBody::Arrow(expression, _) => {
                let expected = self
                    .state
                    .return_type
                    .last()
                    .cloned()
                    .unwrap_or(ResolvedType::Unknown);
                self.with_expected(expected, |walker| walker.visit_expr(expression));
            }
            FunctionBody::Native(_, _) => {}
        }
    }

    fn visit_function(
        &mut self,
        type_parameters: &[TypeParam],
        params: &FormalParamList,
        return_type: Option<&DartType>,
        body: Option<&FunctionBody>,
        contextual: bool,
    ) {
        self.state
            .type_parameters
            .push(type_parameters, &self.state.model);
        let saved_expected = (!contextual).then(|| std::mem::take(&mut self.state.expected));
        let saved_returns = (!contextual).then(|| std::mem::take(&mut self.state.return_type));
        let resolved_return = return_type
            .map(|ty| {
                self.state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters)
            })
            .or_else(|| {
                contextual
                    .then(|| self.contextual_closure_return(type_parameters))
                    .flatten()
            })
            .unwrap_or(ResolvedType::Dynamic);
        self.state.return_type.push(resolved_return);
        self.with_scope(|walker| {
            walker.bind_params(params, None);
            walker.visit_param_defaults(params);
            if let Some(body) = body {
                walker.visit_body(body);
            }
        });
        self.state.return_type.pop();
        if let Some(saved) = saved_returns {
            self.state.return_type = saved;
        }
        if let Some(saved) = saved_expected {
            self.state.expected = saved;
        }
        self.state.type_parameters.pop();
    }

    fn contextual_closure_return(&self, type_parameters: &[TypeParam]) -> Option<ResolvedType> {
        let ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } = self.state.expected()?
        else {
            return None;
        };
        if type_parameters.is_empty() {
            return Some(return_type.as_ref().clone());
        }
        let mut by_owner: HashMap<usize, Vec<TypeParameterId>> = HashMap::new();
        collect_type_parameter_ids(return_type, &mut by_owner);
        for parameter in positional {
            collect_type_parameter_ids(parameter, &mut by_owner);
        }
        for (_, parameter) in named {
            collect_type_parameter_ids(parameter, &mut by_owner);
        }
        let contextual = by_owner.values().find(|parameters| {
            type_parameters
                .iter()
                .enumerate()
                .all(|(ordinal, declared)| {
                    parameters.iter().any(|parameter| {
                        parameter.ordinal == ordinal && parameter.name == declared.name.name
                    })
                })
        })?;
        let substitutions: HashMap<_, _> = contextual
            .iter()
            .filter_map(|parameter| {
                type_parameters
                    .get(parameter.ordinal)
                    .and_then(|declared| self.state.type_parameters.lookup(&declared.name.name))
                    .map(|resolved| (parameter.clone(), resolved))
            })
            .collect();
        Some(self.state.model.substitute(return_type, &substitutions))
    }

    fn bind_params(&mut self, params: &FormalParamList, super_constructor: Option<&str>) {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            let mut ty = resolved_param_type(param, &self.state.model, &self.state.type_parameters);
            if param.param_type.is_none() && param.function_params.is_none() {
                if param.is_super {
                    ty = super_constructor
                        .and_then(|constructor| {
                            self.state.superclass.as_ref().and_then(|superclass| {
                                resolved_super_param_type(
                                    param,
                                    superclass,
                                    constructor,
                                    &self.state.signatures,
                                    &self.state.model,
                                )
                            })
                        })
                        .unwrap_or(ResolvedType::Unknown);
                } else if param.is_field {
                    ty = self.state.environment.lookup(&param.name.name);
                    if matches!(ty, ResolvedType::Unknown) {
                        ty = self
                            .state
                            .enclosing_type
                            .as_ref()
                            .and_then(|enclosing| {
                                self.state.signatures.resolved_field(
                                    enclosing,
                                    &param.name.name,
                                    &self.state.model,
                                )
                            })
                            .map(|(ty, substitutions)| {
                                self.state.model.substitute(&ty, &substitutions)
                            })
                            .unwrap_or(ResolvedType::Unknown);
                    }
                }
            }
            self.state.environment.declare(param.name.name.clone(), ty);
        }
    }

    fn visit_param_defaults(&mut self, params: &FormalParamList) {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            if let Some(value) = &param.default_value {
                let expected = self.state.environment.lookup(&param.name.name);
                self.with_expected(expected, |walker| walker.visit_expr(value));
            }
        }
    }

    fn declare_fields(&mut self, members: &[ClassMember]) {
        for member in members {
            let ClassMember::Field(field) = member else {
                continue;
            };
            for declarator in &field.declarators {
                let ty = field
                    .field_type
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .or_else(|| {
                        declarator
                            .initializer
                            .as_ref()
                            .map(|initializer| self.state.infer(initializer))
                    })
                    .unwrap_or(ResolvedType::Unknown);
                self.state
                    .environment
                    .declare(declarator.name.name.clone(), ty);
            }
        }
    }

    fn visit_local_declaration(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            let declared = declaration.var_type.as_ref().map(|ty| {
                self.state
                    .model
                    .resolve_type_in(ty, &self.state.type_parameters)
            });
            if let Some(initializer) = &declarator.initializer {
                self.with_expected(
                    declared.clone().unwrap_or(ResolvedType::Unknown),
                    |walker| walker.visit_expr(initializer),
                );
            }
            let ty = declared
                .or_else(|| {
                    declarator
                        .initializer
                        .as_ref()
                        .map(|initializer| self.state.infer(initializer))
                })
                .unwrap_or(ResolvedType::Unknown);
            self.state
                .environment
                .declare(declarator.name.name.clone(), ty);
        }
    }

    fn iterable_element_type(&self, iterable: &Expr) -> ResolvedType {
        let iterable = self.state.infer(iterable);
        self.state
            .signatures
            .instantiated_supertype(&iterable, "dart:core", "Iterable", &self.state.model)
            .and_then(|ty| ty.arguments().first().cloned())
            .unwrap_or(ResolvedType::Unknown)
    }

    fn visit_matched_pattern(&mut self, pattern: &Pattern, matched: ResolvedType) {
        self.state.matched.push(matched.clone());
        self.visitor
            .visit_pattern(pattern, Some(&matched), &self.state);
        match pattern {
            Pattern::Variable { type_, name, .. } => {
                let ty = type_
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .unwrap_or(matched);
                self.state.environment.declare(name.name.clone(), ty);
            }
            Pattern::Wildcard { .. } | Pattern::Literal(_) | Pattern::Error { .. } => {}
            Pattern::Const(constant) => {
                if let Some(expression) = &constant.expr {
                    self.visit_expr(expression);
                }
            }
            Pattern::List(list) => {
                let element = list
                    .type_arg
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .or_else(|| {
                        self.state
                            .signatures
                            .instantiated_supertype(
                                &matched,
                                "dart:core",
                                "List",
                                &self.state.model,
                            )
                            .and_then(|ty| ty.arguments().first().cloned())
                    })
                    .unwrap_or(ResolvedType::Unknown);
                for child in &list.elements {
                    match child {
                        ListPatternElement::Pattern(child) => {
                            self.visit_matched_pattern(child, element.clone());
                        }
                        ListPatternElement::Rest(Some(child), _) => {
                            self.visit_matched_pattern(child, matched.clone());
                        }
                        ListPatternElement::Rest(None, _) => {}
                    }
                }
            }
            Pattern::Record(record) => {
                let (positional, named) = match &matched {
                    ResolvedType::Record {
                        positional, named, ..
                    } => (positional.as_slice(), named.as_slice()),
                    _ => (&[][..], &[][..]),
                };
                let mut positional_index = 0;
                for field in &record.fields {
                    let field_type = field
                        .name
                        .as_ref()
                        .and_then(|name| {
                            named
                                .iter()
                                .find(|(field, _)| field == &name.name)
                                .map(|(_, ty)| ty.clone())
                        })
                        .unwrap_or_else(|| {
                            let ty = positional
                                .get(positional_index)
                                .cloned()
                                .unwrap_or(ResolvedType::Unknown);
                            positional_index += 1;
                            ty
                        });
                    self.visit_matched_pattern(&field.pattern, field_type);
                }
            }
            Pattern::Map(map) => {
                let value_type = self
                    .state
                    .signatures
                    .instantiated_supertype(&matched, "dart:core", "Map", &self.state.model)
                    .and_then(|ty| ty.arguments().get(1).cloned())
                    .unwrap_or(ResolvedType::Unknown);
                for entry in &map.entries {
                    self.visit_expr(&entry.key);
                    self.visit_matched_pattern(&entry.pattern, value_type.clone());
                }
            }
            Pattern::Object(object) => {
                let object_type = self
                    .state
                    .model
                    .resolve_type_in(&object.type_, &self.state.type_parameters);
                for field in &object.fields {
                    let field_type = self
                        .state
                        .signatures
                        .resolved_field(&object_type, &field.name.name, &self.state.model)
                        .map(|(ty, substitutions)| self.state.model.substitute(&ty, &substitutions))
                        .unwrap_or(ResolvedType::Unknown);
                    if let Some(child) = &field.pattern {
                        self.visit_matched_pattern(child, field_type);
                    } else {
                        self.state
                            .environment
                            .declare(field.name.name.clone(), field_type);
                    }
                }
            }
            Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
                self.visit_matched_pattern(left, matched.clone());
                self.visit_matched_pattern(right, matched.clone());
            }
            Pattern::Relational { value, .. } => self.visit_expr(value),
            Pattern::Cast {
                inner, cast_type, ..
            } => {
                let cast = self
                    .state
                    .model
                    .resolve_type_in(cast_type, &self.state.type_parameters);
                self.visit_matched_pattern(inner, cast);
            }
            Pattern::NullCheck { inner, .. } | Pattern::NullAssert { inner, .. } => {
                self.visit_matched_pattern(inner, matched.with_nullable(false));
            }
            Pattern::ParenPattern { inner, .. } => {
                self.visit_matched_pattern(inner, matched);
            }
        }
        self.state.matched.pop();
    }

    fn visit_for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_declaration(declaration),
            ForInit::ForIn {
                var_type,
                name,
                iterable,
                ..
            } => {
                self.visit_expr(iterable);
                let ty = var_type
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .unwrap_or_else(|| self.iterable_element_type(iterable));
                self.state.environment.declare(name.name.clone(), ty);
            }
            ForInit::PatternForIn { pattern, iterable } => {
                self.visit_expr(iterable);
                let matched = self.iterable_element_type(iterable);
                self.visit_matched_pattern(pattern, matched);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }

    fn visit_if_condition(
        &mut self,
        condition: &IfCondition,
        then_branch: impl FnOnce(&mut Self),
        else_branch: impl FnOnce(&mut Self),
    ) {
        match condition {
            IfCondition::Expr(expression) => {
                self.visit_expr(expression);
                self.with_scope(then_branch);
                self.with_scope(else_branch);
            }
            IfCondition::Case(value, pattern, guard) => {
                self.visit_expr(value);
                let matched = self.state.infer(value);
                self.with_scope(|walker| {
                    walker.visit_matched_pattern(pattern, matched);
                    if let Some(guard) = guard {
                        walker.visit_expr(guard);
                    }
                    then_branch(walker);
                });
                self.with_scope(else_branch);
            }
        }
    }

    fn visit_collection_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::Expr(expression)
            | CollectionElement::NullAware {
                expr: expression, ..
            }
            | CollectionElement::Spread {
                expr: expression, ..
            } => self.visit_expr(expression),
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => self.visit_if_condition(
                condition,
                |walker| walker.visit_collection_element(then_elem),
                |walker| {
                    if let Some(element) = else_elem {
                        walker.visit_collection_element(element);
                    }
                },
            ),
            CollectionElement::For {
                variable,
                var_type,
                pattern,
                iterable,
                element,
                ..
            } => {
                self.visit_expr(iterable);
                self.with_scope(|walker| {
                    if let Some(variable) = variable {
                        let ty = var_type
                            .as_ref()
                            .map(|ty| {
                                walker
                                    .state
                                    .model
                                    .resolve_type_in(ty, &walker.state.type_parameters)
                            })
                            .unwrap_or_else(|| walker.iterable_element_type(iterable));
                        walker.state.environment.declare(variable.name.clone(), ty);
                    }
                    if let Some(pattern) = pattern {
                        let matched = walker.iterable_element_type(iterable);
                        walker.visit_matched_pattern(pattern, matched);
                    }
                    walker.visit_collection_element(element);
                });
            }
            CollectionElement::CFor {
                init,
                condition,
                updates,
                element,
                ..
            } => self.with_scope(|walker| {
                if let Some(init) = init {
                    walker.visit_for_init(init);
                }
                if let Some(condition) = condition {
                    walker.visit_expr(condition);
                }
                for update in updates {
                    walker.visit_expr(update);
                }
                walker.visit_collection_element(element);
            }),
        }
    }

    fn visit_map_element(&mut self, element: &MapElement) {
        match element {
            MapElement::Entry(entry) => {
                self.visit_expr(&entry.key);
                self.visit_expr(&entry.value);
            }
            MapElement::Spread { expr, .. } => self.visit_expr(expr),
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => self.visit_if_condition(
                condition,
                |walker| walker.visit_map_element(then_entry),
                |walker| {
                    if let Some(entry) = else_entry {
                        walker.visit_map_element(entry);
                    }
                },
            ),
            MapElement::For {
                variable,
                var_type,
                pattern,
                iterable,
                entry,
                ..
            } => {
                self.visit_expr(iterable);
                self.with_scope(|walker| {
                    if let Some(variable) = variable {
                        let ty = var_type
                            .as_ref()
                            .map(|ty| {
                                walker
                                    .state
                                    .model
                                    .resolve_type_in(ty, &walker.state.type_parameters)
                            })
                            .unwrap_or_else(|| walker.iterable_element_type(iterable));
                        walker.state.environment.declare(variable.name.clone(), ty);
                    }
                    if let Some(pattern) = pattern {
                        let matched = walker.iterable_element_type(iterable);
                        walker.visit_matched_pattern(pattern, matched);
                    }
                    walker.visit_map_element(entry);
                });
            }
            MapElement::CFor {
                init,
                condition,
                updates,
                entry,
                ..
            } => self.with_scope(|walker| {
                if let Some(init) = init {
                    walker.visit_for_init(init);
                }
                if let Some(condition) = condition {
                    walker.visit_expr(condition);
                }
                for update in updates {
                    walker.visit_expr(update);
                }
                walker.visit_map_element(entry);
            }),
        }
    }

    fn visit_call_arguments(
        &mut self,
        args: &ArgList,
        positional: &[ResolvedType],
        named: &HashMap<String, ResolvedType>,
    ) {
        for (argument, expected) in args.positional.iter().zip(positional) {
            self.with_expected(expected.clone(), |walker| walker.visit_expr(argument));
        }
        for argument in args.positional.iter().skip(positional.len()) {
            self.visit_expr(argument);
        }
        for argument in &args.named {
            if let Some(expected) = named.get(&argument.name.name) {
                self.with_expected(expected.clone(), |walker| {
                    walker.visit_expr(&argument.value)
                });
            } else {
                self.visit_expr(&argument.value);
            }
        }
    }
}

impl<V: SemanticRuleVisitor> Visitor for ScopedWalker<'_, '_, V> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let enclosing_name = match node {
            TopLevelDecl::Class(declaration) => Some(&declaration.name.name),
            TopLevelDecl::Mixin(declaration) => Some(&declaration.name.name),
            TopLevelDecl::MixinClass(declaration) => Some(&declaration.name.name),
            TopLevelDecl::Enum(declaration) => Some(&declaration.name.name),
            TopLevelDecl::ExtensionType(declaration) => Some(&declaration.name.name),
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::Variable(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => None,
        };
        let superclass = match node {
            TopLevelDecl::Class(declaration) => declaration.extends.as_ref(),
            TopLevelDecl::MixinClass(declaration) => declaration.extends.as_ref(),
            _ => None,
        };
        let scoped = match node {
            TopLevelDecl::Class(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::Mixin(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::MixinClass(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::Enum(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::Extension(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            TopLevelDecl::ExtensionType(declaration) => Some((
                declaration.type_params.as_slice(),
                declaration.members.as_slice(),
            )),
            _ => None,
        };
        if let Some((type_parameters, members)) = scoped {
            self.with_fresh_environment(|walker| {
                walker
                    .state
                    .type_parameters
                    .push(type_parameters, &walker.state.model);
                // Instantiate the enclosing type with its own type parameters, so
                // member and constructor lookups have something to substitute.
                let arguments: Vec<ResolvedType> = type_parameters
                    .iter()
                    .map(|param| {
                        walker
                            .state
                            .type_parameters
                            .lookup(&param.name.name)
                            .unwrap_or(ResolvedType::Unknown)
                    })
                    .collect();
                let previous_enclosing = walker.state.enclosing_type.replace(
                    enclosing_name
                        .and_then(|name| {
                            walker.state.model.resolve_name(std::slice::from_ref(name))
                        })
                        .map(|identity| ResolvedType::Interface {
                            identity,
                            arguments,
                            nullable: false,
                            extension_type: false,
                        })
                        .unwrap_or(ResolvedType::Unknown),
                );
                let previous_superclass = std::mem::replace(
                    &mut walker.state.superclass,
                    superclass.map(|superclass| {
                        walker
                            .state
                            .model
                            .resolve_type_in(superclass, &walker.state.type_parameters)
                    }),
                );
                walker.declare_fields(members);
                walk_top_level_decl(walker, node);
                walker.state.superclass = previous_superclass;
                walker.state.type_parameters.pop();
                walker.state.enclosing_type = previous_enclosing;
            });
        } else if matches!(node, TopLevelDecl::Function(_)) {
            self.with_fresh_environment(|walker| walk_top_level_decl(walker, node));
        } else {
            walk_top_level_decl(self, node);
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.visit_function(
            &node.type_params,
            &node.params,
            node.return_type.as_ref(),
            node.body.as_ref(),
            false,
        );
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.visit_function(
            &node.type_params,
            &node.params,
            node.return_type.as_ref(),
            node.body.as_ref(),
            false,
        );
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.visit_function(
                &[],
                &operator.params,
                operator.return_type.as_ref(),
                operator.body.as_ref(),
                false,
            );
        } else {
            falcon_syntax::visitor::walk_class_member(self, node);
        }
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        for declarator in &node.declarators {
            if let Some(initializer) = &declarator.initializer {
                let expected = node
                    .field_type
                    .as_ref()
                    .map(|ty| {
                        self.state
                            .model
                            .resolve_type_in(ty, &self.state.type_parameters)
                    })
                    .unwrap_or(ResolvedType::Unknown);
                self.with_expected(expected, |walker| walker.visit_expr(initializer));
            }
        }
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let saved_expected = std::mem::take(&mut self.state.expected);
        let saved_returns = std::mem::take(&mut self.state.return_type);
        self.state.type_parameters.push(&[], &self.state.model);
        self.state.return_type.push(ResolvedType::Dynamic);
        let redirects_to_this = node
            .initializers
            .iter()
            .any(|initializer| matches!(initializer, ConstructorInitializer::ThisCall { .. }));
        let super_constructor = (!redirects_to_this).then(|| {
            node.initializers
                .iter()
                .find_map(|initializer| match initializer {
                    ConstructorInitializer::SuperCall { call_name, .. } => {
                        Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
                    }
                    _ => None,
                })
                .unwrap_or("new")
        });
        self.with_scope(|walker| {
            walker.bind_params(&node.params, super_constructor);
            walker.visit_param_defaults(&node.params);
            for initializer in &node.initializers {
                match initializer {
                    ConstructorInitializer::SuperCall { args, .. }
                    | ConstructorInitializer::ThisCall { args, .. } => {
                        walker.visit_call_arguments(args, &[], &HashMap::new());
                    }
                    ConstructorInitializer::FieldInit { field, value, .. } => {
                        let expected = walker
                            .state
                            .enclosing_type
                            .as_ref()
                            .and_then(|enclosing| {
                                walker.state.signatures.resolved_field(
                                    enclosing,
                                    &field.name,
                                    &walker.state.model,
                                )
                            })
                            .map(|(ty, substitutions)| {
                                walker.state.model.substitute(&ty, &substitutions)
                            })
                            .unwrap_or(ResolvedType::Unknown);
                        walker.with_expected(expected, |walker| walker.visit_expr(value));
                    }
                    ConstructorInitializer::Assert {
                        condition, message, ..
                    } => {
                        walker.visit_expr(condition);
                        if let Some(message) = message {
                            walker.visit_expr(message);
                        }
                    }
                }
            }
            if let Some(body) = &node.body {
                walker.visit_body(body);
            }
        });
        self.state.return_type = saved_returns;
        self.state.expected = saved_expected;
        self.state.type_parameters.pop();
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        let empty = FormalParamList {
            positional: Vec::new(),
            optional_positional: Vec::new(),
            named: Vec::new(),
            span: node.span.clone(),
        };
        self.visit_function(
            &[],
            &empty,
            node.return_type.as_ref(),
            node.body.as_ref(),
            false,
        );
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let params = FormalParamList {
            positional: vec![FormalParam {
                annotations: Vec::new(),
                is_required: true,
                is_covariant: false,
                is_final: false,
                is_field: false,
                is_super: false,
                param_type: node.param_type.clone(),
                name: node.param.clone(),
                default_value: None,
                type_params: Vec::new(),
                function_params: None,
                span: node.param.span.clone(),
            }],
            optional_positional: Vec::new(),
            named: Vec::new(),
            span: node.span.clone(),
        };
        self.visit_function(&[], &params, None, node.body.as_ref(), false);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => self.with_scope(|walker| {
                for statement in &block.stmts {
                    walker.visit_stmt(statement);
                }
            }),
            Stmt::LocalVar(declaration) => self.visit_local_declaration(declaration),
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                self.visit_matched_pattern(
                    &declaration.pattern,
                    self.state.infer(&declaration.init),
                );
            }
            Stmt::PatternAssign(assignment) => {
                self.visit_expr(&assignment.value);
                self.visit_matched_pattern(
                    &assignment.pattern,
                    self.state.infer(&assignment.value),
                );
            }
            Stmt::If(statement) => self.visit_if_condition(
                &statement.condition,
                |walker| walker.visit_stmt(&statement.then_branch),
                |walker| {
                    if let Some(branch) = &statement.else_branch {
                        walker.visit_stmt(branch);
                    }
                },
            ),
            Stmt::For(statement) => self.with_scope(|walker| {
                if let Some(init) = &statement.init {
                    walker.visit_for_init(init);
                }
                if let Some(condition) = &statement.condition {
                    walker.visit_expr(condition);
                }
                for update in &statement.update {
                    walker.visit_expr(update);
                }
                walker.visit_stmt(&statement.body);
            }),
            Stmt::While(statement) => {
                self.visit_expr(&statement.condition);
                self.with_scope(|walker| walker.visit_stmt(&statement.body));
            }
            Stmt::DoWhile(statement) => self.with_scope(|walker| {
                walker.visit_stmt(&statement.body);
                walker.visit_expr(&statement.condition);
            }),
            Stmt::Switch(statement) => {
                self.visit_expr(&statement.subject);
                let matched = self.state.infer(&statement.subject);
                for case in &statement.cases {
                    self.with_scope(|walker| {
                        for kind in &case.cases {
                            if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                                walker.visit_matched_pattern(pattern, matched.clone());
                                if let Some(guard) = guard.as_ref() {
                                    walker.visit_expr(guard);
                                }
                            }
                        }
                        for statement in &case.body {
                            walker.visit_stmt(statement);
                        }
                    });
                }
            }
            Stmt::TryCatch(statement) => {
                self.with_scope(|walker| {
                    for statement in &statement.body.stmts {
                        walker.visit_stmt(statement);
                    }
                });
                for catch in &statement.catches {
                    self.with_scope(|walker| {
                        if let Some(name) = &catch.exception_var {
                            let ty = catch
                                .exception_type
                                .as_ref()
                                .map(|ty| {
                                    walker
                                        .state
                                        .model
                                        .resolve_type_in(ty, &walker.state.type_parameters)
                                })
                                .unwrap_or(ResolvedType::Unknown);
                            walker.state.environment.declare(name.name.clone(), ty);
                        }
                        if let Some(name) = &catch.stack_trace_var {
                            walker
                                .state
                                .environment
                                .declare(name.name.clone(), ResolvedType::Unknown);
                        }
                        for statement in &catch.body.stmts {
                            walker.visit_stmt(statement);
                        }
                    });
                }
                if let Some(finally) = &statement.finally {
                    self.with_scope(|walker| {
                        for statement in &finally.stmts {
                            walker.visit_stmt(statement);
                        }
                    });
                }
            }
            Stmt::Return(statement) => {
                if let Some(value) = &statement.value {
                    let expected = self
                        .state
                        .return_type
                        .last()
                        .cloned()
                        .unwrap_or(ResolvedType::Unknown);
                    self.with_expected(expected, |walker| walker.visit_expr(value));
                }
            }
            Stmt::Throw(statement) => self.visit_expr(&statement.value),
            Stmt::Labeled(statement) => self.visit_stmt(&statement.stmt),
            Stmt::LocalFunc(function) => {
                self.state
                    .environment
                    .declare(function.name.name.clone(), ResolvedType::Unknown);
                self.visit_function(
                    &function.type_params,
                    &function.params,
                    function.return_type.as_ref(),
                    Some(&function.body),
                    false,
                );
            }
            Stmt::Assert(statement) => {
                self.visit_expr(&statement.condition);
                if let Some(message) = &statement.message {
                    self.visit_expr(message);
                }
            }
            Stmt::Yield(statement) => self.visit_expr(&statement.value),
            Stmt::Expr(statement) => self.visit_expr(&statement.expr),
            Stmt::Break(_) | Stmt::Continue(_) | Stmt::Error(_) => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        self.visitor.visit_expr(node, &self.state);
        match node {
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => {
                self.with_expected(ResolvedType::Unknown, |walker| walker.visit_expr(callee));
                let (target, explicit_type_args) = match callee.as_ref() {
                    Expr::GenericInstantiation {
                        target, type_args, ..
                    } => (target.as_ref(), type_args.as_slice()),
                    target => (target, type_args.as_slice()),
                };
                let resolved = match target {
                    Expr::Ident(identifier)
                        if !self.state.environment.is_bound(&identifier.name) =>
                    {
                        self.state
                            .enclosing_type
                            .as_ref()
                            .and_then(|receiver| {
                                self.state.signatures.resolved_member(
                                    receiver,
                                    &identifier.name,
                                    &self.state.model,
                                )
                            })
                            .or_else(|| {
                                self.state
                                    .model
                                    .resolve_value(std::slice::from_ref(&identifier.name))
                                    .and_then(|identity| {
                                        self.state
                                            .signatures
                                            .function(&identity)
                                            .cloned()
                                            .map(|signature| (signature, HashMap::new()))
                                    })
                            })
                            .or_else(|| {
                                self.state
                                    .model
                                    .resolve_name(std::slice::from_ref(&identifier.name))
                                    .and_then(|identity| {
                                        self.state
                                            .signatures
                                            .constructor(&identity, "new")
                                            .cloned()
                                            .map(|signature| (signature, HashMap::new()))
                                    })
                            })
                    }
                    Expr::Field { object, field, .. } => {
                        let receiver = self.state.infer(object);
                        self.state.signatures.resolved_member(
                            &receiver,
                            &field.name,
                            &self.state.model,
                        )
                    }
                    _ => None,
                };
                if let Some((signature, mut substitutions)) = resolved {
                    substitutions.extend(
                        signature
                            .call_parameters
                            .iter()
                            .zip(explicit_type_args)
                            .map(|(parameter, argument)| {
                                (
                                    parameter.clone(),
                                    self.state
                                        .model
                                        .resolve_type_in(argument, &self.state.type_parameters),
                                )
                            }),
                    );
                    let positional = signature
                        .positional
                        .iter()
                        .map(|ty| self.state.model.substitute(ty, &substitutions))
                        .collect::<Vec<_>>();
                    let named = signature
                        .named
                        .iter()
                        .map(|(name, ty)| {
                            (
                                name.clone(),
                                self.state.model.substitute(ty, &substitutions),
                            )
                        })
                        .collect();
                    self.visit_call_arguments(args, &positional, &named);
                } else {
                    for argument in &args.positional {
                        self.visit_expr(argument);
                    }
                    for argument in &args.named {
                        self.visit_expr(&argument.value);
                    }
                }
            }
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                let resolved = self
                    .state
                    .model
                    .resolve_type_in(dart_type, &self.state.type_parameters);
                let signature = match resolved {
                    ResolvedType::Interface {
                        identity,
                        arguments,
                        ..
                    } => self
                        .state
                        .signatures
                        .constructor(
                            &identity,
                            constructor_name
                                .as_ref()
                                .map_or("new", |name| name.name.as_str()),
                        )
                        .cloned()
                        .map(|signature| {
                            let substitutions: HashMap<_, _> = signature
                                .owner_parameters
                                .iter()
                                .cloned()
                                .zip(arguments)
                                .collect();
                            let positional: Vec<ResolvedType> = signature
                                .positional
                                .iter()
                                .map(|ty| self.state.model.substitute(ty, &substitutions))
                                .collect();
                            let named: HashMap<String, ResolvedType> = signature
                                .named
                                .iter()
                                .map(|(name, ty)| {
                                    (
                                        name.clone(),
                                        self.state.model.substitute(ty, &substitutions),
                                    )
                                })
                                .collect();
                            (positional, named)
                        }),
                    _ => None,
                };
                if let Some((positional, named)) = signature {
                    self.visit_call_arguments(args, &positional, &named);
                } else {
                    for argument in &args.positional {
                        self.visit_expr(argument);
                    }
                    for argument in &args.named {
                        self.visit_expr(&argument.value);
                    }
                }
            }
            Expr::FuncExpr {
                type_params,
                params,
                body,
                ..
            } => self.visit_function(type_params, params, None, Some(body), true),
            Expr::Assign {
                target,
                op: AssignOp::Eq,
                value,
                ..
            } => {
                self.visit_expr(target);
                let expected = self.state.infer(target);
                self.with_expected(expected, |walker| walker.visit_expr(value));
            }
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.visit_collection_element(element);
                }
            }
            Expr::Map {
                entries, elements, ..
            } => {
                for entry in entries {
                    self.visit_expr(&entry.key);
                    self.visit_expr(&entry.value);
                }
                for element in elements {
                    self.visit_map_element(element);
                }
            }
            Expr::Switch { subject, arms, .. } => {
                self.visit_expr(subject);
                let matched = self.state.infer(subject);
                for arm in arms {
                    self.with_scope(|walker| {
                        walker.visit_matched_pattern(&arm.pattern, matched.clone());
                        if let Some(guard) = &arm.guard {
                            walker.visit_expr(guard);
                        }
                        walker.visit_expr(&arm.body);
                    });
                }
            }
            _ => self.with_expected(ResolvedType::Unknown, |walker| walk_expr(walker, node)),
        }
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        self.visitor
            .visit_pattern(node, self.state.matched.last(), &self.state);
        walk_pattern(self, node);
    }
}

pub(crate) fn returned_expressions(body: &FunctionBody) -> Vec<Expr> {
    struct ReturnCollector {
        expressions: Vec<Expr>,
    }

    impl Visitor for ReturnCollector {
        fn visit_stmt(&mut self, node: &Stmt) {
            match node {
                Stmt::Return(ReturnStmt {
                    value: Some(value), ..
                }) => self.expressions.push(value.clone()),
                Stmt::LocalFunc(_) => {}
                _ => falcon_syntax::visitor::walk_stmt(self, node),
            }
        }

        fn visit_expr(&mut self, node: &Expr) {
            if !matches!(node, Expr::FuncExpr { .. }) {
                walk_expr(self, node);
            }
        }
    }

    let mut collector = ReturnCollector {
        expressions: Vec::new(),
    };
    match body {
        FunctionBody::Block(block) => {
            for statement in &block.stmts {
                collector.visit_stmt(statement);
            }
        }
        FunctionBody::Arrow(expression, _) => {
            collector.expressions.push(expression.as_ref().clone())
        }
        FunctionBody::Native(_, _) => {}
    }
    collector.expressions
}

fn collect_type_parameter_ids(
    ty: &ResolvedType,
    by_owner: &mut HashMap<usize, Vec<TypeParameterId>>,
) {
    match ty {
        ResolvedType::TypeParameter { id, bound, .. } => {
            let parameters = by_owner.entry(id.owner).or_default();
            if !parameters.contains(id) {
                parameters.push(id.clone());
            }
            collect_type_parameter_ids(bound, by_owner);
        }
        ResolvedType::Interface { arguments, .. } => {
            for argument in arguments {
                collect_type_parameter_ids(argument, by_owner);
            }
        }
        ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } => {
            collect_type_parameter_ids(return_type, by_owner);
            for parameter in positional {
                collect_type_parameter_ids(parameter, by_owner);
            }
            for (_, parameter) in named {
                collect_type_parameter_ids(parameter, by_owner);
            }
        }
        ResolvedType::Record {
            positional, named, ..
        } => {
            for field in positional {
                collect_type_parameter_ids(field, by_owner);
            }
            for (_, field) in named {
                collect_type_parameter_ids(field, by_owner);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn finds_nested_returns_without_crossing_function_boundaries() {
        let (program, errors) =
            parse("void f() { if (true) { return 1; } for (;;) { return 2; } () { return 3; }; }");
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let returns = returned_expressions(function.body.as_ref().unwrap());
        assert_eq!(returns.len(), 2);
    }
}
