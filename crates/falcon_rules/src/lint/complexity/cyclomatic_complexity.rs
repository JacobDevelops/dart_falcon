//! Flags a function whose cyclomatic complexity exceeds the configured threshold.
//!
//! Cyclomatic complexity counts the independent paths through a function; a high
//! value means many branches to understand and test. The rule computes
//! complexity as one plus each decision point — `if`, ternary, `&&`, `||`, `??`,
//! loops (`for`/`while`/`do`), `catch` clauses, non-default `case`s, and pattern
//! `when` guards — counted across the whole body tree, so decision points inside
//! nested closures and local functions count toward the enclosing function. It
//! reports at the function name when the total exceeds the threshold.
//! Operator bodies are included. Collection `if`/`for` elements are not decision
//! points themselves, but decisions in their conditions and children are counted.
//! Parameter defaults are constant expressions, not runtime body decisions, and
//! are excluded for both the declaration and its nested functions.
//!
//! ## Options
//!
//! `max_complexity` (integer, default: 20) — flag when the computed complexity
//! exceeds this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct CyclomaticComplexity;

impl Rule for CyclomaticComplexity {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(DecisionPoints {
            ctx,
            diags: Vec::new(),
            name: None,
            body_depth: 0,
            count: 0,
        }))
    }
}

/// Read the `max_complexity` option (default 20). Malformed/missing → default.
fn max_complexity_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "cyclomatic-complexity"),
        "max_complexity",
    )
    .unwrap_or(20)
}

struct DecisionPoints<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    name: Option<Span>,
    body_depth: usize,
    count: usize,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for DecisionPoints<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        self.name = node.body.as_ref().map(|_| node.name.span);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        self.name = falcon_syntax::visitor::declared_member_name(node, self.ctx.source);
        if self.name.is_some() { StructuralTraversal::Continue } else { StructuralTraversal::SkipChildren }
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        match parent {
            StructuralNode::FunctionDecl(_) | StructuralNode::MethodDecl(_)
            | StructuralNode::ConstructorDecl(_) | StructuralNode::GetterDecl(_)
            | StructuralNode::SetterDecl(_) | StructuralNode::OperatorDecl(_)
            | StructuralNode::PrimaryConstructorBody(_) => matches!(child, StructuralNode::FunctionBody(_)),
            _ => true,
        }
    }

    fn visit_function_body(&mut self, _node: &FunctionBody) -> StructuralTraversal {
        if self.name.is_none() {
            return StructuralTraversal::SkipChildren;
        }
        if self.body_depth == 0 {
            self.count = 0;
        }
        self.body_depth += 1;
        StructuralTraversal::Continue
    }

    fn exit_function_body(&mut self, _node: &FunctionBody) {
        if self.name.is_none() {
            return;
        }
        self.body_depth -= 1;
        if self.body_depth == 0 {
            let complexity = self.count + 1;
            let threshold = max_complexity_option(self.ctx);
            if complexity > threshold {
                self.diags.push(self.ctx.diagnostic(
                    "cyclomatic-complexity",
                    format!("Function has a cyclomatic complexity of {complexity} (max {threshold})."),
                    self.name.unwrap(),
                ));
            }
            self.name = None;
        }
    }

    fn visit_formal_param(&mut self, _node: &FormalParam) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if self.body_depth == 0 {
            return StructuralTraversal::Continue;
        }
        self.count += match node {
            Stmt::If(n) => {
                1 + usize::from(matches!(&n.condition, IfCondition::Case(_, _, Some(_))))
            }
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) => 1,
            Stmt::TryCatch(n) => n.catches.len(),
            Stmt::Switch(n) => n
                .cases
                .iter()
                .flat_map(|case| &case.cases)
                .map(|kind| match kind {
                    SwitchCaseKind::Pattern(_, guard) => 1 + usize::from(guard.is_some()),
                    SwitchCaseKind::Expr(_) => 1,
                    SwitchCaseKind::Default => 0,
                })
                .sum(),
            _ => 0,
        };
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if self.body_depth == 0 {
            return StructuralTraversal::Continue;
        }
        self.count += match node {
            Expr::Binary {
                op: BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce,
                ..
            }
            | Expr::Conditional { .. } => 1,
            Expr::Switch { arms, .. } => arms
                .iter()
                .map(|arm| 1 + usize::from(arm.guard.is_some()))
                .sum(),
            _ => 0,
        };
        StructuralTraversal::Continue
    }
}
}
