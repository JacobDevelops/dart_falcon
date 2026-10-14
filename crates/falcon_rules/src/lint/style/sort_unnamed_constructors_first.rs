//! Flags an unnamed constructor declared after a named constructor.
//!
//! The unnamed constructor (`C()` or `C.new()`) is the primary way to create an
//! instance, so it comes before every named constructor of a class, mixin
//! class, enum, or extension type:
//!
//! ```dart
//! class A {
//!   A.named();
//!   A();                       // flagged
//! }
//! ```
//!
//! Only constructors are ordered; fields and methods in between do not matter.
//!
//! Adopted from the official `sort_unnamed_constructors_first` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use super::sort_constructors_first::{constructor_container, constructor_name_range};

#[derive(Default)]
pub struct SortUnnamedConstructorsFirst;

impl Rule for SortUnnamedConstructorsFirst {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for members in program
            .declarations
            .iter()
            .filter_map(constructor_container)
        {
            let mut named_seen = false;
            for member in members {
                let ClassMember::Constructor(constructor) = member else {
                    continue;
                };
                let unnamed = constructor
                    .constructor_name
                    .as_ref()
                    .is_none_or(|name| name.name == "new");
                if !unnamed {
                    named_seen = true;
                } else if named_seen {
                    diags.push(ctx.diagnostic(
                        "sort-unnamed-constructors-first",
                        "Invalid location for the unnamed constructor.",
                        constructor_name_range(constructor),
                    ));
                }
            }
        }
        diags
    }
}
