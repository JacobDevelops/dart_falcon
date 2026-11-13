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
    #[serde(deserialize_with = "falcon_config::integer")]
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
    bodies: Vec<(usize, usize, Option<usize>)>,
    statements: Vec<(usize, Option<usize>)>,
    expressions: Vec<ExpressionFrame>,
}

#[derive(Clone, Copy, Default)]
struct LogicalSequence {
    first: Option<BinaryOp>,
    last: Option<BinaryOp>,
    score: usize,
}

impl LogicalSequence {
    fn append(&mut self, other: Self) {
        self.score += other.score;
        if self.last.is_some() && self.last == other.first {
            self.score -= 1;
        }
        self.first = self.first.or(other.first);
        self.last = other.last.or(self.last);
    }

    fn operator(&mut self, op: BinaryOp) {
        self.append(Self { first: Some(op), last: Some(op), score: 1 });
    }
}

struct ExpressionFrame {
    depth: usize,
    connected: bool,
    operator: Option<BinaryOp>,
    sequence: LogicalSequence,
}
falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }
    fn visit_function_body(&mut self, _node: FunctionBody<'a>) -> StructuralTraversal {
        self.bodies.push((self.count, self.depth, self.else_if));
        self.count = 0; self.depth = 0; self.else_if = None;
        StructuralTraversal::Continue
    }
    fn exit_function_body(&mut self, node: FunctionBody<'a>) {
        if self.count > self.threshold {
            self.diags.push(self.ctx.diagnostic(format!("Function has cognitive complexity {} (max {}). Reduce nesting or extract helpers.", self.count, self.threshold), node.span()));
        }
        if let Some(saved) = self.bodies.pop() { (self.count, self.depth, self.else_if) = saved; }
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
        let operator = match node {
            Expr::Binary(binary) if matches!(binary.op(), BinaryOp::And | BinaryOp::Or) => Some(binary.op()),
            _ => None,
        };
        self.expressions.push(ExpressionFrame {
            depth: self.depth,
            connected: operator.is_some() || matches!(node, Expr::Paren(_)),
            operator,
            sequence: LogicalSequence::default(),
        });
        if self.bodies.is_empty() { return StructuralTraversal::Continue; }
        if matches!(node, Expr::Conditional(_) | Expr::Switch(_)) {
            self.count += 1 + self.depth; self.depth += 1;
        }
        StructuralTraversal::Continue
    }
    fn exit_expr(&mut self, _node: Expr<'a>) {
        if let Some(frame) = self.expressions.pop() {
            self.depth = frame.depth;
            if let Some(parent) = self.expressions.last_mut().filter(|parent| parent.connected) {
                parent.sequence.append(frame.sequence);
                // The shared walker visits the left operand before the right.
                if let Some(operator) = parent.operator.take() {
                    parent.sequence.operator(operator);
                }
            } else if !self.bodies.is_empty() {
                self.count += frame.sequence.score;
            }
        }
    }
}
}
