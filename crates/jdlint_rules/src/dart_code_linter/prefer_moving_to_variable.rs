use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct PreferMovingToVariable;

impl Rule for PreferMovingToVariable {
    fn name(&self) -> &'static str {
        "prefer-moving-to-variable"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn expr_src<'a>(expr: &Expr, source: &'a str) -> &'a str {
    let span = expr.span();
    let end = span.end.min(source.len());
    &source[span.start..end]
}

fn check_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let mut seen: Vec<(&str, &Span)> = Vec::new();

    for stmt in stmts {
        if let Stmt::LocalVar(lv) = stmt {
            for decl in &lv.declarators {
                if let Some(init) = &decl.initializer {
                    // Skip trivial literals — only flag expressions worth extracting
                    if is_trivial(init) {
                        continue;
                    }
                    let src = expr_src(init, ctx.source);
                    if seen.iter().any(|(s, _)| *s == src) {
                        let span = init.span();
                        diags.push(Diagnostic::new(
                            "prefer-moving-to-variable",
                            Severity::Warning,
                            "Duplicate expression — extract to a shared variable",
                            ctx.file_path.to_string_lossy().into_owned(),
                            DiagSpan { start: span.start, end: span.end },
                        ));
                    } else {
                        seen.push((src, init.span()));
                    }
                }
            }
        }
    }
}

fn is_trivial(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::StringLit { .. }
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
            | Expr::Ident(_)
    )
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
            check_stmts(&b.stmts, diags, ctx);
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
            check_stmts(&b.stmts, diags, ctx);
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
            check_stmts(&tc.body.stmts, diags, ctx);
            scan_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                check_stmts(&catch.body.stmts, diags, ctx);
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                check_stmts(&fin.stmts, diags, ctx);
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}
