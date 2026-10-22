//! Flags named extensions whose name is not UpperCamelCase.
//!
//! Extensions are types and should follow the same naming convention as classes
//! and enums so that tooling and readers treat them consistently. Leading
//! underscores are ignored for the check; the remainder must be one or more
//! words, each starting with an uppercase letter or `$`. Unnamed extensions have
//! no identifier to check and are skipped.

use crate::lint::naming::is_upper_camel_case;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct CamelCaseExtensions;

impl Rule for CamelCaseExtensions {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        if let TopLevelDecl::Extension(ext) = decl
            && let Some(name) = ext.name()
            && !is_upper_camel_case(name.name())
        {
            self.diags.push(self.ctx.diagnostic(MESSAGE, name.span()));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

const MESSAGE: &str = "Name extensions using `UpperCamelCase`.";
