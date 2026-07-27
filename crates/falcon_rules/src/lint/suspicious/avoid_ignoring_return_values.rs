//! Flags a discarded call whose resolved return type is non-void.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, ResolvedSignature, ResolvedType, Rule, SemanticModel, SignatureIndex,
    TypeEnvironment, TypeParameterScope,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, bound_names, walk_expr, walk_pattern, walk_stmt};

pub struct AvoidIgnoringReturnValues;

impl Rule for AvoidIgnoringReturnValues {
    fn name(&self) -> &'static str {
        "avoid-ignoring-return-values"
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

    fn signature(&self, callee: &Expr) -> Option<ResolvedSignature> {
        match callee {
            Expr::Ident(identifier) if !self.bound(&identifier.name) => {
                if let Some(current_type) = &self.current_type
                    && let Some((signature, substitutions)) =
                        self.signatures
                            .resolved_member(current_type, &identifier.name, &self.model)
                {
                    return Some(substitute_signature(signature, &substitutions, &self.model));
                }
                let identity = self
                    .model
                    .resolve_value(std::slice::from_ref(&identifier.name))?;
                self.signatures.function(&identity).cloned()
            }
            Expr::Field { object, field, .. } => {
                if let Expr::Ident(prefix) = object.as_ref()
                    && !self.bound(&prefix.name)
                    && let Some(identity) = self
                        .model
                        .resolve_value(&[prefix.name.clone(), field.name.clone()])
                    && let Some(signature) = self.signatures.function(&identity)
                {
                    return Some(signature.clone());
                }
                let receiver = self.infer(object);
                let (signature, substitutions) =
                    self.signatures
                        .resolved_member(&receiver, &field.name, &self.model)?;
                Some(substitute_signature(signature, &substitutions, &self.model))
            }
            _ => None,
        }
    }

    fn check_discarded(&mut self, expression: &Expr) {
        let Expr::Call { callee, span, .. } = expression else {
            return;
        };
        let Some(signature) = self.signature(callee) else {
            return;
        };
        if matches!(
            signature.return_type,
            ResolvedType::Unknown | ResolvedType::Dynamic
        ) || self.model.void_context(&signature.return_type)
        {
            return;
        }
        self.diagnostics.push(Diagnostic::new(
            "avoid-ignoring-return-values",
            Severity::Warning,
            "The return value is not being used",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
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
            Stmt::Expr(statement) => {
                self.check_discarded(&statement.expr);
                self.visit_expr(&statement.expr);
            }
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
                self.push();
                for statement in &statement.body.stmts {
                    self.visit_stmt(statement);
                }
                self.pop();
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
                    self.push();
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                    self.pop();
                }
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr {
            type_params,
            params,
            body,
            ..
        } = node
        {
            self.function(type_params, params, Some(body));
        } else {
            walk_expr(self, node);
        }
    }
}

fn substitute_signature(
    signature: ResolvedSignature,
    substitutions: &std::collections::HashMap<falcon_analyze::TypeParameterId, ResolvedType>,
    model: &SemanticModel<'_>,
) -> ResolvedSignature {
    ResolvedSignature {
        owner_parameters: signature.owner_parameters,
        call_parameters: signature.call_parameters,
        positional: signature
            .positional
            .iter()
            .map(|ty| model.substitute(ty, substitutions))
            .collect(),
        named: signature
            .named
            .iter()
            .map(|(name, ty)| (name.clone(), model.substitute(ty, substitutions)))
            .collect(),
        return_type: model.substitute(&signature.return_type, substitutions),
    }
}
