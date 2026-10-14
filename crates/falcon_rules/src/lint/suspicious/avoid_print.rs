//! Flags calls that resolve to `dart:core`'s top-level `print` function.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
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
        if !model
            .resolve_sdk_member("print")
            .is_some_and(|identity| identity.is_sdk("dart:core", "print"))
        {
            return Vec::new();
        }
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            scopes: vec![HashSet::new()],
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    scopes: Vec<HashSet<String>>,
}

impl Collector<'_> {
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
        for param in params.params.iter() {
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
    fn local(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
            self.declare(&declarator.name.name);
        }
    }
    fn for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_var(declaration),
            ForInit::ForIn { name, iterable, .. } => {
                self.visit_expr(iterable);
                self.declare(&name.name);
            }
            ForInit::PatternForIn {
                pattern, iterable, ..
            } => {
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
    fn function(&mut self, params: Option<&FormalParamList>, body: Option<&FunctionBody>) {
        self.push();
        if let Some(params) = params {
            self.params(params);
        }
        if let Some(body) = body {
            self.body(body);
        }
        self.pop();
    }
}

impl Visitor for Collector<'_> {
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
        self.function(node.params(), node.body.as_ref());
    }
    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(Some(&node.params), node.body.as_ref());
    }
    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.function(Some(&node.params), node.body.as_ref());
    }
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call { callee, span, .. } = node
            && matches!(callee.as_ref(), Expr::Ident(identifier) if identifier.name == "print")
            && !self.bound("print")
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-print",
                "Avoid using print in production code; use a logging framework instead.",
                *span,
            ));
        }
        if let Expr::FuncExpr { params, body, .. } = node {
            self.function(Some(params), Some(body));
        } else {
            walk_expr(self, node);
        }
    }
    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.local(node);
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
            Stmt::LocalFunc(function) => {
                self.declare(&function.name.name);
                self.function(Some(&function.params), Some(&function.body));
            }
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
                    self.push();
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                    self.pop();
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
