use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

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
        _ => None,
    };
    if let Some(body) = body {
        check_body(body, diags, ctx);
    }
}

fn check_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let FunctionBody::Block(block) = body {
        check_stmts(&block.stmts, diags, ctx);
    }
}

fn check_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in stmts {
        check_stmt(stmt, diags, ctx);
    }
}

fn stmt_count(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::Block(block) => block.stmts.len(),
        _ => 1,
    }
}

fn check_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::If(if_stmt) => {
            if let Some(else_branch) = &if_stmt.else_branch {
                // Only flag if both branches have exactly 1 statement
                if stmt_count(&if_stmt.then_branch) == 1 && stmt_count(else_branch) == 1 {
                    diags.push(Diagnostic::new(
                        "prefer-conditional-expressions",
                        Severity::Warning,
                        "Prefer a conditional expression over an if/else with a single statement in each branch",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan { start: if_stmt.span.start, end: if_stmt.span.end },
                    ));
                }
            }
            // Recurse into branches
            check_stmt(&if_stmt.then_branch, diags, ctx);
            if let Some(else_b) = &if_stmt.else_branch {
                check_stmt(else_b, diags, ctx);
            }
        }
        Stmt::Block(block) => check_stmts(&block.stmts, diags, ctx),
        Stmt::For(for_stmt) => check_stmt(&for_stmt.body, diags, ctx),
        Stmt::While(s) => check_stmt(&s.body, diags, ctx),
        Stmt::DoWhile(s) => check_stmt(&s.body, diags, ctx),
        Stmt::TryCatch(s) => {
            check_stmts(&s.body.stmts, diags, ctx);
            for catch in &s.catches {
                check_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &s.finally {
                check_stmts(&fin.stmts, diags, ctx);
            }
        }
        _ => {}
    }
}
