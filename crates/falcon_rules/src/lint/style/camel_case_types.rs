//! Flags type names that are not UpperCamelCase.
//!
//! Classes, mixins, enums, extension types, and type aliases are all types and
//! read most predictably when named in UpperCamelCase, matching the rest of the
//! Dart ecosystem. Leading underscores are ignored, then each word must begin
//! with an uppercase letter or `$`; a name that is entirely underscores is
//! treated as a wildcard and never flagged.

use crate::lint::naming::is_upper_camel_case;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct CamelCaseTypes;

impl Rule for CamelCaseTypes {
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

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let ctx = self.ctx;
        let diags = &mut self.diags;
        match decl {
            TopLevelDecl::Class(c) => check(c.name(), diags, ctx),
            TopLevelDecl::MixinClass(c) => check(c.name(), diags, ctx),
            TopLevelDecl::Mixin(m) => check(m.name(), diags, ctx),
            TopLevelDecl::Enum(e) => check(e.name(), diags, ctx),
            TopLevelDecl::ExtensionType(x) => check(x.name(), diags, ctx),
            TopLevelDecl::TypeAlias(t) => check(t.name(), diags, ctx),
            _ => {}
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

const MESSAGE: &str = "Name types using `UpperCamelCase`.";

fn check(name: Identifier<'_>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !is_upper_camel_case(name.name()) {
        diags.push(ctx.diagnostic(MESSAGE, name.span()));
    }
}
