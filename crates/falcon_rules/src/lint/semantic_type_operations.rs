use falcon_analyze::{
    AnalyzeContext, ResolvedType, SemanticModel, SignatureIndex, TypeEnvironment,
    TypeParameterScope,
};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_pattern, walk_stmt};

use super::semantic_scope::{resolved_param_type, resolved_super_param_type};

pub(crate) enum TypeOperationKind {
    Is { negated: bool },
    As,
}

pub(crate) struct TypeOperation {
    pub(crate) kind: TypeOperationKind,
    pub(crate) operand: ResolvedType,
    pub(crate) target: ResolvedType,
    pub(crate) span: Span,
}

pub(crate) struct TypeOperationAnalysis<'a> {
    pub(crate) model: SemanticModel<'a>,
    pub(crate) signatures: SignatureIndex,
    pub(crate) operations: Vec<TypeOperation>,
}

pub(crate) fn collect<'a>(
    program: &Program,
    ctx: &'a AnalyzeContext<'a>,
) -> Option<TypeOperationAnalysis<'a>> {
    let identities = ctx.identities?;
    let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
    let signatures = ctx
        .signatures
        .cloned()
        .unwrap_or_else(|| SignatureIndex::from_program(program, &model));
    let mut collector = Collector {
        model,
        signatures,
        environment: TypeEnvironment::new(),
        type_parameters: TypeParameterScope::default(),
        enclosing_type: None,
        superclass: None,
        operations: Vec::new(),
    };
    collector.visit_program(program);
    Some(TypeOperationAnalysis {
        model: collector.model,
        signatures: collector.signatures,
        operations: collector.operations,
    })
}

struct Collector<'a> {
    model: SemanticModel<'a>,
    signatures: SignatureIndex,
    environment: TypeEnvironment,
    type_parameters: TypeParameterScope,
    enclosing_type: Option<ResolvedType>,
    superclass: Option<ResolvedType>,
    operations: Vec<TypeOperation>,
}

