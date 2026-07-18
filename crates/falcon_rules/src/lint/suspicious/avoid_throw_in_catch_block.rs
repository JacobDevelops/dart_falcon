//! Flags a `throw` of a new object inside a `catch` block.
//!
//! Throwing a fresh exception from a catch replaces the error in flight along
//! with its original stack trace, so the root cause is lost and the failure
//! becomes much harder to diagnose. Prefer `rethrow` to propagate the caught
//! exception unchanged, or wrap it in an error that keeps the original as a
//! cause. Only a `throw` statement raising a new value is reported; a `rethrow`
//! is always exempt.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidThrowInCatchBlock;

impl Rule for AvoidThrowInCatchBlock {
    fn name(&self) -> &'static str {
        "avoid-throw-in-catch-block"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for decl in &program.declarations {
            collect_diagnostics_for_decl(decl, &mut diagnostics, ctx);
        }

        diagnostics
    }
}

fn collect_diagnostics_for_decl(
    decl: &TopLevelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match decl {
        TopLevelDecl::Function(func_decl) => {
            if let Some(body) = &func_decl.body {
                check_function_body(body, diagnostics, ctx);
            }
        }
        TopLevelDecl::Class(class_decl) => {
            for member in &class_decl.members {
                check_class_member(member, diagnostics, ctx);
            }
        }
        _ => {}
    }
}

fn check_class_member(
    member: &ClassMember,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match member {
        ClassMember::Method(method_decl) => {
            if let Some(body) = &method_decl.body {
                check_function_body(body, diagnostics, ctx);
            }
        }
        ClassMember::Constructor(ctor_decl) => {
            if let Some(body) = &ctor_decl.body {
                check_function_body(body, diagnostics, ctx);
            }
        }
        ClassMember::Getter(getter_decl) => {
            if let Some(body) = &getter_decl.body {
                check_function_body(body, diagnostics, ctx);
            }
        }
        ClassMember::Setter(setter_decl) => {
            if let Some(body) = &setter_decl.body {
                check_function_body(body, diagnostics, ctx);
            }
        }
        _ => {}
    }
}

fn check_function_body(
    body: &FunctionBody,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match body {
        FunctionBody::Block(block) => {
            check_stmts(&block.stmts, diagnostics, ctx);
        }
        FunctionBody::Arrow(_expr, _) => {}
        FunctionBody::Native(..) => {}
    }
}

fn check_stmts(stmts: &[Stmt], diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in stmts {
        check_stmt(stmt, diagnostics, ctx);
    }
}

fn check_stmt(stmt: &Stmt, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::TryCatch(try_catch) => {
            check_stmts(&try_catch.body.stmts, diagnostics, ctx);

            for catch_clause in &try_catch.catches {
                check_throws_in_catch(&catch_clause.body, diagnostics, ctx);
            }

            if let Some(finally_block) = &try_catch.finally {
                check_stmts(&finally_block.stmts, diagnostics, ctx);
            }
        }
        Stmt::Block(block) => {
            check_stmts(&block.stmts, diagnostics, ctx);
        }
        Stmt::If(if_stmt) => {
            check_stmt(&if_stmt.then_branch, diagnostics, ctx);
            if let Some(else_branch) = &if_stmt.else_branch {
                check_stmt(else_branch, diagnostics, ctx);
            }
        }
        Stmt::For(for_stmt) => {
            check_stmt(&for_stmt.body, diagnostics, ctx);
        }
        Stmt::While(while_stmt) => {
            check_stmt(&while_stmt.body, diagnostics, ctx);
        }
        Stmt::DoWhile(do_while_stmt) => {
            check_stmt(&do_while_stmt.body, diagnostics, ctx);
        }
        Stmt::Switch(switch_stmt) => {
            for case in &switch_stmt.cases {
                check_stmts(&case.body, diagnostics, ctx);
            }
        }
        Stmt::LocalFunc(local_func) => {
            check_function_body(&local_func.body, diagnostics, ctx);
        }
        _ => {}
    }
}

fn check_throws_in_catch(block: &Block, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in &block.stmts {
        collect_throws(stmt, diagnostics, ctx);
    }
}

fn collect_throws(stmt: &Stmt, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Throw(throw_stmt) if !throw_stmt.is_rethrow => {
            diagnostics.push(Diagnostic::new(
                "avoid-throw-in-catch-block",
                Severity::Warning,
                "Avoid throwing exceptions within catch blocks",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: throw_stmt.span.start,
                    end: throw_stmt.span.end,
                },
            ));
        }
        Stmt::Block(block) => {
            for s in &block.stmts {
                collect_throws(s, diagnostics, ctx);
            }
        }
        Stmt::If(if_stmt) => {
            collect_throws(&if_stmt.then_branch, diagnostics, ctx);
            if let Some(else_branch) = &if_stmt.else_branch {
                collect_throws(else_branch, diagnostics, ctx);
            }
        }
        Stmt::For(for_stmt) => {
            collect_throws(&for_stmt.body, diagnostics, ctx);
        }
        Stmt::While(while_stmt) => {
            collect_throws(&while_stmt.body, diagnostics, ctx);
        }
        Stmt::DoWhile(do_while_stmt) => {
            collect_throws(&do_while_stmt.body, diagnostics, ctx);
        }
        Stmt::Switch(switch_stmt) => {
            for case in &switch_stmt.cases {
                for s in &case.body {
                    collect_throws(s, diagnostics, ctx);
                }
            }
        }
        Stmt::LocalFunc(local_func) => {
            if let FunctionBody::Block(block) = &local_func.body {
                for s in &block.stmts {
                    collect_throws(s, diagnostics, ctx);
                }
            }
        }
        Stmt::TryCatch(_) => {}
        _ => {}
    }
}
