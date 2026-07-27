//! Flags an `async` closure passed to a proven synchronous callback parameter.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ResolvedSignature, ResolvedType, Rule, SemanticModel, SignatureIndex,
    TypeEnvironment, TypeParameterScope, TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, bound_names, walk_expr, walk_pattern, walk_stmt};

pub struct AvoidPassingAsyncWhenSyncExpected;

impl Rule for AvoidPassingAsyncWhenSyncExpected {
    fn name(&self) -> &'static str {
        "avoid-passing-async-when-sync-expected"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
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
            names: vec![HashSet::new()],
            current_type: None,
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    model: SemanticModel<'a>,
    signatures: SignatureIndex,
    environment: TypeEnvironment,
    type_parameters: TypeParameterScope,
    names: Vec<HashSet<String>>,
    current_type: Option<ResolvedType>,
    diagnostics: Vec<Diagnostic>,
    file: String,
}

impl Collector<'_> {
    fn push(&mut self) {
        self.environment.push_scope();
        self.names.push(HashSet::new());
    }

    fn pop(&mut self) {
        self.environment.pop_scope();
        self.names.pop();
    }

    fn declare(&mut self, name: &str, ty: ResolvedType) {
        self.environment.declare(name.to_string(), ty);
        self.names
            .last_mut()
            .expect("lexical scope")
            .insert(name.to_string());
    }

    fn bound(&self, name: &str) -> bool {
        self.names.iter().rev().any(|scope| scope.contains(name))
    }

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
        self.push();
        self.type_parameters.push(type_params, &self.model);
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            let ty = param
                .param_type
                .as_ref()
                .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                .unwrap_or(ResolvedType::Dynamic);
            self.declare(&param.name.name, ty);
        }
        if let Some(body) = body {
            self.body(body);
        }
        self.type_parameters.pop();
        self.pop();
    }

    fn local(&mut self, declaration: &LocalVarDecl) {
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
                        .map(|value| self.infer(value))
                })
                .unwrap_or(ResolvedType::Unknown);
            self.declare(&declarator.name.name, ty);
        }
    }

    fn pattern(&mut self, pattern: &Pattern) {
        walk_pattern(self, pattern);
        for name in bound_names(pattern) {
            self.declare(&name.name, ResolvedType::Unknown);
        }
    }

    fn for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.local(declaration),
            ForInit::ForIn { name, iterable, .. } => {
                self.visit_expr(iterable);
                self.declare(&name.name, ResolvedType::Unknown);
            }
            ForInit::PatternForIn { pattern, iterable } => {
                self.visit_expr(iterable);
                self.pattern(pattern);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }

    fn resolved_signature(
        &self,
        callee: &Expr,
        type_args: &[DartType],
    ) -> Option<ResolvedSignature> {
        let (signature, mut substitutions) = match callee {
            Expr::Ident(identifier) if !self.bound(&identifier.name) => {
                if let Some(current_type) = &self.current_type
                    && let Some(resolved) =
                        self.signatures
                            .resolved_member(current_type, &identifier.name, &self.model)
                {
                    resolved
                } else {
                    let identity = self
                        .model
                        .resolve_value(std::slice::from_ref(&identifier.name))?;
                    (self.signatures.function(&identity)?.clone(), HashMap::new())
                }
            }
            Expr::Field { object, field, .. } => {
                if let Expr::Ident(prefix) = object.as_ref()
                    && !self.bound(&prefix.name)
                    && let Some(identity) = self
                        .model
                        .resolve_value(&[prefix.name.clone(), field.name.clone()])
                    && let Some(signature) = self.signatures.function(&identity)
                {
                    (signature.clone(), HashMap::new())
                } else {
                    let receiver = self.infer(object);
                    self.signatures
                        .resolved_member(&receiver, &field.name, &self.model)?
                }
            }
            _ => return None,
        };
        substitutions.extend(signature.call_parameters.iter().zip(type_args).map(
            |(parameter, argument)| {
                (
                    parameter.clone(),
                    self.model.resolve_type_in(argument, &self.type_parameters),
                )
            },
        ));
        Some(ResolvedSignature {
            owner_parameters: signature.owner_parameters,
            call_parameters: signature.call_parameters,
            positional: signature
                .positional
                .iter()
                .map(|ty| self.model.substitute(ty, &substitutions))
                .collect(),
            named: signature
                .named
                .iter()
                .map(|(name, ty)| (name.clone(), self.model.substitute(ty, &substitutions)))
                .collect(),
            return_type: self
                .model
                .substitute(&signature.return_type, &substitutions),
        })
    }

    fn check_argument(&mut self, argument: &Expr, expected: &ResolvedType) {
        let Expr::FuncExpr {
            is_async: true,
            span,
            ..
        } = argument
        else {
            return;
        };
        let ResolvedType::Function { return_type, .. } = expected else {
            return;
        };
        if self.model.is_future_like(return_type) != TypeTruth::No {
            return;
        }
        self.diagnostics.push(Diagnostic::new(
            "avoid-passing-async-when-sync-expected",
            Severity::Warning,
            "Avoid passing an async function where a synchronous callback is expected",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn check_call(&mut self, callee: &Expr, type_args: &[DartType], args: &ArgList) {
        let Some(signature) = self.resolved_signature(callee, type_args) else {
            return;
        };
        for (argument, expected) in args.positional.iter().zip(&signature.positional) {
            self.check_argument(argument, expected);
        }
        for argument in &args.named {
            if let Some(expected) = signature.named.get(&argument.name.name) {
                self.check_argument(&argument.value, expected);
            }
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        let saved_type = self.current_type.clone();
        self.type_parameters.push(&node.type_params, &self.model);
        self.current_type = self
            .model
            .resolve_name(std::slice::from_ref(&node.name.name))
            .map(|identity| ResolvedType::Interface {
                identity,
                arguments: node
                    .type_params
                    .iter()
                    .filter_map(|parameter| self.type_parameters.lookup(&parameter.name.name))
                    .collect(),
                nullable: false,
                extension_type: false,
            });
        for member in &node.members {
            self.visit_class_member(member);
        }
        self.type_parameters.pop();
        self.current_type = saved_type;
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function(&node.type_params, &node.params, node.body.as_ref());
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(&node.type_params, &node.params, node.body.as_ref());
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.function(&[], &node.params, node.body.as_ref());
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Block(block) => {
                self.push();
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
                self.pop();
            }
            Stmt::LocalVar(declaration) => self.local(declaration),
            Stmt::LocalFunc(function) => {
                self.declare(&function.name.name, ResolvedType::Unknown);
                self.function(
                    &function.type_params,
                    &function.params,
                    Some(&function.body),
                );
            }
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                self.pattern(&declaration.pattern);
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
                    self.push();
                    self.pattern(pattern);
                    if let Some(guard) = guard {
                        self.visit_expr(guard);
                    }
                    self.visit_stmt(&statement.then_branch);
                    self.pop();
                    if let Some(branch) = &statement.else_branch {
                        self.visit_stmt(branch);
                    }
                }
            },
            Stmt::For(statement) => {
                self.push();
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
                self.pop();
            }
            Stmt::TryCatch(statement) => {
                for statement in &statement.body.stmts {
                    self.visit_stmt(statement);
                }
                for catch in &statement.catches {
                    self.push();
                    if let Some(name) = &catch.exception_var {
                        self.declare(&name.name, ResolvedType::Unknown);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.declare(&name.name, ResolvedType::Unknown);
                    }
                    for statement in &catch.body.stmts {
                        self.visit_stmt(statement);
                    }
                    self.pop();
                }
                if let Some(finally) = &statement.finally {
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                }
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => self.check_call(callee, type_args, args),
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
