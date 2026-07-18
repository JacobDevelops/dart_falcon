//! Flags an old-style function-typed parameter written with inline parameter syntax.
//!
//! Dart lets you declare a function-typed parameter two ways. The legacy form
//! inlines the callee's own parameter list — `void f(int g(int x))` — while the
//! modern generic function type syntax writes the type up front:
//! `void f(int Function(int) g)`. The modern form is preferred because it reads
//! as an ordinary typed parameter, composes with nullability (`Function(int)?`),
//! and keeps the parameter name in its usual place. The rule detects the old form
//! by the nested parameter list the parser attaches to it; the modern form parses
//! as a plain parameter with a function type and is not flagged. Rewrite using
//! `Function` type syntax.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};

pub struct UseFunctionTypeSyntaxForParameters;

impl Rule for UseFunctionTypeSyntaxForParameters {
    fn name(&self) -> &'static str {
        "use-function-type-syntax-for-parameters"
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
    fn visit_formal_param(&mut self, node: &FormalParam) {
        if node.function_params.is_some() {
            self.diags.push(Diagnostic::new(
                "use-function-type-syntax-for-parameters",
                Severity::Warning,
                "Use the generic function type syntax for parameters.".to_string(),
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: node.name.span.start,
                    end: node.name.span.end,
                },
            ));
        }
        visitor::walk_formal_param(self, node);
    }
}
