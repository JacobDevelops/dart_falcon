use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidNonNullAssertion;

impl Rule for AvoidNonNullAssertion {
    fn name(&self) -> &'static str {
        "avoid-non-null-assertion"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            check_top_level(&mut diags, decl, ctx);
        }
        diags
    }
}

fn diag(ctx: &AnalyzeContext, span: &Span) -> Diagnostic {
    Diagnostic::new(
        "avoid-non-null-assertion",
        Severity::Warning,
        "Avoid using the null assertion operator '!'",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan { start: span.start, end: span.end },
    )
}

fn check_expr(diags: &mut Vec<Diagnostic>, expr: &Expr, ctx: &AnalyzeContext) {
    match expr {
        Expr::NullAssert { operand, span } => {
            diags.push(diag(ctx, span));
            check_expr(diags, operand, ctx);
        }
        Expr::Unary { operand, .. } | Expr::PostfixIncDec { operand, .. } => {
            check_expr(diags, operand, ctx);
        }
        Expr::Binary { left, right, .. } => {
            check_expr(diags, left, ctx);
            check_expr(diags, right, ctx);
        }
        Expr::Assign { target, value, .. } => {
            check_expr(diags, target, ctx);
            check_expr(diags, value, ctx);
        }
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            check_expr(diags, condition, ctx);
            check_expr(diags, then_expr, ctx);
            check_expr(diags, else_expr, ctx);
        }
        Expr::Is { expr: e, .. } | Expr::As { expr: e, .. } => check_expr(diags, e, ctx),
        Expr::Field { object, .. } => check_expr(diags, object, ctx),
        Expr::Index { object, index, .. } => {
            check_expr(diags, object, ctx);
            check_expr(diags, index, ctx);
        }
        Expr::Call { callee, args, .. } => {
            check_expr(diags, callee, ctx);
            for a in &args.positional { check_expr(diags, a, ctx); }
            for na in &args.named { check_expr(diags, &na.value, ctx); }
        }
        Expr::Cascade { object, sections, .. } => {
            check_expr(diags, object, ctx);
            for section in sections {
                match &section.op {
                    CascadeOp::Index(idx, _) => check_expr(diags, idx, ctx),
                    CascadeOp::Call(_, _, args) => {
                        for a in &args.positional { check_expr(diags, a, ctx); }
                        for na in &args.named { check_expr(diags, &na.value, ctx); }
                    }
                    CascadeOp::Assign(tgt, _, val) => {
                        check_expr(diags, tgt, ctx);
                        check_expr(diags, val, ctx);
                    }
                    CascadeOp::Field(_, _) => {}
                }
            }
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            for elem in elements { check_collection_elem(diags, elem, ctx); }
        }
        Expr::Map { entries, .. } => {
            for e in entries {
                check_expr(diags, &e.key, ctx);
                check_expr(diags, &e.value, ctx);
            }
        }
        Expr::Record { fields, .. } => {
            for f in fields { check_expr(diags, &f.value, ctx); }
        }
        Expr::FuncExpr { body, .. } => check_body(diags, body, ctx),
        Expr::New { args, .. } => {
            for a in &args.positional { check_expr(diags, a, ctx); }
            for na in &args.named { check_expr(diags, &na.value, ctx); }
        }
        Expr::Await { expr: e, .. } | Expr::Throw { expr: e, .. } => check_expr(diags, e, ctx),
        Expr::Switch { subject, arms, .. } => {
            check_expr(diags, subject, ctx);
            for arm in arms {
                check_expr(diags, &arm.body, ctx);
                if let Some(ref g) = arm.guard { check_expr(diags, g, ctx); }
            }
        }
        _ => {}
    }
}

fn check_collection_elem(diags: &mut Vec<Diagnostic>, elem: &CollectionElement, ctx: &AnalyzeContext) {
    match elem {
        CollectionElement::Expr(e) => check_expr(diags, e, ctx),
        CollectionElement::Spread { expr, .. } => check_expr(diags, expr, ctx),
        CollectionElement::If { condition, then_elem, else_elem, .. } => {
            if let IfCondition::Expr(e) = condition { check_expr(diags, e, ctx); }
            check_collection_elem(diags, then_elem, ctx);
            if let Some(ee) = else_elem { check_collection_elem(diags, ee, ctx); }
        }
        CollectionElement::For { iterable, element, .. } => {
            check_expr(diags, iterable, ctx);
            check_collection_elem(diags, element, ctx);
        }
    }
}

