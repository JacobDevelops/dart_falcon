//! Avoid `async` functions and methods declared to return `void`.
//!
//! Callers cannot await a `void` result, so they cannot know when the work has
//! finished or observe its errors. Return `Future<void>` instead:
//!
//! ```dart
//! void save() async {} // BAD
//! Future<void> save() async {} // GOOD
//! ```
//!
//! A top-level `main` and `async*` generators are exempt. Functions without a
//! written return type are skipped: an override may inherit `void`, which only
//! full type resolution can see.
//!
//! Adopted from the official `avoid_void_async` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidVoidAsync;

impl Rule for AvoidVoidAsync {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check(&mut self, async_marker: AsyncMarker, return_type: Option<&DartType>, name: Span) {
        if async_marker == AsyncMarker::Async && matches!(return_type, Some(DartType::Void { .. }))
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-void-async",
                "An 'async' function should have a 'Future' return type when it doesn't return a value.",
                name,
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        if let StructuralNode::FunctionDecl(function) = parent
            && function.name.name == "main"
        {
            return matches!(child, StructuralNode::FunctionBody(_));
        }
        true
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        if node.name.name != "main" {
            self.check(node.async_marker, node.return_type.as_ref(), node.name.span);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.check(node.async_marker, node.return_type.as_ref(), node.name.span);
        StructuralTraversal::Continue
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) -> StructuralTraversal {
        self.check(node.async_marker, node.return_type.as_ref(), node.name.span);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            // Report at the `operator` keyword, standing in for the name token.
            let span = self
                .ctx
                .syntax
                .keywords_in(
                    Span::new(operator.span.start, operator.params.span.start),
                    falcon_syntax::TokenKind::Operator,
                )
                .next()
                .map_or(operator.span, |token| {
                    Span::new(token.offset(), token.offset() + token.len())
                });
            self.check(operator.async_marker, operator.return_type.as_ref(), span);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check(
                function.async_marker,
                function.return_type.as_ref(),
                function.name.span,
            );
        }
        StructuralTraversal::Continue
    }
}
}
