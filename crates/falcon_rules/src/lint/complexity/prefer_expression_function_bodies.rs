//! Flags block function bodies that consist of a single `return` of a value.
//!
//! A body like `{ return expr; }` reads better as `=> expr`. Every function
//! body is checked: top-level functions, methods, getters, operators, local
//! functions, and closures. Diagnostic ranges include an `async`/generator modifier.
//!
//! ```dart
//! int get width { return _width; } // flagged
//! int get width => _width;         // ok
//! ```
//!
//! Adopted from the official `prefer_expression_function_bodies` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;

use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferExpressionFunctionBodies;

impl Rule for PreferExpressionFunctionBodies {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check(&mut self, body: Option<FunctionBody<'_>>, async_marker: AsyncMarker) {
        if let Some(FunctionBody::Block(block)) = body
            && block.stmts().len() == 1
            && matches!(block.stmts().first(), Some(Stmt::Return(ret)) if ret.value().is_some())
        {
            // The analyzer's function body includes its async/generator modifier.
            let mut preceding = self
                .ctx
                .tree()
                .tokens_in(self.ctx.source, Span::new(0, block.span().start))
                .rev()
                .filter(|token| !token.is_trivia());
            let mut start = block.span().start;
            if async_marker != AsyncMarker::Sync
                && let Some(token) = preceding.next()
            {
                use falcon_syntax::TokenKind;
                let marker = if token.kind() == TokenKind::Star {
                    preceding.next()
                } else {
                    Some(token)
                };
                if let Some(marker) = marker
                    && matches!(marker.kind(), TokenKind::Async | TokenKind::Sync)
                {
                    start = marker.offset();
                }
            }
            self.diags.push(self.ctx.diagnostic(
                "prefer-expression-function-bodies",
                "Unnecessary use of a block function body.",
                Span::new(start, block.span().end),
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        self.check(node.body(), node.async_marker());
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        let (body, marker) = match node {
            ClassMember::Method(member) => (member.body(), member.async_marker()),
            ClassMember::Getter(member) => (member.body(), member.async_marker()),
            ClassMember::Setter(member) => (member.body(), AsyncMarker::Sync),
            ClassMember::Operator(member) => (member.body(), member.async_marker()),
            ClassMember::Constructor(member) => (member.body(), AsyncMarker::Sync),
            ClassMember::PrimaryConstructorBody(member) => {
                (member.body(), AsyncMarker::Sync)
            }
            ClassMember::Field(_) | ClassMember::Error(_) => (None, AsyncMarker::Sync),
        };
        self.check(body, marker);
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check(Some(function.body()), function.async_marker());
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node
        {
            self.check(Some(function.body()), function.async_marker());
        }
        StructuralTraversal::Continue
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn body_ranges_include_async_modifiers_and_comments() {
        let source = "int f() { return 1; } Future<int> g() async /* body */ { return 2; } void h() { final cb = () async { return 3; }; }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics = PreferExpressionFunctionBodies.analyze(parsed.program(), &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            actual,
            [
                "{ return 1; }",
                "async /* body */ { return 2; }",
                "async { return 3; }"
            ]
        );
    }

    #[test]
    fn contextual_getter_names_are_not_body_modifiers() {
        let source = "class C { int get async { return 1; } int get sync { return 2; } }";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics = PreferExpressionFunctionBodies.analyze(parsed.program(), &ctx);
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(actual, ["{ return 1; }", "{ return 2; }"]);
    }
}
