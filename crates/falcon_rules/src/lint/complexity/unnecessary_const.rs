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

pub struct UnnecessaryConst;

impl Rule for UnnecessaryConst {
    fn name(&self) -> &'static str {
        "unnecessary-const"
    }

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

/// An explicit `const` keyword produces one of these nodes with `is_const == true`
/// (implicit/inferred constants leave the flag false), so their span marks the keyword.
fn const_marker_span(expr: &Expr) -> Option<&Span> {
    match expr {
        Expr::New { is_const: true, .. }
        | Expr::List { is_const: true, .. }
        | Expr::Map { is_const: true, .. }
        | Expr::Set { is_const: true, .. }
        | Expr::DotShorthand { is_const: true, .. } => Some(expr.span()),
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
        if let Some(span) = const_marker_span(node) {
            if self.const_depth > 0 {
                self.diags.push(self.ctx.diagnostic(
                    "unnecessary-const",
                    "Unnecessary `const` keyword inside an already-const context.",
                    *span,
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