impl Collector<'_> {
    fn infer(&self, expression: &Expr) -> ResolvedType {
        self.environment.infer_with_signatures(
            expression,
            &self.model,
            &self.signatures,
            &self.type_parameters,
        )
    }

    fn body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(block) => {
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
            }
            FunctionBody::Arrow(expression, _) => self.visit_expr(expression),
            FunctionBody::Native(_, _) => {}
        }
    }

    fn function(
        &mut self,
        type_params: &[TypeParam],
        params: &FormalParamList,
        body: Option<&FunctionBody>,
    ) {
        let saved = self.environment.clone();
        self.environment.push_scope();
        self.type_parameters.push(type_params, &self.model);
        self.bind_params(params, None);
        if let Some(body) = body {
            self.body(body);
        }
        self.type_parameters.pop();
        self.environment = saved;
    }

    fn bind_params(&mut self, params: &FormalParamList, super_constructor: Option<&str>) {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            let mut ty = resolved_param_type(param, &self.model, &self.type_parameters);
            if param.param_type.is_none() && param.function_params.is_none() {
                if param.is_super {
                    ty = super_constructor
                        .and_then(|constructor| {
                            self.superclass.as_ref().and_then(|superclass| {
                                resolved_super_param_type(
                                    param,
                                    superclass,
                                    constructor,
                                    &self.signatures,
                                    &self.model,
                                )
                            })
                        })
                        .unwrap_or(ResolvedType::Unknown);
                } else if param.is_field {
                    ty = self.environment.lookup(&param.name.name);
                    if matches!(ty, ResolvedType::Unknown) {
                        ty = self
                            .enclosing_type
                            .as_ref()
                            .and_then(|enclosing| {
                                self.signatures.resolved_field(
                                    enclosing,
                                    &param.name.name,
                                    &self.model,
                                )
                            })
                            .map(|(ty, substitutions)| self.model.substitute(&ty, &substitutions))
                            .unwrap_or(ResolvedType::Unknown);
                    }
                }
            }
            self.environment.declare(param.name.name.clone(), ty);
        }
    }

    fn constructor(&mut self, declaration: &ConstructorDecl) {
        let saved = self.environment.clone();
        self.environment.push_scope();
        self.type_parameters.push(&[], &self.model);
        let redirects_to_this = declaration
            .initializers
            .iter()
            .any(|initializer| matches!(initializer, ConstructorInitializer::ThisCall { .. }));
        let super_constructor = (!redirects_to_this).then(|| {
            declaration
                .initializers
                .iter()
                .find_map(|initializer| match initializer {
                    ConstructorInitializer::SuperCall { call_name, .. } => {
                        Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
                    }
                    _ => None,
                })
                .unwrap_or("new")
        });
        self.bind_params(&declaration.params, super_constructor);
        for initializer in &declaration.initializers {
            match initializer {
                ConstructorInitializer::SuperCall { args, .. }
                | ConstructorInitializer::ThisCall { args, .. } => {
                    for argument in &args.positional {
                        self.visit_expr(argument);
                    }
                    for argument in &args.named {
                        self.visit_expr(&argument.value);
                    }
                }
                ConstructorInitializer::FieldInit { value, .. } => self.visit_expr(value),
                ConstructorInitializer::Assert {
                    condition, message, ..
                } => {
                    self.visit_expr(condition);
                    if let Some(message) = message {
                        self.visit_expr(message);
                    }
                }
            }
        }
        if let Some(body) = &declaration.body {
            self.body(body);
        }
        self.type_parameters.pop();
        self.environment = saved;
    }

    fn declare_local(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
            let ty = declaration
                .var_type
                .as_ref()
                .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                .or_else(|| {
                    declarator
                        .initializer
                        .as_ref()
                        .map(|initializer| self.infer(initializer))
                })
                .unwrap_or(ResolvedType::Unknown);
            self.environment.declare(declarator.name.name.clone(), ty);
        }
    }

    fn iterable_element_type(&self, iterable: &Expr) -> ResolvedType {
        let iterable = self.infer(iterable);
        self.signatures
            .instantiated_supertype(&iterable, "dart:core", "Iterable", &self.model)
            .and_then(|ty| ty.arguments().first().cloned())
            .unwrap_or(ResolvedType::Unknown)
    }

    fn bind_pattern(&mut self, pattern: &Pattern, matched: ResolvedType) {
        match pattern {
            Pattern::Variable { type_, name, .. } => {
                let ty = type_
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .unwrap_or(matched);
                self.environment.declare(name.name.clone(), ty);
            }
            Pattern::List(list) => {
                let element = list
                    .type_arg
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .or_else(|| {
                        self.signatures
                            .instantiated_supertype(&matched, "dart:core", "List", &self.model)
                            .and_then(|ty| ty.arguments().first().cloned())
                    })
                    .unwrap_or(ResolvedType::Unknown);
                for child in &list.elements {
                    match child {
                        ListPatternElement::Pattern(child) => {
                            self.bind_pattern(child, element.clone());
                        }
                        ListPatternElement::Rest(Some(child), _) => {
                            self.bind_pattern(child, matched.clone());
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
                    self.bind_pattern(&field.pattern, field_type);
                }
            }
            Pattern::Map(map) => {
                let value_type = self
                    .signatures
                    .instantiated_supertype(&matched, "dart:core", "Map", &self.model)
                    .and_then(|ty| ty.arguments().get(1).cloned())
                    .unwrap_or(ResolvedType::Unknown);
                for entry in &map.entries {
                    self.bind_pattern(&entry.pattern, value_type.clone());
                }
            }
            Pattern::Object(object) => {
                let object_type = self
                    .model
                    .resolve_type_in(&object.type_, &self.type_parameters);
                for field in &object.fields {
                    let field_type = self
                        .signatures
                        .resolved_field(&object_type, &field.name.name, &self.model)
                        .map(|(ty, substitutions)| self.model.substitute(&ty, &substitutions))
                        .unwrap_or(ResolvedType::Unknown);
                    if let Some(child) = &field.pattern {
                        self.bind_pattern(child, field_type);
                    } else {
                        self.environment
                            .declare(field.name.name.clone(), field_type);
                    }
                }
            }
            Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
                self.bind_pattern(left, matched.clone());
                self.bind_pattern(right, matched);
            }
            Pattern::Cast {
                inner, cast_type, ..
            } => {
                let cast = self.model.resolve_type_in(cast_type, &self.type_parameters);
                self.bind_pattern(inner, cast);
            }
            Pattern::NullCheck { inner, .. } | Pattern::NullAssert { inner, .. } => {
                self.bind_pattern(inner, matched.with_nullable(false));
            }
            Pattern::ParenPattern { inner, .. } => self.bind_pattern(inner, matched),
            Pattern::Wildcard { .. }
            | Pattern::Literal(_)
            | Pattern::Const(_)
            | Pattern::Relational { .. }
            | Pattern::Error { .. } => {}
        }
    }

    fn type_declaration(
        &mut self,
        name: Option<&str>,
        superclass: Option<&DartType>,
        type_params: &[TypeParam],
        members: &[ClassMember],
    ) {
        let saved = std::mem::replace(&mut self.environment, TypeEnvironment::new());
        let previous_enclosing = self.enclosing_type.replace(
            name.and_then(|name| self.model.resolve_name(&[name.to_string()]))
                .map(|identity| ResolvedType::Interface {
                    identity,
                    arguments: Vec::new(),
                    nullable: false,
                    extension_type: false,
                })
                .unwrap_or(ResolvedType::Unknown),
        );
        self.type_parameters.push(type_params, &self.model);
        let previous_superclass = std::mem::replace(
            &mut self.superclass,
            superclass.map(|superclass| {
                self.model
                    .resolve_type_in(superclass, &self.type_parameters)
            }),
        );
        for member in members {
            if let ClassMember::Field(field) = member {
                for declarator in &field.declarators {
                    let ty = field
                        .field_type
                        .as_ref()
                        .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                        .or_else(|| {
                            declarator
                                .initializer
                                .as_ref()
                                .map(|initializer| self.infer(initializer))
                        })
                        .unwrap_or(ResolvedType::Unknown);
                    self.environment.declare(declarator.name.name.clone(), ty);
                }
            }
        }
        for member in members {
            self.visit_class_member(member);
        }
        self.superclass = previous_superclass;
        self.type_parameters.pop();
        self.enclosing_type = previous_enclosing;
        self.environment = saved;
    }

    fn for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.declare_local(declaration),
            ForInit::ForIn {
                var_type,
                name,
                iterable,
                ..
            } => {
                self.visit_expr(iterable);
                let ty = var_type
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .unwrap_or_else(|| self.iterable_element_type(iterable));
                self.environment.declare(name.name.clone(), ty);
            }
            ForInit::PatternForIn { pattern, iterable } => {
                self.visit_expr(iterable);
                walk_pattern(self, pattern);
                let matched = self.iterable_element_type(iterable);
                self.bind_pattern(pattern, matched);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        match node {
            TopLevelDecl::Class(declaration) => {
                self.type_declaration(
                    Some(&declaration.name.name),
                    declaration.extends.as_ref(),
                    &declaration.type_params,
                    &declaration.members,
                );
            }
            TopLevelDecl::Mixin(declaration) => {
                self.type_declaration(
                    Some(&declaration.name.name),
                    None,
                    &declaration.type_params,
                    &declaration.members,
                );
            }
            TopLevelDecl::MixinClass(declaration) => {
                self.type_declaration(
                    Some(&declaration.name.name),
                    declaration.extends.as_ref(),
                    &declaration.type_params,
                    &declaration.members,
                );
            }
            TopLevelDecl::Enum(declaration) => {
                self.type_declaration(
                    Some(&declaration.name.name),
                    None,
                    &declaration.type_params,
                    &declaration.members,
                );
            }
            TopLevelDecl::Extension(declaration) => {
                self.type_declaration(
                    declaration.name.as_ref().map(|name| name.name.as_str()),
                    None,
                    &declaration.type_params,
                    &declaration.members,
                );
            }
            TopLevelDecl::ExtensionType(declaration) => {
                self.type_declaration(
                    Some(&declaration.name.name),
                    None,
                    &declaration.type_params,
                    &declaration.members,
                );
            }
            _ => falcon_syntax::visitor::walk_top_level_decl(self, node),
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function(&node.type_params, &node.params, node.body.as_ref());
    }
    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(&node.type_params, &node.params, node.body.as_ref());
    }
    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.constructor(node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => {
                self.environment.push_scope();
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
                self.environment.pop_scope();
            }
            Stmt::LocalVar(declaration) => self.declare_local(declaration),
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                walk_pattern(self, &declaration.pattern);
                let matched = self.infer(&declaration.init);
                self.bind_pattern(&declaration.pattern, matched);
            }
            Stmt::If(statement) => match &statement.condition {
                IfCondition::Expr(condition) => {
                    self.visit_expr(condition);
                    self.visit_stmt(&statement.then_branch);
                    if let Some(branch) = &statement.else_branch {
                        self.visit_stmt(branch);
                    }
                }
                IfCondition::Case(value, pattern, guard) => {
                    self.visit_expr(value);
                    self.environment.push_scope();
                    walk_pattern(self, pattern);
                    let matched = self.infer(value);
                    self.bind_pattern(pattern, matched);
                    if let Some(guard) = guard {
                        self.visit_expr(guard);
                    }
                    self.visit_stmt(&statement.then_branch);
                    self.environment.pop_scope();
                    if let Some(branch) = &statement.else_branch {
                        self.visit_stmt(branch);
                    }
                }
            },
            Stmt::For(statement) => {
                self.environment.push_scope();
                if let Some(init) = &statement.init {
                    self.for_init(init);
                }
                if let Some(condition) = &statement.condition {
                    self.visit_expr(condition);
                }
                for update in &statement.update {
                    self.visit_expr(update);
                }
                self.visit_stmt(&statement.body);
                self.environment.pop_scope();
            }
            Stmt::TryCatch(statement) => {
                self.environment.push_scope();
                for statement in &statement.body.stmts {
                    self.visit_stmt(statement);
                }
                self.environment.pop_scope();
                for catch in &statement.catches {
                    self.environment.push_scope();
                    if let Some(name) = &catch.exception_var {
                        let ty = catch
                            .exception_type
                            .as_ref()
                            .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                            .unwrap_or(ResolvedType::Unknown);
                        self.environment.declare(name.name.clone(), ty);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.environment
                            .declare(name.name.clone(), ResolvedType::Unknown);
                    }
                    for statement in &catch.body.stmts {
                        self.visit_stmt(statement);
                    }
                    self.environment.pop_scope();
                }
                if let Some(finally) = &statement.finally {
                    self.environment.push_scope();
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                    self.environment.pop_scope();
                }
            }
            Stmt::LocalFunc(function) => self.function(
                &function.type_params,
                &function.params,
                Some(&function.body),
            ),
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Is {
                expr,
                dart_type,
                negated,
                span,
            } => self.operations.push(TypeOperation {
                kind: TypeOperationKind::Is { negated: *negated },
                operand: self.infer(expr),
                target: self.model.resolve_type_in(dart_type, &self.type_parameters),
                span: span.clone(),
            }),
            Expr::As {
                expr,
                dart_type,
                span,
            } => self.operations.push(TypeOperation {
                kind: TypeOperationKind::As,
                operand: self.infer(expr),
                target: self.model.resolve_type_in(dart_type, &self.type_parameters),
                span: span.clone(),
            }),
            Expr::FuncExpr {
                type_params,
                params,
                body,
                ..
            } => {
                self.function(type_params, params, Some(body));
                return;
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
