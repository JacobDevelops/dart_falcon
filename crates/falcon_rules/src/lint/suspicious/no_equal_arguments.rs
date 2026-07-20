//! Flags an argument passed more than once in the same invocation.
//!
//! When two arguments to a call or constructor have identical source text — such
//! as `Point(x, x)` where one was meant to be `y` — the repetition is usually a
//! copy-paste slip that silently produces wrong results. Positional arguments are
//! compared against other positional arguments by source text, and named
//! arguments against other named arguments by their value expression (the label
//! is ignored); a positional is never matched against a named. Literal-valued
//! arguments are excluded, since repeating a literal like `Size(48, 48)` is
//! intentional. The report lands on the last duplicate.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoEqualArguments;

impl Rule for NoEqualArguments {
    fn name(&self) -> &'static str {
        "no-equal-arguments"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// A literal argument, per dcl semantics, is compared by identity only, so two
/// distinct literals never count as "equal". This mirrors dart_code_linter's
/// `_bothLiterals` short-circuit (`argument == arg`): passing `Size(48, 48)` or
/// `copyWith(isSaving: false, isSaved: false)` is intentional, never a bug.
/// Matches the analyzer's `Literal` hierarchy: scalar literals, collection
/// literals (`TypedLiteral`), and a prefix expression whose operand is a literal
/// (e.g. `-1`).
fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::IntLit { .. }
        | Expr::DoubleLit { .. }
        | Expr::StringLit { .. }
        | Expr::BoolLit { .. }
        | Expr::NullLit { .. }
        | Expr::List { .. }
        | Expr::Map { .. }
        | Expr::Set { .. } => true,
        Expr::Unary { operand, .. } => is_literal(operand),
        _ => false,
    }
}

/// Report the *last* occurrence of each duplicated argument, matching dcl's
/// `lastAppearance` behaviour. Reporting on the last (not first) occurrence is
/// what lets hand-written `// falcon-ignore lint/suspicious/no-equal-arguments`
/// comments — which developers place on the trailing duplicate — line up and
/// suppress the hit.
fn check_args(args: &ArgList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    // Positional args match other positional args by full source text; named
    // args match other named args by their *value* expression text (the label
    // is ignored). dcl never matches a positional against a named argument.
    // Literal-valued arguments are excluded entirely (compared by identity).
    let positional: Vec<(&str, &Span)> = args
        .positional
        .iter()
        .filter(|a| !is_literal(a))
        .map(|a| (expr_src(a, ctx.source), a.span()))
        .collect();
    let named: Vec<(&str, &Span)> = args
        .named
        .iter()
        .filter(|n| !is_literal(&n.value))
        .map(|n| (expr_src(&n.value, ctx.source), &n.span))
        .collect();

    for group in [&positional, &named] {
        report_duplicates(group, diags, ctx);
    }
}

fn report_duplicates(entries: &[(&str, &Span)], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for (i, (src, _)) in entries.iter().enumerate() {
        // Index of the last entry equal to this one.
        let last = entries
            .iter()
            .rposition(|(other, _)| other == src)
            .unwrap_or(i);
        // Only the *earlier* duplicates trigger a report, and the report lands
        // on the last occurrence. Emit once per group by acting on the first
        // member only.
        if last != i && entries[..i].iter().all(|(other, _)| other != src) {
            let span = entries[last].1;
            diags.push(Diagnostic::new(
                "no-equal-arguments",
                Severity::Warning,
                "The argument has already been passed",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

fn expr_src<'a>(expr: &Expr, source: &'a str) -> &'a str {
    let span = expr.span();
    let end = span.end.min(source.len());
    &source[span.start..end]
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
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        Stmt::Labeled(l) => scan_stmt(&l.stmt, diags, ctx),
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
