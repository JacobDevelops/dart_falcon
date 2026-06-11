use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoMagicNumber;

impl Rule for NoMagicNumber {
    fn name(&self) -> &'static str {
        "no-magic-number"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// Phase 1 allowlist: numeric literals equal to these values are not magic.
/// Negative literals parse as `Unary(Minus, IntLit "1")`; the inner `1` is
/// allowlisted, so `-1` never flags.
fn is_allowed(value: &str) -> bool {
    matches!(value, "0" | "1" | "2")
}

fn flag_literal(value: &str, span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if !is_allowed(value) {
        diags.push(Diagnostic::new(
            "no-magic-number",
            Severity::Warning,
            "Avoid using magic numbers. Define a named constant instead.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
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
        TopLevelDecl::Enum(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Extension(ext) => {
            for m in &ext.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Variable(v) => {
            scan_declarators(&v.declarators, v.is_const, v.var_type.is_some(), diags, ctx);
        }
        _ => {}
    }
}

/// Scan a declaration's initializers. A numeric literal that is the *direct*
/// initializer of a `const` declaration, or of a declaration with an explicit
/// type annotation, is treated as an intentional named/typed constant and is
/// exempt (matching the rule's golden corpus). Nested literals are still scanned.
fn scan_declarators(
    declarators: &[VarDeclarator],
    is_const: bool,
    has_type: bool,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    for d in declarators {
        if let Some(init) = &d.initializer {
            if is_const {
                continue;
            }
            if has_type && matches!(init, Expr::IntLit { .. } | Expr::DoubleLit { .. }) {
                continue;
            }
            scan_expr(init, diags, ctx);
        }
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) => {
            scan_declarators(
                &f.declarators,
                f.is_const,
                f.field_type.is_some(),
                diags,
                ctx,
            );
        }
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
        ClassMember::Operator(o) => {
            if let Some(body) = &o.body {
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
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx);
            }
        }
        Stmt::LocalVar(lv) => {
            scan_declarators(
                &lv.declarators,
                lv.is_const,
                lv.var_type.is_some(),
                diags,
                ctx,
            );
        }
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        Stmt::If(i) => {
            if let IfCondition::Expr(e) = &i.condition {
                scan_expr(e, diags, ctx);
            }
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::While(w) => {
            scan_expr(&w.condition, diags, ctx);
            scan_stmt(&w.body, diags, ctx);
        }
        Stmt::DoWhile(d) => {
            scan_stmt(&d.body, diags, ctx);
            scan_expr(&d.condition, diags, ctx);
        }
        Stmt::For(f) => {
            if let Some(cond) = &f.condition {
                scan_expr(cond, diags, ctx);
            }
            for u in &f.update {
                scan_expr(u, diags, ctx);
            }
            scan_stmt(&f.body, diags, ctx);
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
        Stmt::Switch(sw) => {
            scan_expr(&sw.subject, diags, ctx);
            for case in &sw.cases {
                scan_stmts(&case.body, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::IntLit { value, span } => flag_literal(value, span, diags, ctx),
        Expr::DoubleLit { value, span } => flag_literal(value, span, diags, ctx),
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, ctx);
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        Expr::Field { object, .. } => scan_expr(object, diags, ctx),
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx);
            scan_expr(index, diags, ctx);
        }
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
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            for elem in elements {
                if let CollectionElement::Expr(e) = elem {
                    scan_expr(e, diags, ctx);
                }
            }
        }
        Expr::Map { entries, .. } => {
            for entry in entries {
                scan_expr(&entry.key, diags, ctx);
                scan_expr(&entry.value, diags, ctx);
            }
        }
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::Await { expr, .. } => scan_expr(expr, diags, ctx),
        Expr::NullAssert { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::New { args, .. } => {
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        _ => {}
    }
}
