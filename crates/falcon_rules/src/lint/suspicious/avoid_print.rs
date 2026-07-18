//! Flags calls to the top-level `print` function.
//!
//! `print` writes to the system console, which is invisible in released builds
//! and can leak diagnostic detail or stall the UI isolate when it runs on a hot
//! path. In application code a surviving `print` is almost always a debugging
//! statement that was never removed. Route diagnostics through `debugPrint`,
//! `dart:developer`'s `log`, or a real logging framework instead. A file that
//! declares its own `print` (top-level, method, or local function) is left
//! alone, since the call may resolve to that shadow rather than the SDK
//! function.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct AvoidPrint;

impl Rule for AvoidPrint {
    fn name(&self) -> &'static str {
        "avoid-print"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // If the file syntactically declares its own `print` (top-level, method,
        // or local function), a bare `print(...)` may resolve to that shadow, so
        // we stay silent to avoid a false positive.
        let mut scan = DeclScan { has_print: false };
        scan.visit_program(program);
        if scan.has_print {
            return Vec::new();
        }

        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct DeclScan {
    has_print: bool,
}

impl Visitor for DeclScan {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if node.name.name == "print" {
            self.has_print = true;
        }
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        if node.name.name == "print" {
            self.has_print = true;
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(f) = node
            && f.name.name == "print"
        {
            self.has_print = true;
        }
        falcon_syntax::visitor::walk_stmt(self, node);
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call { callee, span, .. } = node
            && let Expr::Ident(id) = callee.as_ref()
            && id.name == "print"
        {
            self.diags.push(Diagnostic::new(
                "avoid-print",
                Severity::Warning,
                "Avoid using print in production code; use a logging framework instead.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}
