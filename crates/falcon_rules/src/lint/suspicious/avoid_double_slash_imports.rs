//! Reject repeated path separators in directive URIs (import, export and part).

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

use crate::lint::support::directive_strings::uri_value;

#[derive(Default)]
pub struct AvoidDoubleSlashImports;

impl Rule for AvoidDoubleSlashImports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Diagnostics(
            directive_uris(program)
                .filter_map(|uri| {
                    let value = uri_value(uri);
                    let path = value.split_once("://").map_or(value.as_ref(), |(_, rest)| {
                        rest.find('/').map_or("", |start| &rest[start..])
                    });
                    path.contains("//").then(|| {
                        ctx.diagnostic("Avoid repeated `/` separators in directive URIs (import, export or part).", uri.span())
                    })
                })
                .collect(),
        )
    }
}

fn directive_uris(program: Program<'_>) -> impl Iterator<Item = StringLitNode<'_>> {
    program
        .imports()
        .flat_map(|import| {
            std::iter::once(import.uri()).chain(import.configurable_uris().map(ConfigurableUri::uri))
        })
        .chain(program.exports().flat_map(|export| {
            std::iter::once(export.uri()).chain(export.configurable_uris().map(ConfigurableUri::uri))
        }))
        .chain(program.part_directives().map(PartDirective::uri))
        .chain(program.part_of_directive().and_then(PartOfDirective::uri))
}
