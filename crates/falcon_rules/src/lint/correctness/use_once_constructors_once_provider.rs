//! Flags providers and constructors that should be instantiated once. Ported from pyramid_lint's `use_once_constructors_once_provider`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UseOnceConstructorsOnceProvider;

impl Rule for UseOnceConstructorsOnceProvider {
    fn name(&self) -> &'static str {
        "use-once-constructors-once-provider"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

const ONCE_PROVIDERS: &[&str] = &["OnceProvider", "FutureProvider", "StateProvider"];

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Variable(v) => {
            for d in &v.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
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
    match member {
        ClassMember::Method(m) => {
            if let Some(body) = &m.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Constructor(c) => {
            if let Some(body) = &c.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Getter(g) => {
            if let Some(body) = &g.body {
                scan_body(body, diags, ctx);
            }
        }
        ClassMember::Setter(s) => {
            if let Some(body) = &s.body {
                scan_body(body, diags, ctx);
            }
        }
        _ => {}
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
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                scan_stmts(&case.body, diags, ctx);
            }
        }
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx);
            }
        }
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::New { .. } => {
            // Skip checking New expressions here; they'll be caught if used as direct calls
            // Don't recurse into New expressions to avoid false positives
        }
        Expr::Call { callee, args, .. } => {
            check_once_provider_call(callee, expr, args, diags, ctx);
            // Don't scan the callee recursively to avoid false positives on .once() calls
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        Expr::Field { object, .. } => {
            scan_expr(object, diags, ctx);
        }
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::PostfixIncDec { operand, .. } => scan_expr(operand, diags, ctx),
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
        Expr::Assign { value, .. } => scan_expr(value, diags, ctx),
        Expr::Is { expr: e, .. } => scan_expr(e, diags, ctx),
        Expr::As { expr: e, .. } => scan_expr(e, diags, ctx),
        Expr::Cascade {
            object, sections, ..
        } => {
            scan_expr(object, diags, ctx);
            for section in sections {
                scan_cascade_section(section, diags, ctx);
            }
        }
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx);
            scan_expr(index, diags, ctx);
        }
        Expr::Await { expr: e, .. } => scan_expr(e, diags, ctx),
        Expr::Throw { expr: e, .. } => scan_expr(e, diags, ctx),
        Expr::NullAssert { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::Switch { subject, arms, .. } => {
            scan_expr(subject, diags, ctx);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    scan_expr(guard, diags, ctx);
                }
                scan_expr(&arm.body, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_cascade_section(
    section: &CascadeSection,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match &section.op {
        CascadeOp::Field(_, _) => {}
        CascadeOp::Index(idx_expr, _) => {
            scan_expr(idx_expr, diags, ctx);
        }
        CascadeOp::Call(_, _, args) => {
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        CascadeOp::Assign(_, _, value) => {
            scan_expr(value, diags, ctx);
        }
    }
}

/// Check if a method call (e.g., `OnceProvider.once(...)` or `OnceProvider(...)`) is properly using `.once()`
fn check_once_provider_call(
    callee: &Expr,
    expr: &Expr,
    _args: &ArgList,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    // Check if this is calling a provider without .once()
    match callee {
        // Direct call: OnceProvider(...)
        Expr::Ident(id) => {
            if is_once_provider_name(&id.name) {
                flag_expr(expr, diags, ctx);
            }
        }
        // Method call: OnceProvider.once(...) or OnceProvider<T>.once(...)
        Expr::Field { field, .. }
            // If it's calling .once(), it's OK, don't flag
            if field.name != "once" => {
                // Otherwise check if this is a provider being called without .once()
                // For now, we assume if it's a Field call, it's either .once() (which is correct)
                // or something else (which we don't need to flag)
            }
        _ => {}
    }
}

fn is_once_provider_name(name: &str) -> bool {
    ONCE_PROVIDERS.contains(&name)
}

fn flag_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let span = expr.span();
    diags.push(Diagnostic::new(
        "use-once-constructors-once-provider",
        Severity::Warning,
        "OnceProvider and similar Riverpod providers should use .once() factory method",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
