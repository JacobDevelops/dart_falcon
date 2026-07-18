//! Flags old-style function-typed parameters (`use-function-type-syntax-for-parameters`,
//! adopted from package:lints): `void f(int g(int x))` should be written with
//! the generic function type syntax `void f(int Function(int) g)`.
//!
//! The parser records the old form as a [`FormalParam`] carrying nested
//! `function_params`; the modern form parses as a plain parameter whose type is
//! a `DartType::Function`. So a non-empty `function_params` is exactly the
//! old-style signature.

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
