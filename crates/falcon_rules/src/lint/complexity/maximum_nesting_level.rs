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
//! `max_nesting` (integer, default: 5) — flag when the nesting depth exceeds
//! this.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct MaximumNestingLevel;

impl Rule for MaximumNestingLevel {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
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

/// Read the `max_nesting` option (default 5). Malformed/missing → default.
fn max_nesting_option(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "maximum-nesting-level"),
        "max_nesting",
    )
    .unwrap_or(5)
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
impl StructuralRuleVisitor for Nesting<'_> {
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
            self.depth = 0;
            self.max = 0;
            self.else_if = None;
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
            let max = self.max;
            let threshold = max_nesting_option(self.ctx);
            if max > threshold {
                self.diags.push(self.ctx.diagnostic(
                    "maximum-nesting-level",
                    format!("Function has a nesting level of {max} (max {threshold})."),
                    self.name.unwrap(),
                ));
            }
            self.name = None;
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
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
            if !matches!(node, Stmt::If(n) if self.else_if == Some(n.span.start)) {
                self.depth += 1;
            }
            self.max = self.max.max(self.depth);
        }
        if let Stmt::If(n) = node {
            // Else-if chains share the outer if's level.
            self.else_if = n.else_branch.as_deref().and_then(|branch| match branch {
                Stmt::If(n) => Some(n.span.start),
                _ => None,
            });
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, _node: &Stmt) {
        (self.depth, self.else_if) = self.statements.pop().unwrap();
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.expressions.push(self.depth);
        if matches!(node, Expr::FuncExpr { .. }) {
            self.depth = 0;
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: &Expr) {
        self.depth = self.expressions.pop().unwrap();
    }
}
}
