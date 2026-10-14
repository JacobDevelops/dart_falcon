//! Flags parameters whose names resolve to existing types.

use falcon_analyze::{AnalyzeContext, NameIdentity, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_formal_param};

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
            ctx,
            identities,
            file_path: ctx.file_path,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    identities: &'a falcon_analyze::IdentityIndex,
    file_path: &'a std::path::Path,
}

impl Collector<'_> {
    fn resolves_to_type(&self, name: &str) -> bool {
        self.identities.resolve(self.file_path, &[name.to_string()]) == NameIdentity::Type
    }
}

impl Visitor for Collector<'_> {
    fn visit_formal_param(&mut self, node: &FormalParam) {
        if self.resolves_to_type(&node.name.name) {
            self.diags.push(self.ctx.diagnostic(
                "avoid-types-as-parameter-names",
                "Avoid using an existing type as a parameter name.",
                node.name.span,
            ));
        }
        walk_formal_param(self, node);
    }
}
