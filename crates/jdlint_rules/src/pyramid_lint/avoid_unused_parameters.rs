//! pyramid_lint `avoid_unused_parameters`: flag function/method parameters that
//! are never referenced in the body. Parameters intentionally unused should be
//! named with a leading underscore (`_`). `dynamic`-typed parameters are exempt
//! (commonly required to match a callback signature).

use std::collections::HashSet;

use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct AvoidUnusedParameters;

impl Rule for AvoidUnusedParameters {
    fn name(&self) -> &'static str {
        "avoid_unused_parameters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                check_function(&f.params, body, diags, ctx);
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
    if let ClassMember::Method(m) = member
        && let Some(body) = &m.body {
            check_function(&m.params, body, diags, ctx);
        }
}

/// Check a function/method's parameters against its body, then descend into any
/// nested local functions.
fn check_function(
    params: &FormalParamList,
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let mut used: HashSet<String> = HashSet::new();
    collect_used_body(body, &mut used);

    for param in params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
    {
        let name = &param.name.name;
        if name.starts_with('_') {
            continue;
        }
        if matches!(param.param_type, Some(DartType::Dynamic { .. })) {
            continue;
        }
        if !used.contains(name) {
            diags.push(Diagnostic::new(
                "avoid_unused_parameters",
                Severity::Warning,
                "Parameter is never used. Rename it to `_` or remove it.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: param.name.span.start, end: param.name.span.end },
            ));
        }
    }

    // Nested local functions have their own parameter scope.
    if let FunctionBody::Block(b) = body {
        for s in &b.stmts {
            scan_nested_fns(s, diags, ctx);
        }
    }
}

fn scan_nested_fns(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::LocalFunc(lf) => check_function(&lf.params, &lf.body, diags, ctx),
        Stmt::Block(b) => b.stmts.iter().for_each(|s| scan_nested_fns(s, diags, ctx)),
        Stmt::If(i) => {
            scan_nested_fns(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_nested_fns(eb, diags, ctx);
            }
        }
        Stmt::While(w) => scan_nested_fns(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_nested_fns(&d.body, diags, ctx),
        Stmt::For(f) => scan_nested_fns(&f.body, diags, ctx),
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                case.body.iter().for_each(|s| scan_nested_fns(s, diags, ctx));
            }
        }
        Stmt::TryCatch(tc) => {
            tc.body.stmts.iter().for_each(|s| scan_nested_fns(s, diags, ctx));
            for catch in &tc.catches {
                catch.body.stmts.iter().for_each(|s| scan_nested_fns(s, diags, ctx));
            }
            if let Some(fin) = &tc.finally {
                fin.stmts.iter().for_each(|s| scan_nested_fns(s, diags, ctx));
            }
        }
        _ => {}
    }
}

// ── Usage collection ──────────────────────────────────────────────────────────

fn collect_used_body(body: &FunctionBody, used: &mut HashSet<String>) {
    match body {
        FunctionBody::Block(b) => b.stmts.iter().for_each(|s| collect_used_stmt(s, used)),
        FunctionBody::Arrow(e, _) => collect_used_expr(e, used),
        FunctionBody::Native(_, _) => {}
    }
}

