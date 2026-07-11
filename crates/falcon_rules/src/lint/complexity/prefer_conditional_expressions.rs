//! Flags `if`/`else` that could be a conditional expression. Ported from dart_code_linter's `prefer-conditional-expressions`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferConditionalExpressions;

impl Rule for PreferConditionalExpressions {
    fn name(&self) -> &'static str {
        "prefer-conditional-expressions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    if let Some(body) = &func.body {
                        check_body(body, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Class(class) => {
                    for member in &class.members {
                        check_member(member, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Mixin(mixin) => {
                    for member in &mixin.members {
                        check_member(member, &mut diags, ctx);
                    }
                }
                TopLevelDecl::MixinClass(mc) => {
                    for member in &mc.members {
                        check_member(member, &mut diags, ctx);
                    }
                }
                _ => {}
            }
        }
        diags
    }
}

fn check_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        ClassMember::Operator(o) => o.body.as_ref(),
        _ => None,
    };
    if let Some(body) = body {
        check_body(body, diags, ctx);
    }
}

fn check_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let FunctionBody::Block(block) = body {
        for stmt in &block.stmts {
            check_stmt(stmt, false, diags, ctx);
        }
    }
}

/// Unwrap a block that contains exactly one statement, recursively, mirroring
/// dart_code_linter's single-statement branch handling.
fn unwrap_single(stmt: &Stmt) -> &Stmt {
    match stmt {
        Stmt::Block(b) if b.stmts.len() == 1 => unwrap_single(&b.stmts[0]),
        _ => stmt,
    }
}

fn is_return(stmt: &Stmt) -> bool {
    matches!(unwrap_single(stmt), Stmt::Return(_))
}

/// The simple-identifier target of a single assignment statement, if any.
fn assign_target_name(stmt: &Stmt) -> Option<&str> {
    if let Stmt::Expr(e) = unwrap_single(stmt)
        && let Expr::Assign { target, .. } = &e.expr
        && let Expr::Ident(id) = target.as_ref()
    {
        return Some(&id.name);
    }
    None
}

fn check_stmt(stmt: &Stmt, parent_is_if: bool, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::If(if_stmt) => {
            if let Some(else_branch) = &if_stmt.else_branch {
                // dcl only reports the outermost `if` of a chain (parent not an
                // `if`) whose else is not itself an `else if`, and where both
                // branches reduce to a single assignment (to the same target)
                // or a single return.
                let else_is_if = matches!(else_branch.as_ref(), Stmt::If(_));
                if !parent_is_if && !else_is_if {
                    let both_return = is_return(&if_stmt.then_branch) && is_return(else_branch);
                    let both_assign = matches!(
                        (
                            assign_target_name(&if_stmt.then_branch),
                            assign_target_name(else_branch),
                        ),
                        (Some(a), Some(b)) if a == b
                    );
                    if both_return || both_assign {
                        diags.push(Diagnostic::new(
                            "prefer-conditional-expressions",
                            Severity::Warning,
                            "Prefer a conditional expression over an if/else with a single statement in each branch",
                            ctx.file_path.to_string_lossy().into_owned(),
                            DiagSpan { start: if_stmt.span.start, end: if_stmt.span.end },
                        ));
                    }
                }
            }
            // A then-branch or else-branch statement has the `if` as its parent.
            check_stmt(&if_stmt.then_branch, true, diags, ctx);
            if let Some(else_b) = &if_stmt.else_branch {
                check_stmt(else_b, true, diags, ctx);
            }
        }
        Stmt::Block(block) => {
            for s in &block.stmts {
                check_stmt(s, false, diags, ctx);
            }
        }
        Stmt::For(for_stmt) => check_stmt(&for_stmt.body, false, diags, ctx),
        Stmt::While(s) => check_stmt(&s.body, false, diags, ctx),
        Stmt::DoWhile(s) => check_stmt(&s.body, false, diags, ctx),
        Stmt::TryCatch(s) => {
            for st in &s.body.stmts {
                check_stmt(st, false, diags, ctx);
            }
            for catch in &s.catches {
                for st in &catch.body.stmts {
                    check_stmt(st, false, diags, ctx);
                }
            }
            if let Some(fin) = &s.finally {
                for st in &fin.stmts {
                    check_stmt(st, false, diags, ctx);
                }
            }
        }
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                for st in &case.body {
                    check_stmt(st, false, diags, ctx);
                }
            }
        }
        _ => {}
    }
}
