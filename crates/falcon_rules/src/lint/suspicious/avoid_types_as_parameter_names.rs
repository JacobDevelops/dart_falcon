//! Flags parameters whose names resolve to existing types.

use falcon_analyze::{AnalyzeContext, IdentityIndex, LibraryId, NameIdentity, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_formal_param};

#[derive(Default)]
pub struct AvoidTypesAsParameterNames;

impl Rule for AvoidTypesAsParameterNames {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some((identities, file)) = ctx
            .identities
            .and_then(|identities| Some((identities, identities.file(ctx.file_path)?)))
        else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            identities,
            library: identities.library(file),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    identities: &'a IdentityIndex,
    library: LibraryId,
}

impl Collector<'_> {
    fn resolves_to_type(&self, name: &str) -> bool {
        self.identities.resolve(self.library, name.into()) == NameIdentity::Type
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
