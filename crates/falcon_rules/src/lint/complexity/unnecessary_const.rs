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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_field_decl, walk_local_var, walk_top_level_decl,
};

#[derive(Default)]
pub struct UnnecessaryConst;

impl Rule for UnnecessaryConst {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            const_depth: 0,
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// An explicit `const` expression starts at the redundant keyword.
fn const_marker_span(expr: &Expr) -> Option<Span> {
    match expr.unparenthesized() {
        Expr::New { is_const: true, .. }
        | Expr::List { is_const: true, .. }
        | Expr::Map { is_const: true, .. }
        | Expr::Set { is_const: true, .. }
        | Expr::DotShorthand { is_const: true, .. } => {
            let start = expr.unparenthesized().span().start;
            Some(Span::new(start, start + "const".len()))
        }
        _ => None,
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    const_depth: usize,
}

impl Visitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node
            && v.keyword == falcon_syntax::ast::VarKeyword::Const
        {
            self.const_depth += 1;
            walk_top_level_decl(self, node);
            self.const_depth -= 1;
            return;
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.keyword == falcon_syntax::ast::VarKeyword::Const {
            self.const_depth += 1;
            walk_field_decl(self, node);
            self.const_depth -= 1;
            return;
        }
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, lv: &LocalVarDecl) {
        if lv.keyword == falcon_syntax::ast::VarKeyword::Const {
            self.const_depth += 1;
            walk_local_var(self, lv);
            self.const_depth -= 1;
            return;
        }
        walk_local_var(self, lv);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::Paren { .. }) {
            falcon_syntax::visitor::walk_expr(self, node);
            return;
        }
        if let Some(span) = const_marker_span(node) {
            if self.const_depth > 0 {
                self.diags.push(self.ctx.diagnostic(
                    "unnecessary-const",
                    "Unnecessary `const` keyword inside an already-const context.",
                    span,
                ));
            }
            self.const_depth += 1;
            walk_expr(self, node);
            self.const_depth -= 1;
        } else {
            walk_expr(self, node);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn reports_only_the_redundant_keyword() {
        let source = "const values = const [const <int>[1], const Duration(seconds: 1)];";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
        let diagnostics = UnnecessaryConst.analyze(&parsed.program, &ctx);
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
