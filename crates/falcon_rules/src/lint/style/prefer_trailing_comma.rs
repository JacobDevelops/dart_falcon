//! Flags argument and parameter lists missing a trailing comma. Ported from dart_code_linter's `prefer-trailing-comma`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferTrailingComma;

impl Rule for PreferTrailingComma {
    fn name(&self) -> &'static str {
        "prefer-trailing-comma"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn line_of(source: &str, offset: usize) -> usize {
    let c = offset.min(source.len());
    source[..c].bytes().filter(|&b| b == b'\n').count()
}

/// A trailing comma is required exactly when the argument list is already split
/// across lines — i.e. the last significant character before the closing
/// delimiter is on an earlier line than the delimiter and is not itself a comma.
///
/// This mirrors the Dart 3.x tall-style formatter, which adds a trailing comma
/// when it breaks a list one element per line (closing bracket on its own line)
/// and omits it when the final argument hugs the bracket (single trailing
/// closure, block-like argument, method chain, single-line list). Scanning the
/// source directly avoids depending on how the parser bounds argument spans.
///
/// The scan skips comments and string contents so that a trailing line comment
/// after the final comma (`foo(\n  a, // note\n)`) is not mistaken for the last
/// significant token — the comma before it still counts. It also locates the
/// real closing `)` by paren-depth matching rather than trusting `args_span.end`,
/// which the parser over-extends past the bracket into trailing trivia (e.g. the
/// newline + indentation before the next `.method` in a chain).
/// Returns the offset of the real closing `)` when a trailing comma is required,
/// or `None` otherwise.
fn needs_trailing_comma(source: &str, args_span: &Span) -> Option<usize> {
    let b = source.as_bytes();
    let end = args_span.end.min(source.len());
    // Advance to the opening `(` of the argument list.
    let mut i = args_span.start;
    while i < end && b[i] != b'(' {
        i += 1;
    }
    if i >= end {
        return None;
    }
    let mut depth = 0usize;
    let mut last_sig: Option<usize> = None;
    let mut close: Option<usize> = None;
    while i < end {
        let c = b[i];
        match c {
            b'\'' | b'"' => {
                last_sig = Some(i);
                let quote = c;
                i += 1;
                while i < end {
                    match b[i] {
                        b'\\' => i += 2,
                        q if q == quote => {
                            last_sig = Some(i);
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b'/' if i + 1 < end && b[i + 1] == b'/' => {
                while i < end && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < end && b[i + 1] == b'*' => {
                i += 2;
                while i + 1 < end && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'(' => {
                depth += 1;
                last_sig = Some(i);
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
                last_sig = Some(i);
                i += 1;
            }
            _ => {
                if !c.is_ascii_whitespace() {
                    last_sig = Some(i);
                }
                i += 1;
            }
        }
    }
    match (last_sig, close) {
        (Some(j), Some(close)) if b[j] != b',' && line_of(source, j) < line_of(source, close) => {
            Some(close)
        }
        _ => None,
    }
}

fn check_args(args: &ArgList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if args.positional.is_empty() && args.named.is_empty() {
        return;
    }
    let Some(close) = needs_trailing_comma(ctx.source, &args.span) else {
        return;
    };
    // Diagnostic points to the real closing `)`.
    let diag_pos = close;
    diags.push(Diagnostic::new(
        "prefer-trailing-comma",
        Severity::Warning,
        "Add a trailing comma to the argument list",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: diag_pos,
            end: diag_pos + 1,
        },
    ));
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
        FunctionBody::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for s in stmts {
        scan_stmt(s, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
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
            scan_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Call { callee, args, .. } => {
            check_args(args, diags, ctx);
            scan_expr(callee, diags, ctx);
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        Expr::New { args, .. } => {
            check_args(args, diags, ctx);
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Field { object, .. } => scan_expr(object, diags, ctx),
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx);
            scan_expr(value, diags, ctx);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            scan_expr(condition, diags, ctx);
            scan_expr(then_expr, diags, ctx);
            scan_expr(else_expr, diags, ctx);
        }
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::Await { expr, .. } => scan_expr(expr, diags, ctx),
        _ => {}
    }
}
