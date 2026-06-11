use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct BinaryExpressionOperandOrder;

impl Rule for BinaryExpressionOperandOrder {
    fn name(&self) -> &'static str {
        "binary-expression-operand-order"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn is_literal_or_const(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::StringLit(_)
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
    )
}

fn should_check_operator(op: &BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::EqEq
            | BinaryOp::NotEq
            | BinaryOp::Lt
            | BinaryOp::Gt
            | BinaryOp::LtEq
            | BinaryOp::GtEq
    )
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
            if let Some(ForInit::VarDecl(lv)) = &f.init {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx);
                    }
                }
            }
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
        Stmt::Assert(a) => {
            scan_expr(&a.condition, diags, ctx);
            if let Some(msg) = &a.message {
                scan_expr(msg, diags, ctx);
            }
        }
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Binary {
            op,
            left,
            right,
            span,
        } => {
            // Flag if left is a literal (value or const), UNLESS both sides are literals and left is null
            let left_is_literal = is_literal_or_const(left);
            let right_is_literal = is_literal_or_const(right);
            let left_is_null = matches!(left.as_ref(), Expr::NullLit { .. });

            let should_flag = left_is_literal && !(left_is_null && right_is_literal);

            if should_check_operator(op) && should_flag {
                diags.push(Diagnostic::new(
                    "binary-expression-operand-order",
                    Severity::Warning,
                    "Avoid placing literal or constant operand on the left side of a binary expression",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
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
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::Await { expr, .. } => scan_expr(expr, diags, ctx),
        _ => {}
    }
}
