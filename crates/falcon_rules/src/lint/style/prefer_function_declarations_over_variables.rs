//! Flags local variables bound to a function literal that should instead be a
//! local function declaration (`prefer-function-declarations-over-variables`,
//! adopted from package:lints).
//!
//! ponytail: reassignment analysis is intentionally skipped — only `final`/
//! `const` local bindings (which provably can never be reassigned) are flagged.
//! A plain `var f = () {...}` never reassigned would also qualify upstream, but
//! proving non-reassignment needs dataflow this syntax-only pass does not do, so
//! those are left alone to stay conservative.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};

pub struct PreferFunctionDeclarationsOverVariables;

impl Rule for PreferFunctionDeclarationsOverVariables {
    fn name(&self) -> &'static str {
        "prefer-function-declarations-over-variables"
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

impl Visitor for Collector<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(local) = node
            && (local.is_final || local.is_const)
        {
            for decl in &local.declarators {
                if let Some(Expr::FuncExpr { .. }) = &decl.initializer {
                    self.diags.push(Diagnostic::new(
                        "prefer-function-declarations-over-variables",
                        Severity::Warning,
                        "Use a function declaration to bind a function to a name.".to_string(),
                        self.ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: decl.name.span.start,
                            end: decl.name.span.end,
                        },
                    ));
                }
            }
        }
        visitor::walk_stmt(self, node);
    }
}
