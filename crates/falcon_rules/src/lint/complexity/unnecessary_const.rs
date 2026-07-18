//! Flags a `const` keyword used inside an already-const context. Ported from package:lints
//! `unnecessary_const`. Inside a const collection, a const constructor invocation's arguments,
//! a const variable initializer, or another const expression, nested `const` keywords are
//! redundant and can be removed.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_field_decl, walk_program, walk_stmt, walk_top_level_decl,
};

pub struct UnnecessaryConst;

impl Rule for UnnecessaryConst {
    fn name(&self) -> &'static str {
        "unnecessary-const"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
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

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
    const_depth: usize,
}

impl Visitor for Collector {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node
            && v.is_const
        {
            self.const_depth += 1;
            walk_top_level_decl(self, node);
            self.const_depth -= 1;
            return;
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.is_const {
            self.const_depth += 1;
            walk_field_decl(self, node);
            self.const_depth -= 1;
            return;
        }
        walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(lv) = node
            && lv.is_const
        {
            self.const_depth += 1;
            walk_stmt(self, node);
            self.const_depth -= 1;
            return;
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Some(span) = const_marker_span(node) {
            if self.const_depth > 0 {
                self.diags.push(Diagnostic::new(
                    "unnecessary-const",
                    Severity::Warning,
                    "Unnecessary `const` keyword inside an already-const context.",
                    self.file.clone(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
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
