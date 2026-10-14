//! Flags an assignment immediately followed by `return` of the same variable.
//!
//! When a block ends with `x = e; return x;` the two statements can be joined
//! into `return x = e;` (or simply `return e;` for a local). The rule looks at
//! the last two statements of every block: the second-to-last must be an
//! assignment (any operator), increment, decrement, or other prefix/postfix
//! expression on the same variable or property chain that the final `return`
//! yields. A run of assignments to that same target (`x = a; x += b; return
//! x;`) is left alone, as are switch-case statement lists, which are not blocks.
//!
//! ```dart
//! int f(int a) {
//!   a += 1; // flagged
//!   return a;
//! }
//! ```
//!
//! Adopted from the official `join_return_with_assignment` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_dart_parser::lexer::Lexer;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::{StatementListKind, for_each_stmt_list};

#[derive(Default)]
pub struct JoinReturnWithAssignment;

impl Rule for JoinReturnWithAssignment {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        for_each_stmt_list(program, &mut |stmts, kind| {
            if kind != StatementListKind::Block {
                return;
            }
            let [.., second_last, last] = stmts else {
                return;
            };
            let Stmt::Return(ReturnStmt {
                value: Some(returned),
                span,
            }) = last
            else {
                return;
            };
            // The analyzer compares the returned node without unwrapping it.
            // Falcon erases parentheses, so preserve that distinction lexically.
            let prefix = &ctx.source[span.start..returned.span().start];
            if Lexer::new(prefix).tokenize().into_iter().any(|token| {
                !token.is_trivia() && token.kind != TokenKind::Eof && token.text(prefix) != "return"
            }) {
                return;
            }
            let Some(assigned) = assignment_target(second_last) else {
                return;
            };
            let third_last = stmts
                .len()
                .checked_sub(3)
                .and_then(|index| assignment_target(&stmts[index]));
            if third_last.is_some_and(|third| same_element(assigned, third))
                || !same_element(returned, assigned)
            {
                return;
            }
            if let Stmt::Expr(statement) = second_last {
                diagnostics.push(ctx.diagnostic(
                    "join-return-with-assignment",
                    "Assignment could be inlined in 'return' statement.",
                    *statement.expr.span(),
                ));
            }
        });
        diagnostics
    }
}

/// The written operand of an expression statement that is an assignment, a
/// prefix expression, or a postfix expression (including `!`).
fn assignment_target(stmt: &Stmt) -> Option<&Expr> {
    let Stmt::Expr(statement) = stmt else {
        return None;
    };
    match &statement.expr {
        Expr::Assign { target, .. } => Some(target),
        Expr::Unary { operand, .. }
        | Expr::PostfixIncDec { operand, .. }
        | Expr::NullAssert { operand, .. } => Some(operand),
        _ => None,
    }
}

/// Whether both expressions name the same element: equal simple identifiers, or
/// property chains (`a.b`, `a.b.c`) rooted at the same simple identifier with
/// equal property names. Adjacent statements of one block share a scope, so an
/// identifier name denotes the same element in both. `this.x` is not an
/// identifier chain and never matches, as in the analyzer.
fn same_element(mut left: &Expr, mut right: &Expr) -> bool {
    loop {
        match (left, right) {
            (Expr::Ident(left), Expr::Ident(right)) => return left.name == right.name,
            (
                Expr::Field {
                    object: left_object,
                    field: left_field,
                    ..
                },
                Expr::Field {
                    object: right_object,
                    field: right_field,
                    ..
                },
            ) if left_field.name == right_field.name => {
                left = left_object;
                right = right_object;
            }
            _ => return false,
        }
    }
}
