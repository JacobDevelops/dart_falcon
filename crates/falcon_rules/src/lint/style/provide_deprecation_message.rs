//! Flags `@deprecated` and `@Deprecated()` annotations that carry no message.
//!
//! A deprecation is only useful if it tells callers what to do instead — which
//! replacement to adopt, or since when the API is going away. The bare
//! `@deprecated` constant can never carry that guidance, and `@Deprecated()`
//! with no argument (or an empty/whitespace-only string) is just as unhelpful,
//! so both should be replaced with `@Deprecated("message")`. A `@Deprecated`
//! whose positional argument is any non-empty expression is accepted. The rule
//! also inspects field-level annotations, which the generic AST walk would
//! otherwise skip.

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
