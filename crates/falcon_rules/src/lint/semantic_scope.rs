use std::borrow::Cow;
use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, ParameterOwner, QualifiedName, ResolvedSignature, ResolvedType, SemanticModel,
    SignatureIndex, TypeEnvironment, TypeParameterId, TypeParameterScope,
};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_function_body, walk_pattern, walk_top_level_decl,
};

pub(crate) trait SemanticRuleVisitor {
    fn visit_expr(&mut self, _node: &Expr, _state: &SemanticState<'_>) {}

    fn visit_stmt(&mut self, _node: &Stmt, _state: &SemanticState<'_>) {}

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
    // Only the single-file fallback owns signatures.
    pub(crate) signatures: Cow<'a, SignatureIndex>,
    pub(crate) environment: TypeEnvironment,
    pub(crate) type_parameters: TypeParameterScope,
    expected: Vec<ResolvedType>,
    return_type: Vec<ResolvedType>,
    matched: Vec<ResolvedType>,
    pub(crate) enclosing_type: Option<ResolvedType>,
    superclass: Option<ResolvedType>,
    globals: HashMap<String, ResolvedType>,
}

impl<'a> SemanticState<'a> {
    pub(crate) fn new(program: &Program, ctx: &'a AnalyzeContext<'a>) -> Option<Self> {
        let identities = ctx.identities?;
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let signatures = ctx
            .signatures
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(SignatureIndex::from_program(program, &model)));
        let mut globals = HashMap::new();
        for declaration in &program.declarations {
            match declaration {
                TopLevelDecl::Variable(variable) => {
                    let ty = variable.var_type.as_ref().map(|ty| model.resolve_type(ty));
                    for declarator in &variable.declarators {
                        globals.insert(
                            declarator.name.name.clone(),
                            ty.clone().unwrap_or(ResolvedType::Unknown),
                        );
                    }
                }
                TopLevelDecl::Function(function)
                    if matches!(function.kind, FunctionKind::Setter(_)) =>
                {
                    if let Some(param) = function.params().and_then(|params| params.params.first())
                    {
                        globals.insert(
                            function.name.name.clone(),
                            resolved_param_type(param, &model, &TypeParameterScope::default()),
                        );
                    }
                }
                _ => {}
            }
        }
        Some(Self {
            model,
            signatures,
            environment: TypeEnvironment::new(),
            type_parameters: TypeParameterScope::default(),
            expected: Vec::new(),
            return_type: Vec::new(),
            matched: Vec::new(),
            enclosing_type: None,
            globals,
            superclass: None,
        })
    }

    pub(crate) fn infer(&self, expression: &Expr) -> ResolvedType {
        if matches!(expression, Expr::This { .. }) {
            return self.enclosing_type.clone().unwrap_or(ResolvedType::Unknown);
        }
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

    fn writable_type(&self, target: &Expr) -> ResolvedType {
        let member = match target {
            Expr::Ident(identifier) if !self.environment.is_bound(&identifier.name) => {
                if let Some(ty) = self.globals.get(&identifier.name) {
                    return ty.clone();
                }
                self.enclosing_type
                    .as_ref()
                    .map(|receiver| (receiver.clone(), identifier.name.as_str()))
            }
            Expr::Field { object, field, .. } => {
                if let Expr::Ident(identifier) = object.as_ref()
                    && !self.environment.is_bound(&identifier.name)
                    && let Some(identity) = self.model.resolve_name(&identifier.name)
                    && let Some((_, ty)) = self.signatures.field(&identity, &field.name)
                {
                    return ty.clone();
                }
                Some((self.infer(object), field.name.as_str()))
            }
            _ => None,
        };
        member
            .and_then(|(receiver, name)| {
                self.signatures
                    .resolved_writable_field(&receiver, name, &self.model)
                    .map(|(ty, substitutions)| self.model.substitute(&ty, &substitutions))
            })
            .unwrap_or_else(|| self.infer(target))
    }

    pub(crate) fn call_signature(
        &self,
        callee: &Expr,
        type_args: &[DartType],
    ) -> Option<ResolvedSignature> {
        let (target, explicit_type_args) = match callee {
            Expr::GenericInstantiation {
                target, type_args, ..
            } => (target.as_ref(), type_args.as_slice()),
            target => (target, type_args),
        };
        let resolved = match target {
            Expr::Ident(identifier) if !self.environment.is_bound(&identifier.name) => self
                .enclosing_type
                .as_ref()
                .and_then(|receiver| {
                    self.signatures
                        .resolved_member(receiver, &identifier.name, &self.model)
                })
                .or_else(|| {
                    self.model
                        .resolve_value(&identifier.name)
                        .and_then(|identity| self.signatures.function(&identity).cloned())
                        .map(|signature| (signature, HashMap::new()))
                })
                .or_else(|| {
                    self.model
                        .resolve_name(&identifier.name)
                        .and_then(|identity| self.signatures.constructor(&identity, "new").cloned())
                        .map(|signature| (signature, HashMap::new()))
                }),
            Expr::Field { object, field, .. } => {
                if let Expr::Ident(prefix) = object.as_ref()
                    && !self.environment.is_bound(&prefix.name)
                    && let Some(identity) = self
                        .model
                        .resolve_value(QualifiedName::prefixed(&prefix.name, &field.name))
                    && let Some(signature) = self.signatures.function(&identity)
                {
                    Some((signature.clone(), HashMap::new()))
                } else if let Expr::Ident(prefix) = object.as_ref()
                    && !self.environment.is_bound(&prefix.name)
                    && let Some(identity) = self.model.resolve_name(&prefix.name)
                {
                    self.signatures
                        .constructor(&identity, &field.name)
                        .or_else(|| self.signatures.member(&identity, &field.name))
                        .cloned()
                        .map(|signature| (signature, HashMap::new()))
                } else {
                    self.signatures
                        .resolved_member(&self.infer(object), &field.name, &self.model)
                }
            }
            _ => None,
        };
        let resolved = resolved.or_else(|| match self.infer(target) {
            ResolvedType::Function {
                return_type,
                positional,
                named,
                ..
            } => Some((
                ResolvedSignature {
                    owner_parameters: Vec::new(),
                    call_parameters: Vec::new(),
                    positional,
                    named: named.into_iter().collect(),
                    return_type: *return_type,
                },
                HashMap::new(),
            )),
            _ => None,
        });
        let (mut signature, mut substitutions) = resolved?;
        substitutions.extend(
            signature
                .call_parameters
                .iter()
                .zip(explicit_type_args)
                .map(|(parameter, argument)| {
                    (
                        parameter.clone(),
                        self.model.resolve_type_in(argument, &self.type_parameters),
                    )
                }),
        );
        signature.return_type = self
            .model
            .substitute(&signature.return_type, &substitutions);
        for ty in &mut signature.positional {
            *ty = self.model.substitute(ty, &substitutions);
        }
        for ty in signature.named.values_mut() {
            *ty = self.model.substitute(ty, &substitutions);
        }
        Some(signature)
    }
}

pub(crate) fn visit_program<'a, V: SemanticRuleVisitor>(
    visitor: &mut V,
    program: &Program,
    state: SemanticState<'a>,
) -> SemanticState<'a> {
    let mut walker = ScopedWalker { visitor, state };
    walker.visit_program(program);
    walker.state
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
            .positional()
            .map(|param| resolved_param_type(param, model, type_parameters))
            .collect(),
        named: function_params
            .named()
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
            FunctionBody::Arrow(expression, _) => {
                let expected = self
                    .state
                    .return_type
                    .last()
                    .cloned()
                    .unwrap_or(ResolvedType::Unknown);
                self.with_expected(expected, |walker| walker.visit_expr(expression));
            }
            _ => walk_function_body(self, body),
        }
    }

    fn visit_function(
        &mut self,
        type_parameters: &[TypeParam],
        params: Option<&FormalParamList>,
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
            if let Some(params) = params {
                walker.bind_params(params, None);
                walker.visit_param_defaults(params);
            }
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
        let mut by_owner: HashMap<ParameterOwner, Vec<TypeParameterId>> = HashMap::new();
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
        for param in params.params.iter() {
            let mut ty = resolved_param_type(param, &self.state.model, &self.state.type_parameters);
            if param.param_type.is_none() && param.function_params.is_none() {
                if param.initializing == Some(InitializingFormal::Super) {
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
                } else if param.initializing == Some(InitializingFormal::This) {
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
        for param in params.params.iter() {
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
                if let ConstPatternValue::Expr(expression) = &constant.value {
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
            ForInit::PatternForIn {
                pattern, iterable, ..
            } => {
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

    fn visit_for_header(&mut self, header: &ForHeader) {
        if let Some(init) = &header.init {
            self.visit_for_init(init);
        }
        if let Some(condition) = &header.condition {
            self.visit_expr(condition);
        }
        for update in &header.updates {
            self.visit_expr(update);
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
                header, element, ..
            } => self.with_scope(|walker| {
                walker.visit_for_header(header);
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
            MapElement::For { header, entry, .. } => self.with_scope(|walker| {
                walker.visit_for_header(header);
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
                        .and_then(|name| walker.state.model.resolve_name(name))
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
        } else if let TopLevelDecl::Variable(variable) = node {
            for declarator in &variable.declarators {
                if let Some(initializer) = &declarator.initializer {
                    let expected = self
                        .state
                        .globals
                        .get(&declarator.name.name)
                        .cloned()
                        .unwrap_or(ResolvedType::Unknown);
                    self.with_expected(expected, |walker| walker.visit_expr(initializer));
                }
            }
        } else {
            walk_top_level_decl(self, node);
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.visit_function(
            &node.type_params,
            node.params(),
            node.return_type.as_ref(),
            node.body.as_ref(),
            false,
        );
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.visit_function(
            &node.type_params,
            Some(&node.params),
            node.return_type.as_ref(),
            node.body.as_ref(),
            false,
        );
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.visit_function(
                &[],
                Some(&operator.params),
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
        self.visit_function(
            &[],
            None,
            node.return_type.as_ref(),
            node.body.as_ref(),
            false,
        );
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let params = FormalParamList {
            params: vec![FormalParam {
                annotations: Vec::new(),
                kind: falcon_syntax::ast::ParamKind::Positional,
                is_covariant: false,
                is_final: false,
                initializing: None,
                param_type: node.param_type.clone(),
                name: node.param.clone(),
                default_value: None,
                type_params: Vec::new(),
                function_params: None,
                span: node.param.span,
            }],
            span: node.span,
        };
        self.visit_function(&[], Some(&params), None, node.body.as_ref(), false);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        self.visitor.visit_stmt(node, &self.state);
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
                if let Some(init) = &statement.header.init {
                    walker.visit_for_init(init);
                }
                if let Some(condition) = &statement.header.condition {
                    walker.visit_expr(condition);
                }
                for update in &statement.header.updates {
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
                let ty = ResolvedType::Function {
                    return_type: Box::new(
                        function
                            .return_type
                            .as_ref()
                            .map(|ty| {
                                self.state
                                    .model
                                    .resolve_type_in(ty, &self.state.type_parameters)
                            })
                            .unwrap_or(ResolvedType::Dynamic),
                    ),
                    positional: function
                        .params
                        .positional()
                        .map(|param| {
                            resolved_param_type(
                                param,
                                &self.state.model,
                                &self.state.type_parameters,
                            )
                        })
                        .collect(),
                    named: function
                        .params
                        .named()
                        .map(|param| {
                            (
                                param.name.name.clone(),
                                resolved_param_type(
                                    param,
                                    &self.state.model,
                                    &self.state.type_parameters,
                                ),
                            )
                        })
                        .collect(),
                    nullable: false,
                };
                self.state
                    .environment
                    .declare(function.name.name.clone(), ty);
                self.visit_function(
                    &function.type_params,
                    Some(&function.params),
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
            Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::Rethrow(_)
            | Stmt::Empty(_)
            | Stmt::Error(_) => {}
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
                if let Some(signature) = self.state.call_signature(callee, type_args) {
                    self.visit_call_arguments(args, &signature.positional, &signature.named);
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
            } => self.visit_function(type_params, Some(params), None, Some(body), true),
            Expr::Assign {
                target,
                op: AssignOp::Eq,
                value,
                ..
            } => {
                self.visit_expr(target);
                let expected = self.state.writable_type(target);
                self.with_expected(expected, |walker| walker.visit_expr(value));
            }
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for element in elements {
                    self.visit_collection_element(element);
                }
            }
            Expr::Map { elements, .. } => {
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

pub(crate) fn returned_expressions(body: &FunctionBody) -> Vec<&Expr> {
    fn collect<'a>(statements: &'a [Stmt], expressions: &mut Vec<&'a Expr>) {
        for statement in statements {
            collect_statement(statement, expressions);
        }
    }

    fn collect_statement<'a>(statement: &'a Stmt, expressions: &mut Vec<&'a Expr>) {
        match statement {
            Stmt::Return(ReturnStmt {
                value: Some(value), ..
            }) => expressions.push(value),
            Stmt::Block(block) => collect(&block.stmts, expressions),
            Stmt::If(node) => {
                collect_statement(&node.then_branch, expressions);
                if let Some(branch) = &node.else_branch {
                    collect_statement(branch, expressions);
                }
            }
            Stmt::For(node) => collect_statement(&node.body, expressions),
            Stmt::While(node) => collect_statement(&node.body, expressions),
            Stmt::DoWhile(node) => collect_statement(&node.body, expressions),
            Stmt::Switch(node) => {
                for case in &node.cases {
                    collect(&case.body, expressions);
                }
            }
            Stmt::TryCatch(node) => {
                collect(&node.body.stmts, expressions);
                for catch in &node.catches {
                    collect(&catch.body.stmts, expressions);
                }
                if let Some(block) = &node.finally {
                    collect(&block.stmts, expressions);
                }
            }
            Stmt::Labeled(node) => collect_statement(&node.stmt, expressions),
            // Expressions contain statements only inside excluded function expressions.
            Stmt::LocalFunc(_)
            | Stmt::Return(_)
            | Stmt::Throw(_)
            | Stmt::Rethrow(_)
            | Stmt::Empty(_)
            | Stmt::Break(_)
            | Stmt::Continue(_)
            | Stmt::LocalVar(_)
            | Stmt::PatternDecl(_)
            | Stmt::PatternAssign(_)
            | Stmt::Assert(_)
            | Stmt::Yield(_)
            | Stmt::Expr(_)
            | Stmt::Error(_) => {}
        }
    }

    let mut expressions = Vec::new();
    match body {
        FunctionBody::Block(block) => collect(&block.stmts, &mut expressions),
        FunctionBody::Arrow(expression, _) => expressions.push(expression.as_ref()),
        FunctionBody::Native(_, _) => {}
    }
    expressions
}

fn collect_type_parameter_ids(
    ty: &ResolvedType,
    by_owner: &mut HashMap<ParameterOwner, Vec<TypeParameterId>>,
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
        let (program, errors) = parse(
            "void f() {
                if (true) { return 1; } else { return 2; }
                for (;;) { return 3; }
                while (true) { return 4; }
                do { return 5; } while (true);
                switch (1) { case 1: return 6; default: return 7; }
                try { return 8; } catch (e) { return 9; } finally { return 10; }
                label: { return 11; }
                void local() { return 12; }
                () { return 13; };
                return () { return 14; };
            }",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!()
        };
        let returns = returned_expressions(function.body.as_ref().unwrap());
        assert_eq!(returns.len(), 12);
        for (expression, expected) in returns
            .iter()
            .zip(["1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11"])
        {
            assert!(matches!(expression, Expr::IntLit { value, .. } if value == expected));
        }
        assert!(matches!(returns[11], Expr::FuncExpr { .. }));
    }

    #[test]
    fn returns_borrowed_block_and_arrow_expressions() {
        let (program, errors) = parse("int block() { return 1; } int arrow() => 2;");
        assert!(errors.is_empty(), "{errors:?}");
        for declaration in &program.declarations {
            let TopLevelDecl::Function(function) = declaration else {
                panic!()
            };
            let body = function.body.as_ref().unwrap();
            let expression = match body {
                FunctionBody::Block(block) => {
                    let Stmt::Return(ReturnStmt {
                        value: Some(expression),
                        ..
                    }) = &block.stmts[0]
                    else {
                        panic!()
                    };
                    expression
                }
                FunctionBody::Arrow(expression, _) => expression.as_ref(),
                FunctionBody::Native(_, _) => panic!(),
            };
            let returns = returned_expressions(body);
            assert_eq!(returns.len(), 1);
            assert!(std::ptr::eq(returns[0], expression));
        }
    }
}
