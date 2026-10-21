//! Disallow nullable return types on functions that never return null.
//!
//! Flags a function or method whose return type is nullable (`T?`) even though
//! every `return` in its body yields a provably non-null value. A needless `?`
//! forces callers to null-check a result that can never be null, spreading
//! defensive code that adds no safety. Drop the `?` from the return type. Only
//! the outer nullability counts, so `Future<T?>` and `List<int?>` are left
//! alone. Without full type resolution the check is conservative.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryNullableReturnType;

impl Rule for UnnecessaryNullableReturnType {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            diags: Vec::new(),
            candidate: None,
            bodies: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    candidate: Option<(Span, Span)>,
    bodies: Vec<ReturnState>,
    ctx: &'a AnalyzeContext<'ctx>,
}

struct ReturnState {
    candidate: Option<Span>,
    count: usize,
    all_non_null: bool,
}

impl Collector<'_, '_> {
    fn candidate(&mut self, return_type: Option<DartType<'_>>, body: Option<FunctionBody<'_>>) {
        if let Some(return_type) = return_type
            && return_type.is_nullable()
            && let Some(body) = body
        {
            self.candidate = Some((body.span(), return_type.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        self.candidate(node.return_type(), node.body());
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.candidate(node.return_type(), node.body());
        StructuralTraversal::Continue
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) -> StructuralTraversal {
        self.candidate(node.return_type(), node.body());
        StructuralTraversal::Continue
    }

    fn visit_function_body(&mut self, body: FunctionBody<'a>) -> StructuralTraversal {
        let candidate = if self.candidate.is_some_and(|(span, _)| span == body.span()) {
            self.candidate.take().map(|(_, span)| span)
        } else {
            None
        };
        let (count, all_non_null) = match body {
            FunctionBody::Arrow(body) => (1, is_provably_non_null(body.expr())),
            FunctionBody::Block(_) => (0, true),
            FunctionBody::Native(_) => (0, false),
        };
        self.bodies.push(ReturnState { candidate, count, all_non_null });
        StructuralTraversal::Continue
    }

    fn visit_return_stmt(&mut self, node: ReturnStmt<'a>) -> StructuralTraversal {
        if let Some(state) = self.bodies.last_mut() {
            state.count += 1;
            state.all_non_null &= node.value().is_some_and(is_provably_non_null);
        }
        StructuralTraversal::Continue
    }

    fn exit_function_body(&mut self, _body: FunctionBody<'a>) {
        let state = self.bodies.pop().unwrap();
        if state.count > 0 && state.all_non_null
            && let Some(span) = state.candidate
        {
            self.diags.push(self.ctx.diagnostic(
                "Function return type is unnecessarily nullable",
                span,
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

use crate::lint::expressions::is_provably_non_null;
