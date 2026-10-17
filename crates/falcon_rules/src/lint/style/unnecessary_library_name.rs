//! Flags a `library` directive that carries an explicit name.
//!
//! Since Dart 2.19 the `library` directive no longer needs a name: `part of`
//! directives can reference their parent by URI, and library-level doc comments
//! and annotations attach to a bare `library;`. A named library is legacy syntax
//! that only adds a global identifier which can collide across packages. Remove
//! the name, keeping the `library;` directive itself if it anchors documentation
//! or annotations.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::*;
use falcon_syntax::language_version::Feature;

#[derive(Default)]
pub struct UnnecessaryLibraryName;

impl Rule for UnnecessaryLibraryName {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !ctx.language_version().supports(Feature::UnnamedLibraries) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mut diags = Vec::new();
        if let Some(lib) = program.library_directive()
            && let (Some(first), Some(last)) = (lib.name().next(), lib.name().last())
        {
            diags.push(ctx.diagnostic(
                "unnecessary-library-name",
                "Library names are not necessary.",
                DiagSpan {
                    start: first.span().start,
                    end: last.span().end,
                },
            ));
        }
        RulePreparation::Diagnostics(diags)
    }
}
