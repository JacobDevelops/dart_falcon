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

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_function_body, walk_function_decl, walk_getter_decl,
    walk_method_decl, walk_stmt,
};

#[derive(Default)]
pub struct AvoidVoidAsync;

impl Rule for AvoidVoidAsync {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        for declaration in &program.declarations {
            if let TopLevelDecl::Function(function) = declaration
                && function.name.name == "main"
            {
                // The entry point itself is exempt; its body is still inspected.
                if let Some(body) = &function.body {
                    walk_function_body(&mut collector, body);
                }
                continue;
            }
            collector.visit_top_level_decl(declaration);
        }
        RulePreparation::Diagnostics(collector.diagnostics)
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

impl Visitor for Collector<'_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.check(node.async_marker, node.return_type.as_ref(), node.name.span);
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check(node.async_marker, node.return_type.as_ref(), node.name.span);
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.check(node.async_marker, node.return_type.as_ref(), node.name.span);
        walk_getter_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
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
        walk_class_member(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node {
            self.check(
                function.async_marker,
                function.return_type.as_ref(),
                function.name.span,
            );
        }
        walk_stmt(self, node);
    }
}
