//! Flags a comparison between a boolean value and a boolean literal, e.g. `x == true`.
//!
//! Comparing a known boolean to `true`/`false` is redundant: `x == true` is
//! just `x`, and `x == false` is `!x`. The rule flags an `==`/`!=` where one
//! side is a boolean literal and the other is provably a non-nullable boolean —
//! either syntactically (a literal, `!`, an `is` check, or a comparison/logical
//! operator) or a local or parameter whose inferred static type is a
//! non-nullable `bool`. A `bool?` operand is deliberately left alone, because
//! `x == true` is the correct null-safe way to test a nullable boolean.

use falcon_analyze::{AnalyzeContext, LocalTypes, Rule, StaticType};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct NoBooleanLiteralCompare;

impl Rule for NoBooleanLiteralCompare {
    fn name(&self) -> &'static str {
        "no-boolean-literal-compare"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// An expression whose result is *provably* a boolean without type resolution:
/// a literal, a negation, an `is`/`is!` check, or a comparison/logical binary.
/// Identifiers, member accesses and calls are excluded — their nullability is
/// unknowable *syntactically*, and `x == true` is the correct null-safe form for
/// a `bool?`. The [`LocalTypes`] pass in [`scan_expr`] widens this to also flag
/// operands whose inferred static type is a *non-nullable* `bool` (declared
/// locals/params), while still exempting `bool?`.
fn is_known_bool(expr: &Expr) -> bool {
    match expr {
        Expr::BoolLit { .. } => true,
        Expr::Unary {
            op: UnaryOp::Bang, ..
        } => true,
        Expr::Is { .. } => true,
        Expr::Binary { op, .. } => matches!(
            op,
            BinaryOp::EqEq
                | BinaryOp::NotEq
                | BinaryOp::Lt
                | BinaryOp::Gt
                | BinaryOp::LtEq
                | BinaryOp::GtEq
                | BinaryOp::And
                | BinaryOp::Or
        ),
        _ => false,
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                let mut lt = LocalTypes::new();
                lt.bind_params(&f.params);
                scan_body(body, diags, &mut lt, ctx);
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
                    let mut lt = LocalTypes::new();
                    scan_expr(init, diags, &mut lt, ctx);
                }
            }
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let mut lt = LocalTypes::new();
    let body = match member {
        ClassMember::Method(m) => {
            lt.bind_params(&m.params);
            m.body.as_ref()
        }
        ClassMember::Constructor(c) => {
            lt.bind_params(&c.params);
            c.body.as_ref()
        }
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => {
            let ty = s
                .param_type
                .as_ref()
                .map(StaticType::from_dart_type)
                .unwrap_or(StaticType::Unknown);
            lt.declare(s.param.name.clone(), ty);
            s.body.as_ref()
        }
        _ => None,
    };
    if let Some(b) = body {
        scan_body(b, diags, &mut lt, ctx);
    }
}

