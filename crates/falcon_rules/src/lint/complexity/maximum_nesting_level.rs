//! Flags a function whose control-flow nesting runs deeper than the configured level.
//!
//! Deeply nested blocks push logic to the right and make control flow hard to
//! follow; early returns, guard clauses, and extracted helpers flatten it. The
//! rule measures the deepest nesting of control structures — `if`, `for`,
//! `while`, `do`, `switch`, and `try` — within a function body. A plain block
//! does not add a level, `else if` chains stay at the same level, and a nested
//! local function or closure restarts the count. Their deepest level still
//! contributes to the enclosing declaration's maximum. Operator bodies are
//! included; the rule reports at the declaration when depth exceeds the threshold.
//!
//! ## Options
//!
//! `max-nesting` (integer, default: 5) — flag when the nesting depth exceeds
//! this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum nesting depth allowed in a function.
    max_nesting: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_nesting: 5 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct MaximumNestingLevel;

impl Rule for MaximumNestingLevel {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Nesting {
            ctx,
            diags: Vec::new(),
            name: None,
            body_depth: 0,
            depth: 0,
            max: 0,
            else_if: None,
            statements: Vec::new(),
            expressions: Vec::new(),
        }))
    }
}

fn max_nesting_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::get::<Options>(ctx, MaximumNestingLevel::NAME).max_nesting
}

struct Nesting<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    name: Option<Span>,
    body_depth: usize,
    depth: usize,
    max: usize,
    else_if: Option<usize>,
    statements: Vec<(usize, Option<usize>)>,
    expressions: Vec<usize>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Nesting<'a> {
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
            self.depth = 0;
            self.max = 0;
            self.else_if = None;
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
            let max = self.max;
            let threshold = max_nesting_option(self.ctx);
            if max > threshold {
                self.diags.push(self.ctx.diagnostic(
                    format!("Function has a nesting level of {max} (max {threshold})."),
                    self.name.unwrap(),
                ));
            }
            self.name = None;
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.statements.push((self.depth, self.else_if));
        if matches!(node, Stmt::LocalFunc(_)) {
            self.depth = 0;
        } else if matches!(
            node,
            Stmt::If(_)
                | Stmt::For(_)
                | Stmt::While(_)
                | Stmt::DoWhile(_)
                | Stmt::Switch(_)
                | Stmt::TryCatch(_)
        ) {
            if !matches!(node, Stmt::If(n) if self.else_if == Some(n.span().start)) {
                self.depth += 1;
            }
            self.max = self.max.max(self.depth);
        }
        if let Stmt::If(n) = node {
            // Else-if chains share the outer if's level.
            self.else_if = n.else_branch().and_then(|branch| match branch {
                Stmt::If(n) => Some(n.span().start),
                _ => None,
            });
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, _node: Stmt<'a>) {
        (self.depth, self.else_if) = self.statements.pop().unwrap();
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.expressions.push(self.depth);
        if matches!(node, Expr::FuncExpr(_)) {
            self.depth = 0;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.depth = self.expressions.pop().unwrap();
    }
}
}
