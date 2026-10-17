//! Disallow mutable global and static state.
//!
//! Flags top-level variables and `static` fields that are mutable — anything not
//! declared `const` or plain `final`, including `late final`, which can still be
//! assigned once at an arbitrary point. Mutable global state is reachable and
//! writable from anywhere, which makes data flow hard to trace, introduces
//! initialization-order and concurrency hazards, and lets tests contaminate one
//! another through leftover shared state. Prefer `const` or `final`
//! declarations, and thread mutable state explicitly through constructors or
//! parameters rather than hanging it off globals.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidGlobalState;

impl Rule for AvoidGlobalState {
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
        if let TopLevelDecl::Variable(var) = decl
            && is_mutable(var.keyword(), var.is_late())
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-global-state",
                "Avoid mutable global state — use const or final declarations instead",
                var.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Field(field) = member
            && field.is_static()
            && is_mutable(field.keyword(), field.is_late())
        {
            self.diags.push(self.ctx.diagnostic(
                "avoid-global-state",
                "Avoid mutable global state — use const or final declarations instead",
                field.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn is_mutable(keyword: VarKeyword, is_late: bool) -> bool {
    keyword == VarKeyword::Var || (keyword == VarKeyword::Final && is_late)
}
