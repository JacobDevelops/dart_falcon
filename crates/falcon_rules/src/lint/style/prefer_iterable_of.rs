//! Flags `List.from`/`Set.from` in favor of the `.of` constructor. Ported from dart_code_linter's `prefer-iterable-of`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferIterableOf;

impl Rule for PreferIterableOf {
    fn name(&self) -> &'static str {
        "prefer-iterable-of"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn is_iterable_base(name: &str) -> bool {
    name == "List" || name == "Set" || name == "Iterable"
}

/// True for the `Expr::New` form, `new List.from(...)` / `new Set.from(...)`.
///
/// The parser may represent this two ways depending on how the named constructor binds:
///   * `dart_type = List`, `constructor_name = Some("from")`, or
///   * the `.from` is folded into the qualified type name, giving
///     `dart_type = Named { segments: [.., "List", "from"] }`, `constructor_name = None`.
fn is_from_constructor(dart_type: &DartType, constructor_name: &Option<Identifier>) -> bool {
    let DartType::Named(nt) = dart_type else {
        return false;
    };
    // Case A: `.from` kept as a separate named constructor.
    if let Some(ctor) = constructor_name
        && ctor.name == "from"
        && let Some(last) = nt.segments.last()
    {
        return is_iterable_base(&last.name);
    }
    // Case B: `.from` folded into the qualified type name -> [.., base, "from"].
    let segs = &nt.segments;
    segs.len() >= 2
        && segs.last().is_some_and(|s| s.name == "from")
        && is_iterable_base(&segs[segs.len() - 2].name)
}

/// Resolve the base type name of a receiver expression.
/// `List`            -> `Expr::Ident("List")`
/// `List<int>`       -> `Expr::GenericInstantiation { target: Ident("List"), type_args: [int], .. }`
fn base_type_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(id) => Some(id.name.as_str()),
        Expr::Call { callee, .. } => base_type_name(callee),
        Expr::GenericInstantiation { target, .. } => base_type_name(target),
        _ => None,
    }
}

/// True for `List.from(...)` / `List<int>.from(...)` parsed as `Call(Field(receiver, "from"), args)`.
fn is_from_static_call(callee: &Expr) -> bool {
    if let Expr::Field { object, field, .. } = callee
        && field.name == "from"
        && let Some(base) = base_type_name(object)
    {
        return is_iterable_base(base);
    }
    false
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "prefer-iterable-of",
        Severity::Warning,
        "Prefer using the 'of' constructor instead of 'from'.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
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
        TopLevelDecl::Variable(v) => {
            for d in &v.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Field(f) => {
            for d in &f.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
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
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
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
        Stmt::If(i) => {
            match &i.condition {
                IfCondition::Expr(e) => scan_expr(e, diags, ctx),
                IfCondition::Case(e, _, guard) => {
                    scan_expr(e, diags, ctx);
                    if let Some(g) = guard {
                        scan_expr(g, diags, ctx);
                    }
                }
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
            scan_stmt(&f.body, diags, ctx);
        }
        Stmt::Switch(sw) => {
            scan_expr(&sw.subject, diags, ctx);
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
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::New {
            is_const: _,
            dart_type,
            constructor_name,
            args,
            span,
        } => {
            if is_from_constructor(dart_type, constructor_name) {
                flag(span, diags, ctx);
            }
            // Recurse into arguments
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::Call {
            callee, args, span, ..
        } => {
            if is_from_static_call(callee) {
                flag(span, diags, ctx);
            }
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
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
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
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx);
            scan_expr(value, diags, ctx);
        }
        Expr::Await { expr: inner, .. } => scan_expr(inner, diags, ctx),
        _ => {}
    }
}