fn scan_body(
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    lt: &mut LocalTypes,
    ctx: &AnalyzeContext,
) {
    match body {
        FunctionBody::Block(b) => {
            lt.push_scope();
            scan_stmts(&b.stmts, diags, lt, ctx);
            lt.pop_scope();
        }
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, lt, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmts(
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    lt: &mut LocalTypes,
    ctx: &AnalyzeContext,
) {
    for s in stmts {
        scan_stmt(s, diags, lt, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, lt: &mut LocalTypes, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Expr(e) => scan_expr(&e.expr, diags, lt, ctx),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, lt, ctx);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, lt, ctx);
                }
            }
            lt.declare_local(lv);
        }
        Stmt::PatternDecl(pd) => {
            scan_expr(&pd.init, diags, lt, ctx);
            lt.bind_pattern(&pd.pattern);
        }
        Stmt::Block(b) => {
            lt.push_scope();
            scan_stmts(&b.stmts, diags, lt, ctx);
            lt.pop_scope();
        }
        Stmt::If(i) => {
            lt.push_scope();
            match &i.condition {
                IfCondition::Expr(e) => scan_expr(e, diags, lt, ctx),
                IfCondition::Case(scrutinee, pattern, guard) => {
                    scan_expr(scrutinee, diags, lt, ctx);
                    lt.bind_pattern(pattern);
                    if let Some(g) = guard {
                        scan_expr(g, diags, lt, ctx);
                    }
                }
            }
            scan_stmt(&i.then_branch, diags, lt, ctx);
            lt.pop_scope();
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, lt, ctx);
            }
        }
        Stmt::While(w) => {
            scan_expr(&w.condition, diags, lt, ctx);
            scan_stmt(&w.body, diags, lt, ctx);
        }
        Stmt::DoWhile(d) => {
            scan_stmt(&d.body, diags, lt, ctx);
            scan_expr(&d.condition, diags, lt, ctx);
        }
        Stmt::For(f) => {
            lt.push_scope();
            if let Some(init) = &f.init {
                if let ForInit::VarDecl(lv) = init {
                    for d in &lv.declarators {
                        if let Some(e) = &d.initializer {
                            scan_expr(e, diags, lt, ctx);
                        }
                    }
                }
                lt.bind_for_init(init);
            }
            if let Some(cond) = &f.condition {
                scan_expr(cond, diags, lt, ctx);
            }
            for u in &f.update {
                scan_expr(u, diags, lt, ctx);
            }
            scan_stmt(&f.body, diags, lt, ctx);
            lt.pop_scope();
        }
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, lt, ctx);
            for catch in &tc.catches {
                lt.push_scope();
                lt.bind_catch(catch);
                scan_stmts(&catch.body.stmts, diags, lt, ctx);
                lt.pop_scope();
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, lt, ctx);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, lt, ctx),
        Stmt::Assert(a) => {
            scan_expr(&a.condition, diags, lt, ctx);
            if let Some(msg) = &a.message {
                scan_expr(msg, diags, lt, ctx);
            }
        }
        Stmt::Throw(t) => scan_expr(&t.value, diags, lt, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, lt: &mut LocalTypes, ctx: &AnalyzeContext) {
    match expr {
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            if matches!(op, BinaryOp::EqEq | BinaryOp::NotEq) {
                let left_bool = matches!(left.as_ref(), Expr::BoolLit { .. });
                let right_bool = matches!(right.as_ref(), Expr::BoolLit { .. });
                // Flag only when the *other* operand is a boolean expression we can
                // trust to be non-nullable: either provably-boolean syntactically
                // (`is_known_bool`) or a local/param whose inferred static type is a
                // non-nullable `bool`. A `bool?` operand resolves to a *nullable*
                // bool and is deliberately not flagged — `x == true` is its
                // idiomatic null-safe form.
                let other = if left_bool {
                    Some(right.as_ref())
                } else if right_bool {
                    Some(left.as_ref())
                } else {
                    None
                };
                if other.is_some_and(|e| is_known_bool(e) || lt.of_expr(e).is_non_nullable_bool()) {
                    diags.push(Diagnostic::new(
                        "no-boolean-literal-compare",
                        Severity::Warning,
                        "Avoid comparing boolean values to boolean literals",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
            }
            scan_expr(left, diags, lt, ctx);
            scan_expr(right, diags, lt, ctx);
        }
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, lt, ctx);
            for arg in &args.positional {
                scan_expr(arg, diags, lt, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, lt, ctx);
            }
        }
        Expr::Field { object, .. } => scan_expr(object, diags, lt, ctx),
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, lt, ctx);
            scan_expr(value, diags, lt, ctx);
            // Track the reassignment so a later comparison sees the current type.
            if let Expr::Ident(id) = target.as_ref() {
                let ty = lt.of_expr(value);
                lt.reassign(&id.name, ty);
            }
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            scan_expr(condition, diags, lt, ctx);
            scan_expr(then_expr, diags, lt, ctx);
            scan_expr(else_expr, diags, lt, ctx);
        }
        Expr::FuncExpr { body, .. } => scan_body(body, diags, lt, ctx),
        Expr::Await { expr, .. } => scan_expr(expr, diags, lt, ctx),
        _ => {}
    }
}
