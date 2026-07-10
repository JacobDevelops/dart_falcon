use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_pattern};

pub struct AvoidRedundantPatternFieldNames;

impl Rule for AvoidRedundantPatternFieldNames {
    fn name(&self) -> &'static str {
        "avoid_redundant_pattern_field_names"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            source: ctx.source,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    file: String,
    source: &'s str,
}

impl Collector<'_> {
    fn flag(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "avoid_redundant_pattern_field_names",
            Severity::Warning,
            "Redundant field name in pattern; use the shorthand (e.g. `:x`).",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector<'_> {
    fn visit_pattern(&mut self, node: &Pattern) {
        match node {
            Pattern::Object(obj) => {
                for field in &obj.fields {
                    if is_redundant_field_source(self.source, &field.span) {
                        self.flag(&field.span);
                    }
                }
            }
            Pattern::Record(rec) => {
                for field in &rec.fields {
                    // A record shorthand `(:x)` parses with `name: None`; only an
                    // explicitly named field can be redundant.
                    if field.name.is_some() && is_redundant_field_source(self.source, &field.span) {
                        self.flag(&field.span);
                    }
                }
            }
            _ => {}
        }
        walk_pattern(self, node);
    }
}

/// True when a pattern field's source reads `name: name` with equal identifiers
/// on both sides (e.g. `x: x`). The colon shorthand `:x` has an empty left side
/// and is intentionally excluded, as is anything with a nested/typed pattern on
/// the right (`x: var x`, `x: Foo(...)`).
fn is_redundant_field_source(source: &str, span: &Span) -> bool {
    let text = &source[span.start..span.end];
    let Some(idx) = text.find(':') else {
        return false;
    };
    let left = text[..idx].trim();
    let right = text[idx + 1..].trim();
    !left.is_empty() && left == right && is_ident(left)
}

fn is_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}
