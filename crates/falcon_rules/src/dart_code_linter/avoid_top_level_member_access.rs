use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use std::collections::HashSet;

pub struct AvoidTopLevelMemberAccess;

impl Rule for AvoidTopLevelMemberAccess {
    fn name(&self) -> &'static str {
        "avoid-top-level-member-access"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Collect all top-level variable declarations (non-const, non-final with certain patterns)
        let mut top_level_vars = HashSet::new();
        for decl in &program.declarations {
            if let TopLevelDecl::Variable(v) = decl {
                for declarator in &v.declarators {
                    // Mark non-const, non-final top-level variables as "global state"
                    if !v.is_const && !v.is_final {
                        top_level_vars.insert(declarator.name.name.clone());
                    }
                }
            }
        }

        for decl in &program.declarations {
            if let TopLevelDecl::Variable(v) = decl
                && !v.is_const && !v.is_final {
                    // Report the declaration itself
                    diags.push(Diagnostic::new(
                        "avoid-top-level-member-access",
                        Severity::Warning,
                        "Avoid using non-const top-level members",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: v.span.start,
                            end: v.span.end,
                        },
                    ));
                }
        }

        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx, &top_level_vars);
        }

        for decl in &program.declarations {
            if let TopLevelDecl::Class(c) = decl {
                for member in &c.members {
                    if let ClassMember::Field(f) = member
                        && f.is_static && !f.is_final && !f.is_const {
                            diags.push(Diagnostic::new(
                                "avoid-top-level-member-access",
                                Severity::Warning,
                                "Avoid using non-const top-level members",
                                ctx.file_path.to_string_lossy().into_owned(),
                                DiagSpan {
                                    start: f.span.start,
                                    end: f.span.end,
                                },
                            ));
                        }
                }
            }
        }

        diags
    }
}

fn scan_top(
    decl: &TopLevelDecl,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    top_level_vars: &HashSet<String>,
) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx, top_level_vars);
            }
        }
        TopLevelDecl::Class(c) => {
            for m in &c.members {
                scan_member(m, diags, ctx, top_level_vars);
            }
        }
        TopLevelDecl::Mixin(m) => {
            for mem in &m.members {
                scan_member(mem, diags, ctx, top_level_vars);
            }
        }
        TopLevelDecl::MixinClass(mc) => {
            for m in &mc.members {
                scan_member(m, diags, ctx, top_level_vars);
            }
        }
        _ => {}
    }
}

fn scan_member(
    member: &ClassMember,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    top_level_vars: &HashSet<String>,
) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    };
    if let Some(b) = body {
        scan_body(b, diags, ctx, top_level_vars);
    }
}

fn scan_body(
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    top_level_vars: &HashSet<String>,
) {
    match body {
        FunctionBody::Block(b) => scan_stmts(&b.stmts, diags, ctx, top_level_vars),
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx, top_level_vars),
        FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmts(
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    top_level_vars: &HashSet<String>,
) {
    for s in stmts {
        scan_stmt(s, diags, ctx, top_level_vars);
    }
}

fn scan_stmt(
    stmt: &Stmt,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    top_level_vars: &HashSet<String>,
) {
    match stmt {
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx, top_level_vars),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx, top_level_vars);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx, top_level_vars);
                }
            }
        }
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx, top_level_vars),
        Stmt::If(i) => {
            if let IfCondition::Expr(e) = &i.condition {
                scan_expr(e, diags, ctx, top_level_vars);
            }
            scan_stmt(&i.then_branch, diags, ctx, top_level_vars);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx, top_level_vars);
            }
        }
        Stmt::While(w) => {
            scan_expr(&w.condition, diags, ctx, top_level_vars);
            scan_stmt(&w.body, diags, ctx, top_level_vars);
        }
        Stmt::DoWhile(d) => {
            scan_stmt(&d.body, diags, ctx, top_level_vars);
            scan_expr(&d.condition, diags, ctx, top_level_vars);
        }
        Stmt::For(f) => {
            if let Some(ForInit::VarDecl(lv)) = &f.init {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx, top_level_vars);
                    }
                }
            }
            if let Some(cond) = &f.condition {
                scan_expr(cond, diags, ctx, top_level_vars);
            }
            for u in &f.update {
                scan_expr(u, diags, ctx, top_level_vars);
            }
            scan_stmt(&f.body, diags, ctx, top_level_vars);
        }
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx, top_level_vars);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx, top_level_vars);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx, top_level_vars);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx, top_level_vars),
        Stmt::Assert(a) => {
            scan_expr(&a.condition, diags, ctx, top_level_vars);
            if let Some(msg) = &a.message {
                scan_expr(msg, diags, ctx, top_level_vars);
            }
        }
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx, top_level_vars),
        _ => {}
    }
}

fn scan_expr(
    expr: &Expr,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    top_level_vars: &HashSet<String>,
) {
    match expr {
        Expr::Ident(id) => {
            if top_level_vars.contains(&id.name) {
                diags.push(Diagnostic::new(
                    "avoid-top-level-member-access",
                    Severity::Warning,
                    "Avoid using non-const top-level members",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: id.span.start,
                        end: id.span.end,
                    },
                ));
            }
        }
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx, top_level_vars);
            scan_expr(value, diags, ctx, top_level_vars);
        }
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, ctx, top_level_vars);
            for arg in &args.positional {
                scan_expr(arg, diags, ctx, top_level_vars);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx, top_level_vars);
            }
        }
        Expr::Field { object, .. } => scan_expr(object, diags, ctx, top_level_vars),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx, top_level_vars);
            scan_expr(right, diags, ctx, top_level_vars);
        }
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            scan_expr(condition, diags, ctx, top_level_vars);
            scan_expr(then_expr, diags, ctx, top_level_vars);
            scan_expr(else_expr, diags, ctx, top_level_vars);
        }
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx, top_level_vars),
        Expr::Await { expr, .. } => scan_expr(expr, diags, ctx, top_level_vars),
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx, top_level_vars),
        Expr::PostfixIncDec { operand, .. } => scan_expr(operand, diags, ctx, top_level_vars),
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx, top_level_vars);
            scan_expr(index, diags, ctx, top_level_vars);
        }
        _ => {}
    }
}
