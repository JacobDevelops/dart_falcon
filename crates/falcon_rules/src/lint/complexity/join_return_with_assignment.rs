//! Flags an assignment immediately followed by `return` of the same variable.
//!
//! When a block ends with `x = e; return x;` the two statements can be joined
//! into `return x = e;` (or simply `return e;` for a local). The rule looks at
//! the last two statements of every block: the second-to-last must be an
//! assignment (any operator), increment, decrement, or other prefix/postfix
//! expression on the same variable or property chain that the final `return`
//! yields. A run of assignments to that same target (`x = a; x += b; return
//! x;`) is left alone, as are switch-case statement lists, which are not blocks.
//! Diagnostics cover the expression statement, including its semicolon.
//!
//! ```dart
//! int f(int a) {
//!   a += 1; // flagged
//!   return a;
//! }
//! ```
//!
//! Adopted from the official `join_return_with_assignment` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct JoinReturnWithAssignment;

impl Rule for JoinReturnWithAssignment {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a, 's> {
    ctx: &'a AnalyzeContext<'s>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_block_end(&mut self, node: Block<'a>, state: &SemanticState<'a>) {
        let mut stmts = node.stmts().rev();
        let Some(Stmt::Return(ret)) = stmts.next() else {
            return;
        };
        let Some(returned) = ret.value() else {
            return;
        };
        let Some(second_last) = stmts.next() else {
            return;
        };
        let Some((assigned, write)) = assignment_target(second_last) else {
            return;
        };
        let third = stmts.next().and_then(assignment_target);
        if !third.is_some_and(|(third, third_write)| {
            same_element(assigned, write, third, third_write, state)
        }) && same_element(returned, false, assigned, write, state)
            && let Stmt::Expr(statement) = second_last
        {
            self.diagnostics.push(self.ctx.diagnostic("Assignment could be inlined in `return` statement.", statement.span()));
        }
    }
}

/// The written operand of an expression statement that is an assignment, a
/// prefix expression, or a postfix expression (including `!`).
fn assignment_target(stmt: Stmt<'_>) -> Option<(Expr<'_>, bool)> {
    let Stmt::Expr(statement) = stmt else {
        return None;
    };
    match statement.expr().unparenthesized() {
        Expr::Assign(assign) => Some((assign.target(), true)),
        Expr::Unary(unary) => Some((
            unary.operand(),
            matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus),
        )),
        Expr::PostfixIncDec(expr) => Some((expr.operand(), true)),
        Expr::NullAssert(expr) => Some((expr.operand(), false)),
        _ => None,
    }
}

use crate::lint::support::element_identity::same_element;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn assignment_range_includes_the_statement_semicolon() {
        let source = "int f(int x) { x += 1; return x; }";
        let diagnostics = inference::analyze(JoinReturnWithAssignment::NAME, &JoinReturnWithAssignment, Path::new("join_span.dart"),
        source,
        &FalconConfig::default(),);
        assert_eq!(diagnostics.len(), 1);
        let span = diagnostics[0].span;
        assert_eq!(span.start, source.find("x +=").unwrap());
        assert_eq!(&source[span.start..span.end], "x += 1;");
    }
}
