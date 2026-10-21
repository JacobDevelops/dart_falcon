//! Flags a redundant `const` keyword inside an already-const context.
//!
//! Once an expression sits in a constant context — a const collection, the
//! arguments of a const constructor invocation, a const variable initializer,
//! or another const expression — every nested `const` is inferred and the
//! keyword is just noise; removing it leaves behavior unchanged. The rule
//! tracks const depth as it descends through const variable, field, and local
//! declarations and const expressions, flagging any explicit `const` marker
//! (on a `new`, list, map, set, or dot-shorthand) found at depth greater than
//! zero.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryConst;

impl Rule for UnnecessaryConst {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            const_depth: 0,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

/// An explicit `const` expression starts at the redundant keyword.
fn const_marker_span(expr: Expr<'_>) -> Option<Span> {
    let inner = expr.unparenthesized();
    let is_const = match inner {
        Expr::New(node) => node.is_const(),
        Expr::List(node) => node.is_const(),
        Expr::Map(node) => node.is_const(),
        Expr::Set(node) => node.is_const(),
        Expr::DotShorthand(node) => node.is_const(),
        _ => false,
    };
    is_const.then(|| {
        let start = inner.span().start;
        Span::new(start, start + "const".len())
    })
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    const_depth: usize,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if matches!(node, TopLevelDecl::Variable(v) if v.keyword() == VarKeyword::Const) {
            self.const_depth += 1;
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if matches!(node, TopLevelDecl::Variable(v) if v.keyword() == VarKeyword::Const) {
            self.const_depth -= 1;
        }
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) -> StructuralTraversal {
        self.const_depth += usize::from(node.keyword() == VarKeyword::Const);
        StructuralTraversal::Continue
    }

    fn exit_field_decl(&mut self, node: FieldDecl<'a>) {
        self.const_depth -= usize::from(node.keyword() == VarKeyword::Const);
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        self.const_depth += usize::from(node.keyword() == VarKeyword::Const);
        StructuralTraversal::Continue
    }

    fn exit_local_var(&mut self, node: LocalVarDecl<'a>) {
        self.const_depth -= usize::from(node.keyword() == VarKeyword::Const);
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if !matches!(node, Expr::Paren(_))
            && let Some(span) = const_marker_span(node)
        {
            if self.const_depth > 0 {
                self.diags.push(self.ctx.diagnostic(
                    "Unnecessary `const` keyword inside an already-const context.",
                    span,
                ));
            }
            self.const_depth += 1;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if !matches!(node, Expr::Paren(_)) && const_marker_span(node).is_some() {
            self.const_depth -= 1;
        }
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
    fn reports_only_the_redundant_keyword() {
        let source = "const values = const [const <int>[1], const Duration(seconds: 1)];";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        });
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), &config, &parsed);
        let diagnostics = UnnecessaryConst.analyze(UnnecessaryConst::NAME, parsed.program(), &ctx);
        let starts: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.span.start)
            .collect();
        let expected: Vec<_> = source
            .match_indices("const")
            .skip(1)
            .map(|(start, _)| start)
            .collect();
        assert_eq!(starts, expected);
        for diagnostic in diagnostics {
            assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "const");
        }
    }
}
