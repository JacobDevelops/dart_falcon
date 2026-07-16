//! Flags `@deprecated` / `@Deprecated()` annotations that carry no message.
//! Ported from package:lints `provide_deprecation_message`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;

pub struct ProvideDeprecationMessage;

impl Rule for ProvideDeprecationMessage {
    fn name(&self) -> &'static str {
        "provide-deprecation-message"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_annotation(&mut self, node: &Annotation) {
        let Some(last) = node.name.last() else {
            return;
        };
        let flagged = match last.name.as_str() {
            // `@deprecated`: the predefined constant, never carries a message.
            "deprecated" => node.args.is_none(),
            // `@Deprecated(...)`: flag a missing argument, or a single positional
            // message that is an empty/whitespace-only string literal.
            "Deprecated" => match &node.args {
                None => true,
                Some(args) => match args.positional.first() {
                    None => true,
                    Some(Expr::StringLit(s)) => s.value.trim().is_empty(),
                    Some(_) => false,
                },
            },
            _ => false,
        };
        if flagged {
            self.diags.push(Diagnostic::new(
                "provide-deprecation-message",
                Severity::Warning,
                "Provide a deprecation message, via @Deprecated(\"message\").",
                self.file.clone(),
                DiagSpan {
                    start: node.span.start,
                    end: node.span.end,
                },
            ));
        }
    }
}
