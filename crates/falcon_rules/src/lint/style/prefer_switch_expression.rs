//! Prefer switch expressions for exhaustive switches that only return or assign a value.
//!
//! A switch expression makes value selection explicit without repeating `return`
//! or the assignment target. This rule flags Dart 3 switches whose branches are
//! all a single value-returning `return`, or all a plain `=` assignment to the
//! same simple identifier, optionally followed by an unlabeled `break`. Single
//! `throw` branches may accompany either form.
//!
//! A `default` or unguarded, untyped wildcard case is required: exhaustiveness
//! is not inferred from types. Grouped/fallthrough cases, labeled cases, empty
//! bodies, blocks, extra statements, mixed return/assignment branches, compound
//! assignments, property/index targets, and labeled breaks are excluded.
//! Assignment patterns binding the target name are excluded, since equal target
//! spellings need not refer to the same variable across branches.
//! No automatic fix is offered.

use falcon_analyze::{
    AnalyzeContext, Feature, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferSwitchExpression;

impl Rule for PreferSwitchExpression {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !ctx.language_version().supports(Feature::Patterns) {
            return RulePreparation::Diagnostics(Vec::new());
        }
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

#[derive(PartialEq, Eq)]
enum Branch<'a> {
    Return,
    Throw,
    Assign(&'a str),
}

fn branch(case: SwitchCase<'_>) -> Option<Branch<'_>> {
    let mut body = case.body();
    match body.next()? {
        Stmt::Return(statement) if statement.value().is_some() && body.next().is_none() => {
            Some(Branch::Return)
        }
        Stmt::Throw(_) if body.next().is_none() => Some(Branch::Throw),
        Stmt::Expr(statement) => {
            let Expr::Assign(assignment) = statement.expr() else {
                return None;
            };
            let Expr::Ident(target) = assignment.target() else {
                return None;
            };
            if assignment.op() != AssignOp::Eq || target.name() == "_" {
                return None;
            }
            let valid_end = match body.next() {
                None => true,
                Some(Stmt::Break(statement)) => statement.label().is_none(),
                _ => false,
            };
            if !valid_end || body.next().is_some() {
                return None;
            }
            if case.cases().any(|label| {
                label.syntax().descendants().any(|node| {
                    VariablePattern::cast(node)
                        .is_some_and(|pattern| pattern.name().name() == target.name())
                })
            }) {
                return None;
            }
            Some(Branch::Assign(target.name()))
        }
        _ => None,
    }
}

fn convertible(switch: SwitchStmt<'_>) -> bool {
    let mut expected = None;
    let mut exhaustive = false;
    for case in switch.cases() {
        if case.labels().next().is_some() {
            return false;
        }
        let mut labels = case.cases();
        let Some(label) = labels.next() else {
            return false;
        };
        if labels.next().is_some() {
            return false;
        }
        exhaustive |= match label {
            SwitchCaseKind::Default(_) => true,
            SwitchCaseKind::Pattern(label) if label.guard().is_none() => {
                matches!(label.pattern(), Pattern::Wildcard(pattern) if pattern.type_().is_none())
            }
            _ => false,
        };
        let Some(actual) = branch(case) else {
            return false;
        };
        if actual == Branch::Throw {
            continue;
        }
        if expected
            .as_ref()
            .is_some_and(|expected| *expected != actual)
        {
            return false;
        }
        expected = Some(actual);
    }
    exhaustive && expected.is_some()
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, statement: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Switch(switch) = statement
            && convertible(switch)
        {
            self.diags.push(self.ctx.diagnostic(
                "prefer-switch-expression",
                "Use a switch expression to select this value.",
                switch.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}
