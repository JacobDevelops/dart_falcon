//! Reports function bodies with excessive nesting-weighted control flow.
//!
//! `max-complexity` (integer, default: 15) limits the score. Ifs, loops,
//! switches, ternaries, and catches cost one plus their nesting depth. Else-if
//! and else cost one without extra nesting. Each connected logical operator
//! sequence costs one, with another point when && changes to || or vice versa.
//! Try/finally and plain blocks do not add complexity. Nested callable bodies
//! are measured independently.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Maximum cognitive complexity allowed for a function.
    #[serde(deserialize_with = "crate::lint::options::integer")]
    max_complexity: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { max_complexity: 15 }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct CognitiveComplexity;
impl Rule for CognitiveComplexity {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
            threshold: crate::lint::options::get::<Options>(ctx, Self::NAME).max_complexity,
            count: 0,
            depth: 0,
            else_if: None,
            logical: None,
            bodies: Vec::new(),
            statements: Vec::new(),
            expressions: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
    threshold: usize,
    count: usize,
    depth: usize,
    else_if: Option<usize>,
    logical: Option<BinaryOp>,
    bodies: Vec<(usize, usize, Option<usize>, Option<BinaryOp>)>,
    statements: Vec<(usize, Option<usize>)>,
    expressions: Vec<(usize, Option<BinaryOp>)>,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
    fn visit_function_body(&mut self, _node: FunctionBody<'a>) -> StructuralTraversal {
        self.bodies.push((self.count, self.depth, self.else_if, self.logical));
        self.count = 0; self.depth = 0; self.else_if = None; self.logical = None;
        StructuralTraversal::Continue
    }
    fn exit_function_body(&mut self, node: FunctionBody<'a>) {
        if self.count > self.threshold {
            self.diags.push(self.ctx.diagnostic(format!("Function has cognitive complexity {} (max {}). Reduce nesting or extract helpers.", self.count, self.threshold), node.span()));
        }
        if let Some(saved) = self.bodies.pop() { (self.count, self.depth, self.else_if, self.logical) = saved; }
    }
    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.statements.push((self.depth, self.else_if));
        if self.bodies.is_empty() { return StructuralTraversal::Continue; }
        if let Stmt::If(statement) = node {
            if self.else_if == Some(statement.span().start) { self.count += 1; }
            else { self.count += 1 + self.depth; self.depth += 1; }
            self.else_if = match statement.else_branch() {
                Some(Stmt::If(next)) => Some(next.span().start),
                Some(_) => { self.count += 1; None },
                None => None,
            };
        } else if matches!(node, Stmt::For(_) | Stmt::While(_) | Stmt::DoWhile(_) | Stmt::Switch(_)) {
            self.count += 1 + self.depth; self.depth += 1;
        }
        StructuralTraversal::Continue
    }
    fn exit_stmt(&mut self, _node: Stmt<'a>) {
        if let Some(saved) = self.statements.pop() { (self.depth, self.else_if) = saved; }
    }
    fn visit_catch_clause(&mut self, _node: CatchClause<'a>) -> StructuralTraversal {
        if !self.bodies.is_empty() { self.count += 1 + self.depth; self.depth += 1; }
        StructuralTraversal::Continue
    }
    fn exit_catch_clause(&mut self, _node: CatchClause<'a>) {
        if !self.bodies.is_empty() { self.depth -= 1; }
    }
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.expressions.push((self.depth, self.logical));
        if self.bodies.is_empty() { return StructuralTraversal::Continue; }
        if matches!(node, Expr::Conditional(_) | Expr::Switch(_)) {
            self.count += 1 + self.depth; self.depth += 1;
        }
        if let Expr::Binary(binary) = node && matches!(binary.op(), BinaryOp::And | BinaryOp::Or) {
            if self.logical != Some(binary.op()) { self.count += 1; }
            self.logical = Some(binary.op());
        } else if !matches!(node, Expr::Paren(_)) { self.logical = None; }
        StructuralTraversal::Continue
    }
    fn exit_expr(&mut self, _node: Expr<'a>) {
        if let Some(saved) = self.expressions.pop() { (self.depth, self.logical) = saved; }
    }
}
}
