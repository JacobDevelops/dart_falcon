//! Flags `return null;` (and `=> null` bodies) in synchronous `void`
//! functions and asynchronous `Future<void>` functions. A nullable future
//! permits a null result only in a synchronous body; `void async` is exempt.
//!
//! Returning an explicit `null` from a void function is contradictory: the
//! signature promises no value, so the `null` is dead and usually reflects
//! confusion about the function's contract or a copy-paste from a nullable one.
//! Drop the value and write a bare `return;`, or change the return type if the
//! function is really meant to produce a value. Each nested function is judged by
//! its own return type, so an inner closure is not tainted by an enclosing void
//! frame.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidReturningNullForVoid;

impl Rule for AvoidReturningNullForVoid {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            void_stack: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    /// Void-ness of each enclosing function boundary. The innermost frame decides
    /// whether a `return null;` is flagged; a nested function pushes its own frame
    /// so it is judged by its own return type.
    void_stack: Vec<bool>,
}

impl Collector<'_> {
    /// An arrow body `=> null` of a void function is the expression form of
    /// `return null;`, so check it at the function boundary.
    fn check_arrow(&mut self, body: Option<FunctionBody<'_>>, is_void: bool) {
        if !is_void {
            return;
        }
        if let Some(FunctionBody::Arrow(arrow)) = body
            && matches!(arrow.expr(), Expr::NullLit(_))
        {
            self.diags.push(self.ctx.diagnostic("Don't return null from a function with a void return type.", arrow.expr().span()));
        }
    }
}

fn is_void_return(rt: Option<DartType<'_>>, marker: AsyncMarker) -> bool {
    match (rt, marker) {
        (Some(DartType::Void(_)), AsyncMarker::Sync) => true,
        (Some(DartType::Named(n)), AsyncMarker::Async) => {
            let mut args = n.type_args();
            n.segments().last().is_some_and(|s| s.name() == "Future")
                && matches!(args.next(), Some(DartType::Void(_)))
                && args.next().is_none()
        }
        _ => false,
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            let is_void = is_void_return(operator.return_type(), operator.async_marker());
            self.check_arrow(operator.body(), is_void);
            self.void_stack.push(is_void);
        }
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, node: ClassMember<'a>) {
        if matches!(node, ClassMember::Operator(_)) {
            self.void_stack.pop();
        }
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        let is_void = is_void_return(node.return_type(), node.async_marker());
        self.check_arrow(node.body(), is_void);
        self.void_stack.push(is_void);
        StructuralTraversal::Continue
    }

    fn exit_function_decl(&mut self, _node: FunctionDecl<'a>) {
        self.void_stack.pop();
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        let is_void = is_void_return(node.return_type(), node.async_marker());
        self.check_arrow(node.body(), is_void);
        self.void_stack.push(is_void);
        StructuralTraversal::Continue
    }

    fn exit_method_decl(&mut self, _node: MethodDecl<'a>) {
        self.void_stack.pop();
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) -> StructuralTraversal {
        let is_void = is_void_return(node.return_type(), node.async_marker());
        self.check_arrow(node.body(), is_void);
        self.void_stack.push(is_void);
        StructuralTraversal::Continue
    }

    fn exit_getter_decl(&mut self, _node: GetterDecl<'a>) {
        self.void_stack.pop();
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        // Setters are always void-returning.
        self.check_arrow(node.body(), true);
        self.void_stack.push(true);
        StructuralTraversal::Continue
    }

    fn exit_setter_decl(&mut self, _node: SetterDecl<'a>) {
        self.void_stack.pop();
    }

    fn visit_constructor_decl(&mut self, _node: ConstructorDecl<'a>) -> StructuralTraversal {
        // Constructors don't return values; never flag inside one.
        self.void_stack.push(false);
        StructuralTraversal::Continue
    }

    fn exit_constructor_decl(&mut self, _node: ConstructorDecl<'a>) {
        self.void_stack.pop();
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        match node {
            Stmt::Return(r) => {
                if self.void_stack.last() == Some(&true)
                    && matches!(r.value(), Some(Expr::NullLit(_)))
                {
                    self.diags.push(self.ctx.diagnostic(
                        "Don't return null from a function with a void return type.",
                        r.span(),
                    ));
                }
            }
            Stmt::LocalFunc(lf) => {
                let is_void = is_void_return(lf.return_type(), lf.async_marker());
                self.check_arrow(Some(lf.body()), is_void);
                self.void_stack.push(is_void);
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if matches!(node, Stmt::LocalFunc(_)) {
            self.void_stack.pop();
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            // Function expressions have no written return type, so they can never
            // be declared `void`.
            self.check_arrow(Some(function.body()), false);
            self.void_stack.push(false);
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) {
            self.void_stack.pop();
        }
    }
}
}
