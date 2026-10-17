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
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferAsyncAwait;

impl Rule for PreferAsyncAwait {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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
    fn enter_body(&mut self, body: Option<FunctionBody<'_>>) {
        self.arrow_depth += usize::from(matches!(body, Some(FunctionBody::Arrow(..))));
    }

    fn exit_body(&mut self, body: Option<FunctionBody<'_>>) {
        self.arrow_depth -= usize::from(matches!(body, Some(FunctionBody::Arrow(..))));
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        self.enter_body(node.body());
        StructuralTraversal::Continue
    }
    fn exit_function_decl(&mut self, node: FunctionDecl<'a>) { self.exit_body(node.body()); }

    fn visit_constructor_decl(&mut self, node: ConstructorDecl<'a>) -> StructuralTraversal {
        self.enter_body(node.body());
        StructuralTraversal::Continue
    }
    fn exit_constructor_decl(&mut self, node: ConstructorDecl<'a>) { self.exit_body(node.body()); }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.enter_body(node.body());
        StructuralTraversal::Continue
    }
    fn exit_method_decl(&mut self, node: MethodDecl<'a>) { self.exit_body(node.body()); }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) -> StructuralTraversal {
        self.enter_body(node.body());
        StructuralTraversal::Continue
    }
    fn exit_getter_decl(&mut self, node: GetterDecl<'a>) { self.exit_body(node.body()); }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        self.enter_body(node.body());
        StructuralTraversal::Continue
    }
    fn exit_setter_decl(&mut self, node: SetterDecl<'a>) { self.exit_body(node.body()); }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.enter_body(operator.body());
        }
        StructuralTraversal::Continue
    }
    fn exit_class_member(&mut self, node: ClassMember<'a>) {
        if let ClassMember::Operator(operator) = node { self.exit_body(operator.body()); }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(local) = node { self.enter_body(Some(local.body())); }
        StructuralTraversal::Continue
    }
    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if let Stmt::LocalFunc(local) = node { self.exit_body(Some(local.body())); }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::FuncExpr(_)) {
            return StructuralTraversal::SkipChildren;
        }
        if self.arrow_depth == 0
            && let Expr::Call(call) = node
            && let Expr::Field(field) = call.callee().unparenthesized()
            && field.field().name() == "then"
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-async-await",
                "Prefer async/await over .then() chains",
                call.span(),
            ));
        }
        StructuralTraversal::Continue
    }
}
}
