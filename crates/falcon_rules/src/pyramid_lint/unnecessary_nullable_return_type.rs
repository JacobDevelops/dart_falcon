use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UnnecessaryNullableReturnType;

impl Rule for UnnecessaryNullableReturnType {
    fn name(&self) -> &'static str {
        "unnecessary_nullable_return_type"
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
        TopLevelDecl::Function(f) => check(f.return_type.as_ref(), f.body.as_ref(), diags, ctx),
        TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::MixinClass(mc) => mc.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let ClassMember::Method(m) = member {
        check(m.return_type.as_ref(), m.body.as_ref(), diags, ctx);
    }
}

fn check(
    return_type: Option<&DartType>,
    body: Option<&FunctionBody>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Some(return_type) = return_type
        && is_outer_nullable(return_type)
        && let Some(body) = body
        && all_returns_provably_non_null(body)
    {
        flag_return_type(return_type, diags, ctx);
    }
}

/// pyramid_lint keys off `returnType.question` — only the *outer* `?` matters.
/// `Future<T?>` or `List<int?>` are not themselves nullable and are ignored.
fn is_outer_nullable(ty: &DartType) -> bool {
    match ty {
        DartType::Named(nt) => nt.is_nullable,
        DartType::Function(ft) => ft.is_nullable,
        DartType::Record(rt) => rt.is_nullable,
        _ => false,
    }
}

/// True only when the body has at least one return and *every* returned value is
/// a provably non-null literal or constructor invocation. Without type
/// resolution this is the conservative analogue of pyramid_lint's
/// `type.isNullable` check: any return we cannot prove non-null (a variable,
/// call, `await`, bare `return;`, …) suppresses the report.
fn all_returns_provably_non_null(body: &FunctionBody) -> bool {
    match body {
        FunctionBody::Block(b) => {
            let mut count = 0usize;
            let mut all_non_null = true;
            scan_returns(&b.stmts, &mut count, &mut all_non_null);
            count > 0 && all_non_null
        }
        FunctionBody::Arrow(e, _) => is_provably_non_null(e),
        FunctionBody::Native(_, _) => false,
    }
}

fn scan_returns(stmts: &[Stmt], count: &mut usize, all_non_null: &mut bool) {
    for stmt in stmts {
        scan_returns_stmt(stmt, count, all_non_null);
    }
}

fn scan_returns_stmt(stmt: &Stmt, count: &mut usize, all_non_null: &mut bool) {
    match stmt {
        Stmt::Return(ret) => {
            *count += 1;
            match &ret.value {
                Some(v) if is_provably_non_null(v) => {}
                _ => *all_non_null = false,
            }
        }
        Stmt::Block(b) => scan_returns(&b.stmts, count, all_non_null),
        Stmt::If(i) => {
            scan_returns_stmt(&i.then_branch, count, all_non_null);
            if let Some(eb) = &i.else_branch {
                scan_returns_stmt(eb, count, all_non_null);
            }
        }
        Stmt::While(w) => scan_returns_stmt(&w.body, count, all_non_null),
        Stmt::DoWhile(d) => scan_returns_stmt(&d.body, count, all_non_null),
        Stmt::For(f) => scan_returns_stmt(&f.body, count, all_non_null),
        Stmt::TryCatch(tc) => {
            scan_returns(&tc.body.stmts, count, all_non_null);
            for catch in &tc.catches {
                scan_returns(&catch.body.stmts, count, all_non_null);
            }
            if let Some(fin) = &tc.finally {
                scan_returns(&fin.stmts, count, all_non_null);
            }
        }
        Stmt::Switch(s) => {
            for case in &s.cases {
                scan_returns(&case.body, count, all_non_null);
            }
        }
        _ => {}
    }
}

/// Expressions whose value is provably non-null without type resolution.
fn is_provably_non_null(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::StringLit(_)
            | Expr::List { .. }
            | Expr::Map { .. }
            | Expr::Set { .. }
            | Expr::Record { .. }
            | Expr::New { .. }
    )
}

fn flag_return_type(ty: &DartType, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let span = ty.span();
    diags.push(Diagnostic::new(
        "unnecessary_nullable_return_type",
        Severity::Warning,
        "Function return type is unnecessarily nullable",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
