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
use falcon_syntax::cst::*;

use crate::lint::support::constructors::{constructor_container, constructor_name_range};

#[derive(Default)]
pub struct SortUnnamedConstructorsFirst;

impl Rule for SortUnnamedConstructorsFirst {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        if let Some(members) = constructor_container(node) {
            let mut named_seen = false;
            for member in members {
                let ClassMember::Constructor(constructor) = member else {
                    continue;
                };
                let unnamed = constructor
                    .constructor_name()
                    .is_none_or(|name| name.name() == "new");
                if !unnamed {
                    named_seen = true;
                } else if named_seen {
                    self.diags.push(self.ctx.diagnostic(
                        "Place the unnamed constructor before named constructors.",
                        constructor_name_range(constructor),
                    ));
                }
            }
        }
        StructuralTraversal::SkipChildren
    }
}
}
