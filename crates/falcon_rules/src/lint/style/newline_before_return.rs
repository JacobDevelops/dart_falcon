//! Flags `return` statements that are not preceded by a blank line.
//!
//! Separating a `return` from the statements above it with a blank line makes
//! the exit point of a block visually distinct, so readers can spot where
//! control leaves the function without parsing the surrounding code. The rule
//! only checks returns that follow another statement in the same block.
//! Switch-case statement lists are not blocks. A blank line is any run of
//! whitespace containing two or more newlines; spaces and tabs do not matter.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{StatementListKind, for_each_stmt_list};

#[derive(Default)]
pub struct NewlineBeforeReturn;

impl Rule for NewlineBeforeReturn {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        for_each_stmt_list(program, &mut |stmts, kind| {
            if kind == StatementListKind::Block {
                check_stmts_for_return(stmts, &mut diags, ctx);
            }
        });
        RulePreparation::Diagnostics(diags)
    }
}

fn has_blank_line_before(source: &str, pos: usize) -> bool {
    // Scan backwards from pos looking for two consecutive newlines (blank line).
    // Skips spaces and tabs between newlines.
    let bytes = &source.as_bytes()[..pos.min(source.len())];
    let mut i = bytes.len();
    let mut newlines = 0usize;
    while i > 0 {
        i -= 1;
        match bytes[i] {
            b'\n' => {
                newlines += 1;
                if newlines >= 2 {
                    return true;
                }
            }
            b' ' | b'\t' | b'\r' => {}
            _ => break,
        }
    }
    false
}

fn check_stmts_for_return(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in stmts.iter().skip(1) {
        if let Stmt::Return(ret) = stmt
            && !has_blank_line_before(ctx.source, ret.span.start)
        {
            diags.push(ctx.diagnostic(
                "newline-before-return",
                "Add a blank line before return statement",
                ret.span,
            ));
        }
    }
}
