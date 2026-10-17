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
use falcon_syntax::cst::{self, AsyncMarker};

#[derive(Default)]
pub struct AvoidVoidAsync;

impl Rule for AvoidVoidAsync {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
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
    fn check(
        &mut self,
        async_marker: AsyncMarker,
        return_type: Option<cst::DartType<'_>>,
        name: cst::Span,
    ) {
        if async_marker == AsyncMarker::Async && matches!(return_type, Some(cst::DartType::Void(_)))
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
impl<'a> StructuralRuleVisitor<'a> for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        if let StructuralNode::FunctionDecl(function) = parent
            && function.name().name() == "main"
        {
            return matches!(child, StructuralNode::FunctionBody(_));
        }
        true
    }

    fn visit_function_decl(&mut self, node: cst::FunctionDecl<'_>) -> StructuralTraversal {
        let name = node.name();
        if name.name() != "main" {
            self.check(node.async_marker(), node.return_type(), name.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: cst::MethodDecl<'_>) -> StructuralTraversal {
        self.check(node.async_marker(), node.return_type(), node.name().span());
        StructuralTraversal::Continue
    }

    fn visit_getter_decl(&mut self, node: cst::GetterDecl<'_>) -> StructuralTraversal {
        self.check(node.async_marker(), node.return_type(), node.name().span());
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: cst::ClassMember<'_>) -> StructuralTraversal {
        if let cst::ClassMember::Operator(operator) = node {
            let keyword_span = self
                .ctx
                .tree()
                .tokens_in(self.ctx.source, operator.span())
                .find(|token| token.kind() == falcon_syntax::TokenKind::Operator)
                .map_or(operator.span(), |token| token.span());
            self.check(
                operator.async_marker(),
                operator.return_type(),
                keyword_span,
            );
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: cst::Stmt<'_>) -> StructuralTraversal {
        if let cst::Stmt::LocalFunc(function) = node {
            self.check(function.async_marker(), function.return_type(), function.name().span());
        }
        StructuralTraversal::Continue
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{ParsedFile, RuleAnalysis};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;
    use std::sync::Arc;

    #[test]
    fn operator_diagnostic_points_to_keyword() {
        let source = "class C { void operator []=(int i, int v) async {} }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let context = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);

        let diagnostics = AvoidVoidAsync.analyze(parsed.program(), &context);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        let span = diagnostics[0].span;
        assert_eq!(&source[span.start..span.end], "operator");
    }
}
