//! Flags a variable declaration that declares more than one variable.
//!
//! Declaring each variable on its own line makes each one easier to find,
//! document, and change. Top-level variables, fields, and local variables are
//! checked; the second declared name is reported:
//!
//! ```dart
//! var a = 1, b = 2;            // `b` flagged
//! int x, y;                    // `y` flagged
//! for (var i = 0, j = 1; ;) {} // fine: loop initializers are exempt
//! ```
//!
//! Adopted from the official `avoid_multiple_declarations_per_line` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_field_decl, walk_stmt, walk_top_level_decl};

#[derive(Default)]
pub struct AvoidMultipleDeclarationsPerLine;

impl Rule for AvoidMultipleDeclarationsPerLine {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check(&mut self, declarators: &[VarDeclarator]) {
        if let Some(second) = declarators.get(1) {
            self.diags.push(self.ctx.diagnostic(
                "avoid-multiple-declarations-per-line",
                "Multiple variables declared on a single line.",
                second.name.span,
            ));
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.check(&variable.declarators);
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check(&node.declarators);
        walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(local) = node {
            self.check(&local.declarators);
        }
        walk_stmt(self, node);
    }
}
