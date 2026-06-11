use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NewlineBeforeReturn;

impl Rule for NewlineBeforeReturn {
    fn name(&self) -> &'static str {
        "newline-before-return"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
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
            diags.push(Diagnostic::new(
                "newline-before-return",
                Severity::Warning,
                "Add a blank line before return statement",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: ret.span.start,
                    end: ret.span.end,
                },
            ));
        }
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx);
            }
        }
        TopLevelDecl::Class(c) => {
            for m in &c.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Mixin(m) => {
            for mem in &m.members {
                scan_member(mem, diags, ctx);
            }
        }
        TopLevelDecl::MixinClass(mc) => {
            for m in &mc.members {
                scan_member(m, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    };
    if let Some(b) = body {
        scan_body(b, diags, ctx);
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => {
            check_stmts_for_return(&b.stmts, diags, ctx);
            scan_stmts(&b.stmts, diags, ctx);
        }
        FunctionBody::Arrow(_, _) | FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for s in stmts {
        scan_stmt(s, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Block(b) => {
            check_stmts_for_return(&b.stmts, diags, ctx);
            scan_stmts(&b.stmts, diags, ctx);
        }
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
        Stmt::TryCatch(tc) => {
            check_stmts_for_return(&tc.body.stmts, diags, ctx);
            scan_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                check_stmts_for_return(&catch.body.stmts, diags, ctx);
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                check_stmts_for_return(&fin.stmts, diags, ctx);
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}
