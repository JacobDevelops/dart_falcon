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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use super::sort_constructors_first::{constructor_container, constructor_name_range};

#[derive(Default)]
pub struct SortUnnamedConstructorsFirst;

impl Rule for SortUnnamedConstructorsFirst {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        if let Some(members) = constructor_container(node) {
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
                    self.diags.push(self.ctx.diagnostic(
                        "sort-unnamed-constructors-first",
                        "Invalid location for the unnamed constructor.",
                        constructor_name_range(constructor),
                    ));
                }
            }
        }
        StructuralTraversal::SkipChildren
    }
}
}
