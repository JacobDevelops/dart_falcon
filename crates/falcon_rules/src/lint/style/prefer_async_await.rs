//! Flags `.then(...)` future chains that should use `async`/`await`.
//!
//! Chaining callbacks with `Future.then` nests logic inside closures and scatters
//! error handling across `onError`/`catchError`, whereas `async`/`await` lets
//! asynchronous code read top-to-bottom with ordinary `try`/`catch`. Function
//! literals passed to a chain are treated as delayed scopes and remain opaque.
//!
//! Matching is currently syntactic on the `then` name. Resolver-backed Future
//! identity is required before unrelated user-defined `then` methods can be
//! excluded reliably.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_constructor_decl, walk_expr, walk_function_decl,
    walk_getter_decl, walk_method_decl, walk_setter_decl, walk_stmt,
};

pub struct PreferAsyncAwait;

impl Rule for PreferAsyncAwait {
    fn name(&self) -> &'static str {
        "prefer-async-await"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            suppress: false,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    suppress: bool,
}

impl Collector<'_, '_> {
    fn with_body_suppression(&mut self, body: Option<&FunctionBody>, walk: impl FnOnce(&mut Self)) {
        let previous = self.suppress;
        self.suppress |= matches!(body, Some(FunctionBody::Arrow(..)));
        walk(self);
        self.suppress = previous;
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.with_body_suppression(node.body.as_ref(), |this| walk_function_decl(this, node));
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.with_body_suppression(node.body.as_ref(), |this| walk_constructor_decl(this, node));
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.with_body_suppression(node.body.as_ref(), |this| walk_method_decl(this, node));
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.with_body_suppression(node.body.as_ref(), |this| walk_getter_decl(this, node));
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.with_body_suppression(node.body.as_ref(), |this| walk_setter_decl(this, node));
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.with_body_suppression(operator.body.as_ref(), |this| {
                walk_class_member(this, node)
            });
        } else {
            walk_class_member(self, node);
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if !matches!(node, Stmt::LocalFunc(_)) {
            walk_stmt(self, node);
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::FuncExpr { .. }) {
            return;
        }
        if !self.suppress
            && let Expr::Call { callee, span, .. } = node
            && let Expr::Field { field, .. } = callee.as_ref()
            && field.name == "then"
        {
            self.diags.push(Diagnostic::new(
                "prefer-async-await",
                Severity::Warning,
                "Prefer async/await over .then() chains",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}
