//! Flags use of the `late` keyword on variables and fields.
//!
//! `late` defers initialization and moves the "is it set?" check from compile
//! time to runtime: reading a `late` variable before it is assigned throws a
//! `LateInitializationError` rather than failing to compile. Preferring eagerly
//! initialized or nullable declarations keeps that guarantee static and the
//! failure mode visible. Top-level variables, instance fields, and local
//! declarations inside any function body are all checked.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_field_decl, walk_stmt, walk_top_level_decl};

pub struct AvoidLateKeyword;

impl Rule for AvoidLateKeyword {
    fn name(&self) -> &'static str {
        "avoid-late-keyword"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(var) = node
            && var.is_late
        {
            self.diags.push(make_diag(self.ctx, &var.span));
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.is_late {
            self.diags.push(make_diag(self.ctx, &node.span));
        }
        walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(local) = node
            && local.is_late
        {
            self.diags.push(make_diag(self.ctx, &local.span));
        }
        walk_stmt(self, node);
    }
}

fn make_diag(ctx: &AnalyzeContext, span: &Span) -> Diagnostic {
    Diagnostic::new(
        "avoid-late-keyword",
        Severity::Warning,
        "Avoid using the late keyword — use nullable types or initialize immediately instead",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    )
}
