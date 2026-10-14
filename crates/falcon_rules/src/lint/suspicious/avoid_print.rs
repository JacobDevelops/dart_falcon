//! Flags calls that resolve to `dart:core`'s top-level `print` function.

use crate::lint::lexical_usage::NameScopes;
use falcon_analyze::{AnalyzeContext, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body, walk_stmt};

#[derive(Default)]
pub struct AvoidPrint;

impl Rule for AvoidPrint {
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
            scopes: NameScopes::default(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    scopes: NameScopes,
}

impl Collector<'_> {
    fn local(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
            self.scopes.declare(&declarator.name.name);
        }
    }
    fn for_init(&mut self, init: &ForInit) {
        match init {
            ForInit::VarDecl(declaration) => self.visit_local_var(declaration),
            ForInit::ForIn { name, iterable, .. } => {
                self.visit_expr(iterable);
                self.scopes.declare(&name.name);
            }
            ForInit::PatternForIn {
                pattern, iterable, ..
            } => {
                self.visit_expr(iterable);
                self.scopes.pattern(pattern);
            }
            ForInit::Exprs(expressions) => {
                for expression in expressions {
                    self.visit_expr(expression);
                }
            }
        }
    }
    fn function(&mut self, params: Option<&FormalParamList>, body: Option<&FunctionBody>) {
        self.scopes.push();
        if let Some(params) = params {
            self.scopes.params(params);
        }
        if let Some(body) = body {
            walk_function_body(self, body);
        }
        self.scopes.pop();
    }
}

impl Visitor for Collector<'_> {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.scopes.push();
        if node.members.iter().any(
            |member| matches!(member, ClassMember::Method(method) if method.name.name == "print"),
        ) {
            self.scopes.declare("print");
        }
        for member in &node.members {
            self.visit_class_member(member);
        }
        self.scopes.pop();
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
            && !self.scopes.bound("print")
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
                self.scopes.push();
                for statement in &block.stmts {
                    self.visit_stmt(statement);
                }
                self.scopes.pop();
            }
            Stmt::LocalFunc(function) => {
                self.scopes.declare(&function.name.name);
                self.function(Some(&function.params), Some(&function.body));
            }
            Stmt::For(statement) => {
                self.scopes.push();
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
                self.scopes.pop();
            }
            Stmt::TryCatch(statement) => {
                self.scopes.push();
                for statement in &statement.body.stmts {
                    self.visit_stmt(statement);
                }
                self.scopes.pop();
                for catch in &statement.catches {
                    self.scopes.push();
                    if let Some(name) = &catch.exception_var {
                        self.scopes.declare(&name.name);
                    }
                    if let Some(name) = &catch.stack_trace_var {
                        self.scopes.declare(&name.name);
                    }
                    for statement in &catch.body.stmts {
                        self.visit_stmt(statement);
                    }
                    self.scopes.pop();
                }
                if let Some(finally) = &statement.finally {
                    self.scopes.push();
                    for statement in &finally.stmts {
                        self.visit_stmt(statement);
                    }
                    self.scopes.pop();
                }
            }
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                self.scopes.pattern(&declaration.pattern);
            }
            _ => walk_stmt(self, node),
        }
    }
}