fn collect_used_stmt(stmt: &Stmt, used: &mut HashSet<String>) {
    match stmt {
        Stmt::Block(b) => b.stmts.iter().for_each(|s| collect_used_stmt(s, used)),
        Stmt::Expr(e) => collect_used_expr(&e.expr, used),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                collect_used_expr(v, used);
            }
        }
        Stmt::Throw(t) => collect_used_expr(&t.value, used),
        Stmt::Yield(y) => collect_used_expr(&y.value, used),
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    collect_used_expr(init, used);
                }
            }
        }
        Stmt::If(i) => {
            if let IfCondition::Expr(c) = &i.condition {
                collect_used_expr(c, used);
            }
            collect_used_stmt(&i.then_branch, used);
            if let Some(eb) = &i.else_branch {
                collect_used_stmt(eb, used);
            }
        }
        Stmt::While(w) => {
            collect_used_expr(&w.condition, used);
            collect_used_stmt(&w.body, used);
        }
        Stmt::DoWhile(d) => {
            collect_used_stmt(&d.body, used);
            collect_used_expr(&d.condition, used);
        }
        Stmt::For(f) => {
            if let Some(cond) = &f.condition {
                collect_used_expr(cond, used);
            }
            match &f.init {
                Some(ForInit::VarDecl(lv)) => {
                    for d in &lv.declarators {
                        if let Some(init) = &d.initializer {
                            collect_used_expr(init, used);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => collect_used_expr(iterable, used),
                Some(ForInit::Exprs(es)) => es.iter().for_each(|e| collect_used_expr(e, used)),
                None => {}
            }
            f.update.iter().for_each(|e| collect_used_expr(e, used));
            collect_used_stmt(&f.body, used);
        }
        Stmt::Switch(sw) => {
            collect_used_expr(&sw.subject, used);
            for case in &sw.cases {
                case.body.iter().for_each(|s| collect_used_stmt(s, used));
            }
        }
        Stmt::TryCatch(tc) => {
            tc.body.stmts.iter().for_each(|s| collect_used_stmt(s, used));
            for catch in &tc.catches {
                catch.body.stmts.iter().for_each(|s| collect_used_stmt(s, used));
            }
            if let Some(fin) = &tc.finally {
                fin.stmts.iter().for_each(|s| collect_used_stmt(s, used));
            }
        }
        Stmt::Assert(a) => {
            collect_used_expr(&a.condition, used);
            if let Some(m) = &a.message {
                collect_used_expr(m, used);
            }
        }
        Stmt::LocalFunc(lf) => collect_used_body(&lf.body, used),
        _ => {}
    }
}

fn collect_used_expr(expr: &Expr, used: &mut HashSet<String>) {
    match expr {
        Expr::Ident(id) => {
            used.insert(id.name.clone());
        }
        Expr::StringLit(s) => collect_interpolation_idents(&s.raw, used),
        Expr::Unary { operand, .. } => collect_used_expr(operand, used),
        Expr::PostfixIncDec { operand, .. } => collect_used_expr(operand, used),
        Expr::Binary { left, right, .. } => {
            collect_used_expr(left, used);
            collect_used_expr(right, used);
        }
        Expr::Assign { target, value, .. } => {
            collect_used_expr(target, used);
            collect_used_expr(value, used);
        }
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            collect_used_expr(condition, used);
            collect_used_expr(then_expr, used);
            collect_used_expr(else_expr, used);
        }
        Expr::Is { expr, .. } => collect_used_expr(expr, used),
        Expr::As { expr, .. } => collect_used_expr(expr, used),
        Expr::Field { object, .. } => collect_used_expr(object, used),
        Expr::Index { object, index, .. } => {
            collect_used_expr(object, used);
            collect_used_expr(index, used);
        }
        Expr::Call { callee, args, .. } => {
            collect_used_expr(callee, used);
            collect_used_args(args, used);
        }
        Expr::Cascade { object, sections, .. } => {
            collect_used_expr(object, used);
            for s in sections {
                match &s.op {
                    CascadeOp::Index(e, _) => collect_used_expr(e, used),
                    CascadeOp::Call(_, _, args) => collect_used_args(args, used),
                    CascadeOp::Assign(t, _, v) => {
                        collect_used_expr(t, used);
                        collect_used_expr(v, used);
                    }
                    CascadeOp::Field(_, _) => {}
                }
            }
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            for e in elements {
                collect_used_collection_element(e, used);
            }
        }
        Expr::Map { entries, .. } => {
            for entry in entries {
                collect_used_expr(&entry.key, used);
                collect_used_expr(&entry.value, used);
            }
        }
        Expr::Record { fields, .. } => fields.iter().for_each(|f| collect_used_expr(&f.value, used)),
        Expr::FuncExpr { body, .. } => collect_used_body(body, used),
        Expr::New { args, .. } => collect_used_args(args, used),
        Expr::Await { expr, .. } => collect_used_expr(expr, used),
        Expr::Throw { expr, .. } => collect_used_expr(expr, used),
        Expr::NullAssert { operand, .. } => collect_used_expr(operand, used),
        Expr::Switch { subject, arms, .. } => {
            collect_used_expr(subject, used);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_used_expr(g, used);
                }
                collect_used_expr(&arm.body, used);
            }
        }
        _ => {}
    }
}

fn collect_used_args(args: &ArgList, used: &mut HashSet<String>) {
    for a in &args.positional {
        collect_used_expr(a, used);
    }
    for n in &args.named {
        collect_used_expr(&n.value, used);
    }
}

fn collect_used_collection_element(el: &CollectionElement, used: &mut HashSet<String>) {
    match el {
        CollectionElement::Expr(e) => collect_used_expr(e, used),
        CollectionElement::Spread { expr, .. } => collect_used_expr(expr, used),
        CollectionElement::If { then_elem, else_elem, .. } => {
            collect_used_collection_element(then_elem, used);
            if let Some(ee) = else_elem {
                collect_used_collection_element(ee, used);
            }
        }
        CollectionElement::For { iterable, element, .. } => {
            collect_used_expr(iterable, used);
            collect_used_collection_element(element, used);
        }
    }
}

/// Extract identifiers referenced in string interpolations (`$name`, `${...}`).
fn collect_interpolation_idents(raw: &str, used: &mut HashSet<String>) {
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            i += 1;
            if i < bytes.len() && bytes[i] == b'{' {
                // ${ ... } — collect every identifier-like token until the closing brace.
                i += 1;
                while i < bytes.len() && bytes[i] != b'}' {
                    if is_ident_start(bytes[i]) {
                        let start = i;
                        while i < bytes.len() && is_ident_continue(bytes[i]) {
                            i += 1;
                        }
                        used.insert(raw[start..i].to_string());
                    } else {
                        i += 1;
                    }
                }
            } else if i < bytes.len() && is_ident_start(bytes[i]) {
                let start = i;
                while i < bytes.len() && is_ident_continue(bytes[i]) {
                    i += 1;
                }
                used.insert(raw[start..i].to_string());
            }
        } else {
            i += 1;
        }
    }
}

fn is_ident_start(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphabetic()
}

fn is_ident_continue(b: u8) -> bool {
    b == b'_' || b.is_ascii_alphanumeric()
}