fn check_body(diags: &mut Vec<Diagnostic>, body: &FunctionBody, ctx: &AnalyzeContext) {
    match body {
        FunctionBody::Block(b) => check_stmts(diags, &b.stmts, ctx),
        FunctionBody::Arrow(e, _) => check_expr(diags, e, ctx),
        FunctionBody::Native(_, _) => {}
    }
}

fn check_stmts(diags: &mut Vec<Diagnostic>, stmts: &[Stmt], ctx: &AnalyzeContext) {
    for s in stmts { check_stmt(diags, s, ctx); }
}

fn check_stmt(diags: &mut Vec<Diagnostic>, stmt: &Stmt, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Expr(e) => check_expr(diags, &e.expr, ctx),
        Stmt::Return(r) => { if let Some(v) = &r.value { check_expr(diags, v, ctx); } }
        Stmt::Throw(t) => check_expr(diags, &t.value, ctx),
        Stmt::Yield(y) => check_expr(diags, &y.value, ctx),
        Stmt::Assert(a) => {
            check_expr(diags, &a.condition, ctx);
            if let Some(m) = &a.message { check_expr(diags, m, ctx); }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer { check_expr(diags, init, ctx); }
            }
        }
        Stmt::LocalFunc(lf) => check_body(diags, &lf.body, ctx),
        Stmt::Block(b) => check_stmts(diags, &b.stmts, ctx),
        Stmt::If(s) => {
            if let IfCondition::Expr(e) = &s.condition { check_expr(diags, e, ctx); }
            check_stmt(diags, &s.then_branch, ctx);
            if let Some(e) = &s.else_branch { check_stmt(diags, e, ctx); }
        }
        Stmt::For(s) => {
            match &s.init {
                Some(ForInit::VarDecl(lv)) => {
                    for d in &lv.declarators {
                        if let Some(init) = &d.initializer { check_expr(diags, init, ctx); }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => check_expr(diags, iterable, ctx),
                Some(ForInit::Exprs(exprs)) => { for e in exprs { check_expr(diags, e, ctx); } }
                None => {}
            }
            if let Some(c) = &s.condition { check_expr(diags, c, ctx); }
            for u in &s.update { check_expr(diags, u, ctx); }
            check_stmt(diags, &s.body, ctx);
        }
        Stmt::While(s) => {
            check_expr(diags, &s.condition, ctx);
            check_stmt(diags, &s.body, ctx);
        }
        Stmt::DoWhile(s) => {
            check_stmt(diags, &s.body, ctx);
            check_expr(diags, &s.condition, ctx);
        }
        Stmt::Switch(s) => {
            check_expr(diags, &s.subject, ctx);
            for case in &s.cases { check_stmts(diags, &case.body, ctx); }
        }
        Stmt::TryCatch(s) => {
            check_stmts(diags, &s.body.stmts, ctx);
            for c in &s.catches { check_stmts(diags, &c.body.stmts, ctx); }
            if let Some(f) = &s.finally { check_stmts(diags, &f.stmts, ctx); }
        }
        _ => {}
    }
}

fn check_members(diags: &mut Vec<Diagnostic>, members: &[ClassMember], ctx: &AnalyzeContext) {
    for member in members {
        match member {
            ClassMember::Method(m) => {
                if let Some(b) = &m.body { check_body(diags, b, ctx); }
            }
            ClassMember::Getter(g) => {
                if let Some(b) = &g.body { check_body(diags, b, ctx); }
            }
            ClassMember::Setter(s) => {
                if let Some(b) = &s.body { check_body(diags, b, ctx); }
            }
            ClassMember::Field(f) => {
                for d in &f.declarators {
                    if let Some(init) = &d.initializer { check_expr(diags, init, ctx); }
                }
            }
            ClassMember::Constructor(c) => {
                if let Some(b) = &c.body { check_body(diags, b, ctx); }
            }
            _ => {}
        }
    }
}

fn check_top_level(diags: &mut Vec<Diagnostic>, decl: &TopLevelDecl, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(b) = &f.body { check_body(diags, b, ctx); }
        }
        TopLevelDecl::Variable(v) => {
            for d in &v.declarators {
                if let Some(init) = &d.initializer { check_expr(diags, init, ctx); }
            }
        }
        TopLevelDecl::Class(c) => check_members(diags, &c.members, ctx),
        TopLevelDecl::Mixin(m) => check_members(diags, &m.members, ctx),
        TopLevelDecl::MixinClass(mc) => check_members(diags, &mc.members, ctx),
        TopLevelDecl::Enum(e) => check_members(diags, &e.members, ctx),
        TopLevelDecl::Extension(e) => check_members(diags, &e.members, ctx),
        TopLevelDecl::ExtensionType(e) => check_members(diags, &e.members, ctx),
        _ => {}
    }
}
