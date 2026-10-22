//! Flags a redundant `late` modifier on an initialized static or top-level variable.
//!
//! Static fields and top-level variables in Dart are already initialized lazily:
//! their initializer does not run until the variable is first read. Marking such
//! a declaration `late` therefore buys nothing, and it misleads readers into
//! thinking deferred initialization is significant here. The rule only fires when
//! the declarator actually has an initializer, since `late` on an uninitialized
//! static or top-level variable does carry meaning. Drop the `late` keyword.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryLate;

impl Rule for UnnecessaryLate {
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
        let ctx = self.ctx;
        let diags = &mut self.diags;
        match decl {
            TopLevelDecl::Variable(var) if var.is_late() => {
                flag_initialized(var.declarators(), ctx, diags);
            }
            _ => {
                if let Some(members) = crate::lint::declarations::members_of(decl) {
                    static_late_fields(members, ctx, diags);
                }
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn static_late_fields<'a>(
    members: AstChildren<'a, ClassMember<'a>>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    for member in members {
        if let ClassMember::Field(field) = member
            && field.is_static()
            && field.is_late()
        {
            flag_initialized(field.declarators(), ctx, diags);
        }
    }
}

fn flag_initialized<'a>(
    declarators: AstChildren<'a, VarDeclarator<'a>>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    for decl in declarators {
        if decl.initializer().is_some() {
            diags.push(ctx.diagnostic("Remove `late` from this already-lazy static or top-level variable.".to_string(), decl.name().span()));
        }
    }
}
