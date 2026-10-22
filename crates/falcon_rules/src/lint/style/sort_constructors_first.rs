//! Flags a constructor declared after a non-constructor member.
//!
//! Constructors describe how an instance comes to be, so they lead the member
//! list of a class, mixin class, enum, or extension type:
//!
//! ```dart
//! class A {
//!   A();
//!   int x = 0;
//!   A.named();                 // flagged
//! }
//! ```
//!
//! Each out-of-place constructor is reported at its name. Enum values are not
//! members, so they never count as preceding declarations.
//! Primary constructor headers do not participate in body ordering; their
//! explicit `this` bodies do, and are reported at the `this` keyword.
//!
//! Adopted from the official `sort_constructors_first` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct SortConstructorsFirst;

impl Rule for SortConstructorsFirst {
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
            let mut other = false;
            for member in members {
                match member {
                    ClassMember::Constructor(constructor) => {
                        if other {
                            self.diags.push(self.ctx.diagnostic(
                                "Constructor declarations should be before non-constructor \
                         declarations.",
                                constructor_name_range(constructor),
                            ));
                        }
                    }
                    ClassMember::PrimaryConstructorBody(body) => {
                        if other {
                            self.diags.push(self.ctx.diagnostic(
                                "Constructor declarations should be before non-constructor \
                         declarations.",
                                body.this_span(),
                            ));
                        }
                    }
                    _ => other = true,
                }
            }
        }
        StructuralTraversal::SkipChildren
    }
}
}

use crate::lint::support::constructors::{constructor_container, constructor_name_range};
