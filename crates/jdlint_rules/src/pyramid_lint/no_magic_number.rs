//! pyramid_lint `no_magic_number`: flag numeric literals other than the allowed
//! set (0, 1, 2, -1). Top-level `const` declarations are exempt — they are the
//! named-constant definitions that magic numbers should be extracted into.

use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct NoMagicNumber;

impl Rule for NoMagicNumber {
    fn name(&self) -> &'static str {
        "no_magic_number"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// `0`, `1`, `2` are allowed. `-1` parses as a unary negation of the literal `1`,
/// so the inner literal is already covered.
fn is_allowed(value: &str) -> bool {
    matches!(value, "0" | "1" | "2")
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "no_magic_number",
        Severity::Warning,
        "Avoid magic numbers. Extract this value into a named constant.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan { start: span.start, end: span.end },
    ));
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        // Top-level `const` declarations are the canonical place for literals.
        TopLevelDecl::Variable(v) if v.is_const => {}
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
    match member {
        ClassMember::Field(f) => {
            for d in &f.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        ClassMember::Method(m) => {
            if let Some(b) = &m.body {
                scan_body(b, diags, ctx);
            }
        }
        ClassMember::Constructor(c) => {
            if let Some(b) = &c.body {
                scan_body(b, diags, ctx);
            }
        }
        ClassMember::Getter(g) => {
            if let Some(b) = &g.body {
                scan_body(b, diags, ctx);
            }
        }
        ClassMember::Setter(s) => {
            if let Some(b) = &s.body {
                scan_body(b, diags, ctx);
            }
        }
        ClassMember::Operator(o) => {
            if let Some(b) = &o.body {
                scan_body(b, diags, ctx);
            }
        }
        ClassMember::Error(_) => {}
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => b.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx)),
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Block(b) => b.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx)),
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx);
            }
        }
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        Stmt::Yield(y) => scan_expr(&y.value, diags, ctx),
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        Stmt::If(i) => {
            if let IfCondition::Expr(c) = &i.condition {
                scan_expr(c, diags, ctx);
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
            match &f.init {
                Some(ForInit::VarDecl(lv)) => {
                    for d in &lv.declarators {
                        if let Some(init) = &d.initializer {
                            scan_expr(init, diags, ctx);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => scan_expr(iterable, diags, ctx),
                Some(ForInit::Exprs(es)) => es.iter().for_each(|e| scan_expr(e, diags, ctx)),
                None => {}
            }
            f.update.iter().for_each(|e| scan_expr(e, diags, ctx));
            scan_stmt(&f.body, diags, ctx);
        }
        Stmt::Switch(sw) => {
            scan_expr(&sw.subject, diags, ctx);
            for case in &sw.cases {
                case.body.iter().for_each(|s| scan_stmt(s, diags, ctx));
            }
        }
        Stmt::TryCatch(tc) => {
            tc.body.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx));
            for catch in &tc.catches {
                catch.body.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx));
            }
            if let Some(fin) = &tc.finally {
                fin.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx));
            }
        }
        Stmt::Assert(a) => {
            scan_expr(&a.condition, diags, ctx);
            if let Some(m) = &a.message {
                scan_expr(m, diags, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::IntLit { value, span } | Expr::DoubleLit { value, span } => {
            if !is_allowed(value) {
                flag(span, diags, ctx);
            }
        }
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::PostfixIncDec { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx);
            scan_expr(value, diags, ctx);
        }
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            scan_expr(condition, diags, ctx);
            scan_expr(then_expr, diags, ctx);
            scan_expr(else_expr, diags, ctx);
        }
        Expr::Is { expr, .. } => scan_expr(expr, diags, ctx),
        Expr::As { expr, .. } => scan_expr(expr, diags, ctx),
        Expr::Field { object, .. } => scan_expr(object, diags, ctx),
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx);
            scan_expr(index, diags, ctx);
        }
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, ctx);
            scan_args(args, diags, ctx);
        }
        Expr::Cascade { object, sections, .. } => {
            scan_expr(object, diags, ctx);
            for s in sections {
                match &s.op {
                    CascadeOp::Index(e, _) => scan_expr(e, diags, ctx),
                    CascadeOp::Call(_, _, args) => scan_args(args, diags, ctx),
                    CascadeOp::Assign(t, _, v) => {
                        scan_expr(t, diags, ctx);
                        scan_expr(v, diags, ctx);
                    }
                    CascadeOp::Field(_, _) => {}
                }
            }
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            for e in elements {
                scan_collection_element(e, diags, ctx);
            }
        }
        Expr::Map { entries, .. } => {
            for entry in entries {
                scan_expr(&entry.key, diags, ctx);
                scan_expr(&entry.value, diags, ctx);
            }
        }
        Expr::Record { fields, .. } => fields.iter().for_each(|f| scan_expr(&f.value, diags, ctx)),
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::New { args, .. } => scan_args(args, diags, ctx),
        Expr::Await { expr, .. } => scan_expr(expr, diags, ctx),
        Expr::Throw { expr, .. } => scan_expr(expr, diags, ctx),
        Expr::NullAssert { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::Switch { subject, arms, .. } => {
            scan_expr(subject, diags, ctx);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    scan_expr(g, diags, ctx);
                }
                scan_expr(&arm.body, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_args(args: &ArgList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for a in &args.positional {
        scan_expr(a, diags, ctx);
    }
    for n in &args.named {
        scan_expr(&n.value, diags, ctx);
    }
}

fn scan_collection_element(el: &CollectionElement, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match el {
        CollectionElement::Expr(e) => scan_expr(e, diags, ctx),
        CollectionElement::Spread { expr, .. } => scan_expr(expr, diags, ctx),
        CollectionElement::If { then_elem, else_elem, .. } => {
            scan_collection_element(then_elem, diags, ctx);
            if let Some(ee) = else_elem {
                scan_collection_element(ee, diags, ctx);
            }
        }
        CollectionElement::For { iterable, element, .. } => {
            scan_expr(iterable, diags, ctx);
            scan_collection_element(element, diags, ctx);
        }
    }
}
