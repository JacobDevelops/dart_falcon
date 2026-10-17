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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferAsyncAwait;

impl Rule for PreferAsyncAwait {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            ctx,
            arrow_depth: 0,
        }))
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    arrow_depth: usize,
}

impl Collector<'_, '_> {
    fn enter_body(&mut self, body: Option<&FunctionBody>) {
        self.arrow_depth += usize::from(matches!(body, Some(FunctionBody::Arrow(..))));
    }

    fn exit_body(&mut self, body: Option<&FunctionBody>) {
        self.arrow_depth -= usize::from(matches!(body, Some(FunctionBody::Arrow(..))));
    }
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        self.enter_body(node.body.as_ref());
        StructuralTraversal::Continue
    }
    fn exit_function_decl(&mut self, node: &FunctionDecl) { self.exit_body(node.body.as_ref()); }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) -> StructuralTraversal {
        self.enter_body(node.body.as_ref());
        StructuralTraversal::Continue
    }
    fn exit_constructor_decl(&mut self, node: &ConstructorDecl) { self.exit_body(node.body.as_ref()); }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.enter_body(node.body.as_ref());
        StructuralTraversal::Continue
    }
    fn exit_method_decl(&mut self, node: &MethodDecl) { self.exit_body(node.body.as_ref()); }

    fn visit_getter_decl(&mut self, node: &GetterDecl) -> StructuralTraversal {
        self.enter_body(node.body.as_ref());
        StructuralTraversal::Continue
    }
    fn exit_getter_decl(&mut self, node: &GetterDecl) { self.exit_body(node.body.as_ref()); }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
        self.enter_body(node.body.as_ref());
        StructuralTraversal::Continue
    }
    fn exit_setter_decl(&mut self, node: &SetterDecl) { self.exit_body(node.body.as_ref()); }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.enter_body(operator.body.as_ref());
        }
        StructuralTraversal::Continue
    }
    fn exit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node { self.exit_body(operator.body.as_ref()); }
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::LocalFunc(local) = node { self.enter_body(Some(&local.body)); }
        StructuralTraversal::Continue
    }
    fn exit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(local) = node { self.exit_body(Some(&local.body)); }
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr { .. }) {
            return StructuralTraversal::SkipChildren;
        }
        if self.arrow_depth == 0
            && let Expr::Call { callee, span, .. } = node
            && let Expr::Field { field, .. } = callee.unparenthesized()
            && field.name == "then"
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-async-await",
                "Prefer async/await over .then() chains",
                *span,
            ));
        }
        StructuralTraversal::Continue
    }
}
}
