use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidInvertedBooleanExpressions;

impl Rule for AvoidInvertedBooleanExpressions {
    fn name(&self) -> &'static str {
        "avoid_inverted_boolean_expressions"
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
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Unary {
            op: UnaryOp::Bang,
            operand,
            span,
        } => {
            // Check if operand is also a bang unary
            if is_bang_unary(operand) {
                // Double negation (or more): emit one diagnostic at this outer bang
                diags.push(Diagnostic::new(
                    "avoid_inverted_boolean_expressions",
                    Severity::Warning,
                    "Avoid inverted boolean expressions. Simplify the double negation.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
                // Skip past all consecutive leading bangs to avoid re-flagging inner bangs
                let mut current = operand.as_ref();
                loop {
                    if let Expr::Unary {
                        op: UnaryOp::Bang,
                        operand: next,
                        ..
                    } = current
                    {
                        current = next.as_ref();
                    } else {
                        // Found the first non-bang operand, scan from here
                        scan_expr(current, diags, ctx);
                        break;
                    }
                }
            } else {
                // Single bang: just continue scanning the operand
                scan_expr(operand, diags, ctx);
            }
        }
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
        Expr::List { elements, .. } => {
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

fn is_bang_unary(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Unary {
            op: UnaryOp::Bang,
            ..
        }
    )
}
