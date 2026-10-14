//! Flags repeated property/getter chains in local variable initializers.
//!
//! Only chains rooted in an identifier, `this`, or `super` are candidates;
//! calls, constructors, operators, awaits, and fresh collections must retain
//! their separate evaluations. Getters are assumed to be observational reads.
//! Initializer source text is compared within each statement list, with counts
//! reset across non-declaration statements or potentially effectful initializers
//! to avoid suggesting stale values after mutation. Late declarations are also
//! barriers: their reads are deferred. Statement lists are checked independently.
//!
//! ## Options
//!
//! `allowed_duplicated_chains` (integer, default: 2) — the occurrence index at
//! which a repeat is flagged; `2` flags the 2nd and later occurrences, `3` the
//! 3rd and later. Values below `2` are clamped to `2`.

use super::traversal::for_each_stmt_list;
use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferMovingToVariable;

impl Rule for PreferMovingToVariable {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let threshold = allowed_duplicated_chains(ctx);
        for_each_stmt_list(program, &mut |stmts| {
            check_stmts(stmts, &mut diags, ctx, threshold);
        });
        diags
    }
}

fn allowed_duplicated_chains(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "prefer-moving-to-variable"),
        "allowed_duplicated_chains",
    )
    .unwrap_or(2)
    .max(2)
}

use crate::lint::expressions::expr_src;

fn check_stmts(
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    threshold: usize,
) {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    for stmt in stmts {
        let Stmt::LocalVar(lv) = stmt else {
            counts.clear();
            continue;
        };
        if lv.is_late {
            counts.clear();
            continue;
        }
        for decl in &lv.declarators {
            let Some(init) = &decl.initializer else {
                continue;
            };
            if let Expr::Field { object, .. } = init
                && is_chain(object)
            {
                let src = expr_src(init, ctx.source);
                let occurrence = *counts.entry(src).and_modify(|c| *c += 1).or_insert(1);
                if occurrence >= threshold {
                    diags.push(ctx.diagnostic(
                        "prefer-moving-to-variable",
                        "Duplicate expression — extract to a shared variable",
                        *init.span(),
                    ));
                }
            } else if !matches!(
                init,
                Expr::IntLit { .. }
                    | Expr::DoubleLit { .. }
                    | Expr::BoolLit { .. }
                    | Expr::NullLit { .. }
                    | Expr::Ident(_)
                    | Expr::This { .. }
                    | Expr::Super { .. }
            ) && !matches!(init, Expr::StringLit(literal) if literal.interpolations.is_empty())
            {
                counts.clear();
            }
        }
    }
}

fn is_chain(mut expr: &Expr) -> bool {
    while let Expr::Field { object, .. } = expr {
        expr = object;
    }
    matches!(
        expr,
        Expr::Ident(_) | Expr::This { .. } | Expr::Super { .. }
    )
}
