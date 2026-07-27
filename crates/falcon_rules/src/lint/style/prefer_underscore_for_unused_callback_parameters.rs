//! Flags unused closure parameters that are not named with underscores.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::lexical_usage::used_parameters;

pub struct PreferUnderscoreForUnusedCallbackParameters;

impl Rule for PreferUnderscoreForUnusedCallbackParameters {
    fn name(&self) -> &'static str {
        "prefer-underscore-for-unused-callback-parameters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector {
    diagnostics: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr {
            params, body, span, ..
        } = node
        {
            let used = used_parameters(params, body);
            if params
                .positional
                .iter()
                .chain(&params.optional_positional)
                .chain(&params.named)
                .any(|param| {
                    let name = &param.name.name;
                    !(name.is_empty()
                        || name.bytes().all(|byte| byte == b'_')
                        || used.contains(&param.name.span.start))
                })
            {
                self.diagnostics.push(Diagnostic::new(
                    "prefer-underscore-for-unused-callback-parameters",
                    Severity::Warning,
                    "Unused callback parameter should be named '_'.",
                    self.file.clone(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
        }
        walk_expr(self, node);
    }
}
