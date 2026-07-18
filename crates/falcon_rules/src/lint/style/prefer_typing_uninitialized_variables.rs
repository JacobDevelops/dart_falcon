//! Flags uninitialized variables declared without a type annotation.
//!
//! When a declaration has no initializer, there is no value for the analyzer to
//! infer a type from, so `var x;` leaves the variable as `dynamic` — silently
//! opting out of static checking on every later use. Writing `int x;` instead
//! documents intent and restores type safety at no cost. The rule fires only on
//! declarators that are both untyped and uninitialized; a declaration is exempt
//! as soon as it carries a type annotation. It covers local variables, fields,
//! and top-level variables.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};

pub struct PreferTypingUninitializedVariables;

impl Rule for PreferTypingUninitializedVariables {
    fn name(&self) -> &'static str {
        "prefer-typing-uninitialized-variables"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'c> {
    ctx: &'a AnalyzeContext<'c>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check(&mut self, has_type: bool, declarators: &[VarDeclarator]) {
        if has_type {
            return;
        }
        for decl in declarators {
            if decl.initializer.is_none() {
                self.diags.push(Diagnostic::new(
                    "prefer-typing-uninitialized-variables",
                    Severity::Warning,
                    "Prefer typing uninitialized variables and fields.".to_string(),
                    self.ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: decl.name.span.start,
                        end: decl.name.span.end,
                    },
                ));
            }
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_program(&mut self, node: &Program) {
        for decl in &node.declarations {
            if let TopLevelDecl::Variable(var) = decl {
                self.check(var.var_type.is_some(), &var.declarators);
            }
        }
        visitor::walk_program(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check(node.field_type.is_some(), &node.declarators);
        visitor::walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(local) = node {
            self.check(local.var_type.is_some(), &local.declarators);
        }
        visitor::walk_stmt(self, node);
    }
}
