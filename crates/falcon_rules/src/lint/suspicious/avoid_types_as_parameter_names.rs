//! Flags parameters whose names resolve to existing types.

use falcon_analyze::{AnalyzeContext, NameIdentity, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_formal_param, walk_program};

pub struct AvoidTypesAsParameterNames;

impl Rule for AvoidTypesAsParameterNames {
    fn name(&self) -> &'static str {
        "avoid-types-as-parameter-names"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            identities,
            file_path: ctx.file_path,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    file: String,
    identities: &'a falcon_analyze::IdentityIndex,
    file_path: &'a std::path::Path,
}

impl Collector<'_> {
    fn resolves_to_type(&self, name: &str) -> bool {
        self.identities.resolve(self.file_path, &[name.to_string()]) == NameIdentity::Type
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if self.resolves_to_type(&node.name.name) {
            self.diags.push(Diagnostic::new(
                "avoid-types-as-parameter-names",
                Severity::Warning,
                "Avoid using an existing type as a parameter name.",
                self.file.clone(),
                DiagSpan {
                    start: node.name.span.start,
                    end: node.name.span.end,
                },
            ));
        }
        walk_formal_param(self, node);
    }
}
