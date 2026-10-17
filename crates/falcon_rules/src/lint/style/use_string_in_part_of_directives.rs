//! Flags a `part of` directive that names its library instead of using a string URI.
//!
//! A `part of my.library.name;` directive identifies the enclosing library by its
//! declared name, which forces the parent to carry a `library` name and couples the
//! part to it indirectly. The URI form, `part of 'parent.dart';`, points straight at
//! the containing file, is more robust, and is the form modern Dart tooling prefers.
//! The rule reports only the named form (a non-empty dotted name with no string URI).

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseStringInPartOfDirectives;

impl Rule for UseStringInPartOfDirectives {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        if let Some(part_of) = program.part_of_directive()
            && part_of.uri().is_none()
            && part_of.name().next().is_some()
        {
            diags.push(ctx.diagnostic(
                "use-string-in-part-of-directives",
                "Use a string to refer to the containing library in a part-of directive.",
                part_of.span(),
            ));
        }
        RulePreparation::Diagnostics(diags)
    }
}
