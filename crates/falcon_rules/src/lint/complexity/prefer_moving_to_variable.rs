//! Flags repeated complex expressions that should be extracted to a variable. Ported from dart_code_linter's `prefer-moving-to-variable`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferMovingToVariable;

impl Rule for PreferMovingToVariable {
    fn name(&self) -> &'static str {
        "prefer-moving-to-variable"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let threshold = allowed_duplicated_chains(ctx);
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx, threshold);
        }
        diags
    }
}

/// Read the `allowed_duplicated_chains` option (default 2). A duplicate is
/// flagged once its 1-based occurrence index reaches this value, so the default
/// flags the 2nd (and later) occurrences; `3` flags the 3rd and later.
/// Malformed/missing → default. A value below 2 is clamped to 2 (the first
/// occurrence can never be a duplicate).
fn allowed_duplicated_chains(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("prefer-moving-to-variable")
        .and_then(|m| ctx.rule_options(m.group, "prefer-moving-to-variable"))
        .and_then(|o| o.get("allowed_duplicated_chains"))
        .and_then(|v| v.as_u64())
        .map(|v| (v as usize).max(2))
        .unwrap_or(2)
}

fn expr_src<'a>(expr: &Expr, source: &'a str) -> &'a str {
    let span = expr.span();
    let end = span.end.min(source.len());
    &source[span.start..end]
}

fn check_stmts(
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    threshold: usize,
) {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    for stmt in stmts {
        if let Stmt::LocalVar(lv) = stmt {
            for decl in &lv.declarators {
                if let Some(init) = &decl.initializer {
                    // Skip trivial literals — only flag expressions worth extracting
                    if is_trivial(init) {
                        continue;
                    }
                    let src = expr_src(init, ctx.source);
                    let occurrence = *counts.entry(src).and_modify(|c| *c += 1).or_insert(1);
                    if occurrence >= threshold {
                        let span = init.span();
                        diags.push(Diagnostic::new(
                            "prefer-moving-to-variable",
                            Severity::Warning,
                            "Duplicate expression — extract to a shared variable",
                            ctx.file_path.to_string_lossy().into_owned(),
                            DiagSpan {
                                start: span.start,
                                end: span.end,
                            },
                        ));
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

fn scan_top(
    decl: &TopLevelDecl,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    threshold: usize,
) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx, threshold);
            }
        }
        TopLevelDecl::Class(c) => {
            for m in &c.members {
                scan_member(m, diags, ctx, threshold);
            }
        }
        TopLevelDecl::Mixin(m) => {
            for mem in &m.members {
                scan_member(mem, diags, ctx, threshold);
            }
        }
        TopLevelDecl::MixinClass(mc) => {
            for m in &mc.members {
                scan_member(m, diags, ctx, threshold);
            }
        }
        _ => {}
    }
}

fn scan_member(
    member: &ClassMember,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    threshold: usize,
) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    };
    if let Some(b) = body {
        scan_body(b, diags, ctx, threshold);
    }
}

fn scan_body(
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    threshold: usize,
) {
    match body {
        FunctionBody::Block(b) => {
            check_stmts(&b.stmts, diags, ctx, threshold);
            scan_stmts(&b.stmts, diags, ctx, threshold);
        }
        FunctionBody::Arrow(_, _) | FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, threshold: usize) {
    for s in stmts {
        scan_stmt(s, diags, ctx, threshold);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, threshold: usize) {
    match stmt {
        Stmt::Block(b) => {
            check_stmts(&b.stmts, diags, ctx, threshold);
            scan_stmts(&b.stmts, diags, ctx, threshold);
        }
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx, threshold);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx, threshold);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx, threshold),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx, threshold),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx, threshold),
        Stmt::TryCatch(tc) => {
            check_stmts(&tc.body.stmts, diags, ctx, threshold);
            scan_stmts(&tc.body.stmts, diags, ctx, threshold);
            for catch in &tc.catches {
                check_stmts(&catch.body.stmts, diags, ctx, threshold);
                scan_stmts(&catch.body.stmts, diags, ctx, threshold);
            }
            if let Some(fin) = &tc.finally {
                check_stmts(&fin.stmts, diags, ctx, threshold);
                scan_stmts(&fin.stmts, diags, ctx, threshold);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx, threshold),
        _ => {}
    }
}
