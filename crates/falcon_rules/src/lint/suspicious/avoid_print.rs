//! Flags calls that resolve to `dart:core`'s top-level `print` function.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, SemanticModel};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

pub struct AvoidPrint;

impl Rule for AvoidPrint {
    fn name(&self) -> &'static str {
        "avoid-print"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        if !matches!(model.resolve_sdk_member(&["print".to_string()]), Some(DeclarationIdentity::Sdk { library, name }) if library == "dart:core" && name == "print")
        {
            return Vec::new();
        }
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            scopes: vec![HashSet::new()],
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector {
    diagnostics: Vec<Diagnostic>,
    file: String,
    scopes: Vec<HashSet<String>>,
}

impl Collector {
    fn push(&mut self) {
        self.scopes.push(HashSet::new());
    }
    fn pop(&mut self) {
        self.scopes.pop();
    }
    fn declare(&mut self, name: &str) {
        self.scopes
            .last_mut()
            .expect("scope")
            .insert(name.to_string());
    }
    fn bound(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }
    fn params(&mut self, params: &FormalParamList) {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            self.declare(&param.name.name);
        }
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
    fn function(&mut self, params: &FormalParamList, body: Option<&FunctionBody>) {
        self.push();
        self.params(params);
        if let Some(body) = body {
            self.body(body);
        }
        self.pop();
    }
}

impl Visitor for Collector {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.push();
        if node.members.iter().any(
            |member| matches!(member, ClassMember::Method(method) if method.name.name == "print"),
        ) {
            self.declare("print");
        }
        for member in &node.members {
            self.visit_class_member(member);
        }
        self.pop();
    }
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function(&node.params, node.body.as_ref());
    }
    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(&node.params, node.body.as_ref());
    }
    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.function(&node.params, node.body.as_ref());
    }
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call { callee, span, .. } = node
            && matches!(callee.as_ref(), Expr::Ident(identifier) if identifier.name == "print")
            && !self.bound("print")
        {
            self.diagnostics.push(Diagnostic::new(
                "avoid-print",
                Severity::Warning,
                "Avoid using print in production code; use a logging framework instead.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        if let Expr::FuncExpr { params, body, .. } = node {
            self.function(params, Some(body));
        } else {
            walk_expr(self, node);
        }
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
            Stmt::LocalVar(declaration) => {
                for declarator in &declaration.declarators {
                    if let Some(initializer) = &declarator.initializer {
                        self.visit_expr(initializer);
                    }
                    self.declare(&declarator.name.name);
                }
            }
            Stmt::LocalFunc(function) => {
                self.declare(&function.name.name);
                self.function(&function.params, Some(&function.body));
            }
            Stmt::For(statement) => {
                self.push();
                if let Some(init) = &statement.init {
                    match init {
                        ForInit::VarDecl(declaration) => {
                            self.visit_stmt(&Stmt::LocalVar(declaration.clone()))
                        }
                        ForInit::ForIn { name, iterable, .. } => {
                            self.visit_expr(iterable);
                            self.declare(&name.name);
                        }
                        ForInit::PatternForIn { pattern, iterable } => {
                            self.visit_expr(iterable);
                            for name in falcon_syntax::visitor::bound_names(pattern) {
                                self.declare(&name.name);
                            }
                        }
                        ForInit::Exprs(expressions) => {
                            for expression in expressions {
                                self.visit_expr(expression);
                            }
                        }
                    }
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
                        self.declare(&name.name);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.declare(&name.name);
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
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                for name in falcon_syntax::visitor::bound_names(&declaration.pattern) {
                    self.declare(&name.name);
                }
            }
            _ => walk_stmt(self, node),
        }
    }
}
