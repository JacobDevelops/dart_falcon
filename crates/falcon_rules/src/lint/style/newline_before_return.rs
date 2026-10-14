//! Flags `return` statements that are not preceded by a blank line.
//!
//! Separating a `return` from the statements above it with a blank line makes
//! the exit point of a block visually distinct, so readers can spot where
//! control leaves the function without parsing the surrounding code. The rule
//! only checks returns that follow another statement in the same statement
//! list — the first statement of a block or switch case is never flagged,
//! since there is nothing to separate it from. A blank line is any run of
//! whitespace containing two or more newlines; spaces and tabs do not matter.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_expr, walk_function_decl, walk_stmt,
};

#[derive(Default)]
pub struct NewlineBeforeReturn;

impl Rule for NewlineBeforeReturn {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut visitor = Collector {
            diags: Vec::new(),
            ctx,
        };
        visitor.visit_program(program);
        visitor.diags
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

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Collector<'_, '_> {
    fn check_body(&mut self, body: Option<&FunctionBody>) {
        if let Some(FunctionBody::Block(block)) = body {
            check_stmts_for_return(&block.stmts, &mut self.diags, self.ctx);
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, function: &FunctionDecl) {
        self.check_body(function.body.as_ref());
        walk_function_decl(self, function);
    }

    fn visit_class_member(&mut self, member: &ClassMember) {
        self.check_body(match member {
            ClassMember::Method(m) => m.body.as_ref(),
            ClassMember::Constructor(c) => c.body.as_ref(),
            ClassMember::Getter(g) => g.body.as_ref(),
            ClassMember::Setter(s) => s.body.as_ref(),
            ClassMember::Operator(o) => o.body.as_ref(),
            _ => None,
        });
        walk_class_member(self, member);
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Block(b) => check_stmts_for_return(&b.stmts, &mut self.diags, self.ctx),
            Stmt::Switch(s) => {
                for case in &s.cases {
                    check_stmts_for_return(&case.body, &mut self.diags, self.ctx);
                }
            }
            Stmt::TryCatch(t) => {
                check_stmts_for_return(&t.body.stmts, &mut self.diags, self.ctx);
                for catch in &t.catches {
                    check_stmts_for_return(&catch.body.stmts, &mut self.diags, self.ctx);
                }
                if let Some(finally) = &t.finally {
                    check_stmts_for_return(&finally.stmts, &mut self.diags, self.ctx);
                }
            }
            Stmt::LocalFunc(f) => self.check_body(Some(&f.body)),
            _ => {}
        }
        walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &Expr) {
        if let Expr::FuncExpr { body, .. } = expr {
            self.check_body(Some(body));
        }
        walk_expr(self, expr);
    }
}
