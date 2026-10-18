//! Flags assignments evaluated inside control-flow conditions.
//!
//! Assignments can hide a mistyped `==` or an unexpected state change. Move the
//! assignment before the condition, or use a comparison. This checks all assignment
//! operators in `if`, `while`, `do`/`while`, traditional `for`, ternary, and collection
//! conditions, including nested expressions and `if case` or switch guards.
//! Loop initializers, updates, and branches are excluded unless nested within a
//! surrounding condition. Closure bodies never inherit an outer condition, but
//! conditions inside them are checked. Increment/decrement operators and assert
//! arguments are outside this rule's scope.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidAssignmentsAsConditions;

impl Rule for AvoidAssignmentsAsConditions {
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

impl Collector<'_> {
    fn check(&mut self, node: SyntaxNode<'_>) {
        if is_in_condition(node) {
            self.diags.push(self.ctx.diagnostic(
                "avoid-assignments-as-conditions",
                "Avoid assignments inside conditions; move the assignment out or use a comparison.",
                node.span(),
            ));
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Assign(assign) = node {
            self.check(assign.syntax());
        }
        StructuralTraversal::Continue
    }

    fn visit_cascade_op(&mut self, node: CascadeOp<'a>) -> StructuralTraversal {
        match node {
            CascadeOp::Assign(assign) => self.check(assign.syntax()),
            CascadeOp::IndexAssign(assign) => self.check(assign.syntax()),
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}

pub(super) fn is_in_condition(mut child: SyntaxNode<'_>) -> bool {
    while let Some(parent) = child.parent() {
        match parent.kind() {
            NodeKind::ExprCondition | NodeKind::ForCondition | NodeKind::ExprGuard => return true,
            NodeKind::FuncExpr => return false,
            _ => {}
        }
        if let Some(label) = CasePatternLabel::cast(parent)
            && label.guard().is_some_and(|guard| guard.syntax() == child)
        {
            return true;
        }
        if let Some(statement) = Stmt::cast(parent) {
            return match statement {
                Stmt::If(node) => node.condition().syntax() == child,
                Stmt::While(node) => node.condition().syntax() == child,
                Stmt::DoWhile(node) => node.condition().syntax() == child,
                _ => false,
            };
        }
        if let Some(Expr::Conditional(node)) = Expr::cast(parent)
            && node.condition().syntax() == child
        {
            return true;
        }
        child = parent;
    }
    false
}
