//! Flags a local variable that is returned on the immediately following statement.
//!
//! Assigning a value to a variable only to return it on the next line adds an
//! intermediate name that carries no information; return the expression
//! directly. The rule fires when a single-declarator local variable declaration
//! is immediately followed by a `return` of that same variable, and it descends
//! into blocks and `if`, loop, and try/catch bodies to check nested statements.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferImmediateReturn;

impl Rule for PreferImmediateReturn {
    fn name(&self) -> &'static str {
        "prefer-immediate-return"
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
    // Check pairs: LocalVar immediately followed by Return of that var
    for i in 0..stmts.len().saturating_sub(1) {
        if let Stmt::LocalVar(local) = &stmts[i]
            && local.declarators.len() == 1
        {
            let var_name = &local.declarators[0].name.name;
            if let Stmt::Return(ret) = &stmts[i + 1]
                && let Some(Expr::Ident(ident)) = &ret.value
                && &ident.name == var_name
            {
                diags.push(Diagnostic::new(
                    "prefer-immediate-return",
                    Severity::Warning,
                    "Prefer returning the value directly instead of assigning to a variable first",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: local.span.start,
                        end: local.span.end,
                    },
                ));
            }
        }
    }

    // Recurse into nested blocks
    for stmt in stmts {
        match stmt {
            Stmt::Block(block) => check_stmts(&block.stmts, diags, ctx),
            Stmt::If(if_stmt) => {
                check_nested_stmt(&if_stmt.then_branch, diags, ctx);
                if let Some(else_b) = &if_stmt.else_branch {
                    check_nested_stmt(else_b, diags, ctx);
                }
            }
            Stmt::For(for_stmt) => check_nested_stmt(&for_stmt.body, diags, ctx),
            Stmt::While(s) => check_nested_stmt(&s.body, diags, ctx),
            Stmt::DoWhile(s) => check_nested_stmt(&s.body, diags, ctx),
            Stmt::TryCatch(s) => {
                check_stmts(&s.body.stmts, diags, ctx);
                for catch in &s.catches {
                    check_stmts(&catch.body.stmts, diags, ctx);
                }
            }
            Stmt::Switch(s) => {
                for case in &s.cases {
                    check_stmts(&case.body, diags, ctx);
                }
            }
            Stmt::LocalFunc(lf) => check_body(&lf.body, diags, ctx),
            _ => {}
        }
    }
}

fn check_nested_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Stmt::Block(block) = stmt {
        check_stmts(&block.stmts, diags, ctx);
    }
}
