//! pyramid_lint `no_empty_block`: forbid empty blocks (catch, method/function
//! bodies, if/else branches, loops). An empty block is usually a mistake or a
//! `TODO` left behind; if intentional, add a statement or a comment.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoEmptyBlock;

impl Rule for NoEmptyBlock {
    fn name(&self) -> &'static str {
        "no_empty_block"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "no_empty_block",
        Severity::Warning,
        "Avoid empty blocks. Add a statement or a comment explaining why it is empty.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

/// Flag the block if empty, otherwise recurse into its statements.
fn check_block(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if block.stmts.is_empty() {
        flag(&block.span, diags, ctx);
    } else {
        for s in &block.stmts {
            scan_stmt(s, diags, ctx);
        }
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let FunctionBody::Block(b) = body {
        check_block(b, diags, ctx);
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx);
            }
        }
        TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::MixinClass(mc) => mc.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Enum(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Extension(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::ExtensionType(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        ClassMember::Operator(o) => o.body.as_ref(),
        _ => None,
    };
    if let Some(b) = body {
        scan_body(b, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Block(b) => check_block(b, diags, ctx),
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                for s in &case.body {
                    scan_stmt(s, diags, ctx);
                }
            }
        }
        Stmt::TryCatch(tc) => {
            check_block(&tc.body, diags, ctx);
            for catch in &tc.catches {
                check_block(&catch.body, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                check_block(fin, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}
