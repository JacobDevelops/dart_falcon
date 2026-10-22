//! Requires public declarations in test files, except main, to be private.
//!
//! Private test helpers and fixtures cannot accidentally become another file's
//! API and can be identified as unused without cross-library consumers.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidTopLevelMembersInTests;

impl Rule for AvoidTopLevelMembersInTests {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !crate::lint::declarations::has_test_file_suffix(ctx.file_path()) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, name: Identifier<'_>) {
        if !name.name().starts_with('_') {
            self.diags.push(self.ctx.diagnostic("Make this top-level test declaration private.", name.span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let name = match declaration {
            TopLevelDecl::Class(node) => Some(node.name()),
            TopLevelDecl::ClassTypeAlias(node) => Some(node.name()),
            TopLevelDecl::Mixin(node) => Some(node.name()),
            TopLevelDecl::MixinClass(node) => Some(node.name()),
            TopLevelDecl::Enum(node) => Some(node.name()),
            TopLevelDecl::Extension(node) => node.name(),
            TopLevelDecl::ExtensionType(node) => Some(node.name()),
            TopLevelDecl::TypeAlias(node) => Some(node.name()),
            TopLevelDecl::Function(node) if node.name().name() != "main" => Some(node.name()),
            TopLevelDecl::Variable(node) => {
                for variable in node.declarators() { self.check(variable.name()); }
                None
            }
            _ => None,
        };
        if let Some(name) = name { self.check(name); }
        StructuralTraversal::SkipChildren
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
}
}
