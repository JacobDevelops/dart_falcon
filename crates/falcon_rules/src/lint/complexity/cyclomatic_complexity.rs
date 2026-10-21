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
//! `max-complexity` (integer, default: 20) — flag when the computed complexity
//! exceeds this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum cyclomatic complexity allowed for a function.
    #[serde(deserialize_with = "falcon_config::integer")]
    max_complexity: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_complexity: 20 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct CyclomaticComplexity;

impl Rule for CyclomaticComplexity {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
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

fn max_complexity_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::get::<Options>(ctx, CyclomaticComplexity::NAME).max_complexity
}

struct DecisionPoints<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    name: Option<Span>,
    body_depth: usize,
    count: usize,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for DecisionPoints<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        self.name = node.body().map(|_| node.name().span());
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        self.name = falcon_syntax::cst::visit::declared_member_name(node, self.ctx.source());
        if self.name.is_some() { StructuralTraversal::Continue } else { StructuralTraversal::SkipChildren }
    }

    fn route_child(&mut self, parent: StructuralNode<'a>, child: StructuralNode<'a>) -> bool {
        match parent {
            StructuralNode::FunctionDecl(_) | StructuralNode::MethodDecl(_)
            | StructuralNode::ConstructorDecl(_) | StructuralNode::GetterDecl(_)
            | StructuralNode::SetterDecl(_) | StructuralNode::OperatorDecl(_)
            | StructuralNode::PrimaryConstructorBody(_) => matches!(child, StructuralNode::FunctionBody(_)),
            _ => true,
        }
    }

    fn visit_function_body(&mut self, _node: FunctionBody<'a>) -> StructuralTraversal {
        if self.name.is_none() {
            return StructuralTraversal::SkipChildren;
        }
        if self.body_depth == 0 {
            self.count = 0;
        }
        self.body_depth += 1;
        StructuralTraversal::Continue
    }

    fn exit_function_body(&mut self, _node: FunctionBody<'a>) {
        if self.name.is_none() {
            return;
        }
        self.body_depth -= 1;
        if self.body_depth == 0 {
            let complexity = self.count + 1;
            let threshold = max_complexity_option(self.ctx);
            if complexity > threshold {
                self.diags.push(self.ctx.diagnostic(
                    format!("Function has a cyclomatic complexity of {complexity} (max {threshold})."),
                    self.name.unwrap(),
                ));
            }
            self.name = None;
        }
    }

    fn visit_formal_param(&mut self, _node: FormalParam<'a>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if self.body_depth == 0 {
            return StructuralTraversal::Continue;
        }
        self.count += match node {
            Stmt::If(n) => {
                1 + usize::from(matches!(n.condition(), IfCondition::Case(case) if case.guard().is_some()))
            }
            Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) => 1,
            Stmt::TryCatch(n) => n.catches().len(),
            Stmt::Switch(n) => n
                .cases()
                .flat_map(|case| case.cases())
                .map(|kind| match kind {
                    SwitchCaseKind::Pattern(label) => 1 + usize::from(label.guard().is_some()),
                    SwitchCaseKind::Expr(_) => 1,
                    SwitchCaseKind::Default(_) => 0,
                })
                .sum(),
            _ => 0,
        };
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if self.body_depth == 0 {
            return StructuralTraversal::Continue;
        }
        self.count += match node {
            Expr::Binary(binary) if matches!(binary.op(), BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce) => 1,
            Expr::Conditional(_) => 1,
            Expr::Switch(switch) => switch.arms()
                .map(|arm| 1 + usize::from(arm.guard().is_some()))
                .sum(),
            _ => 0,
        };
        StructuralTraversal::Continue
    }
}
}
