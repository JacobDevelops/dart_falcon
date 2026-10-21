//! Flags a variable declaration that declares more than one variable.
//!
//! Declaring each variable on its own line makes each one easier to find,
//! document, and change. Top-level variables, fields, and local variables are
//! checked; the second declared name is reported:
//!
//! ```dart
//! var a = 1, b = 2;            // `b` flagged
//! int x, y;                    // `y` flagged
//! for (var i = 0, j = 1; ;) {} // fine: loop initializers are exempt
//! ```
//!
//! Adopted from the official `avoid_multiple_declarations_per_line` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_syntax::cst::{self, AstChildren};

use falcon_diagnostics::Diagnostic;
#[derive(Default)]
pub struct AvoidMultipleDeclarationsPerLine;

impl Rule for AvoidMultipleDeclarationsPerLine {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check<'a>(&mut self, mut declarators: AstChildren<'a, cst::VarDeclarator<'a>>) {
        if let Some(second) = declarators.nth(1) {
            self.diags.push(self.ctx.diagnostic("Multiple variables declared on a single line.", second.name().span()));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: cst::TopLevelDecl<'_>) -> StructuralTraversal {
        if let cst::TopLevelDecl::Variable(variable) = node {
            self.check(variable.declarators());
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: cst::FieldDecl<'_>) -> StructuralTraversal {
        self.check(node.declarators());
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: cst::Stmt<'_>) -> StructuralTraversal {
        if let cst::Stmt::LocalVar(local) = node {
            self.check(local.declarators());
        }
        StructuralTraversal::Continue
    }
}
}
