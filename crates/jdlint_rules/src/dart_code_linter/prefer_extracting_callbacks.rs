use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct PreferExtractingCallbacks;

impl Rule for PreferExtractingCallbacks {
    fn name(&self) -> &'static str {
        "prefer-extracting-callbacks"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn count_statements(body: &FunctionBody) -> usize {
    match body {
        FunctionBody::Block(block) => block.stmts.len(),
        FunctionBody::Arrow(_, _) => 1, // Arrow functions are simple
        FunctionBody::Native(_, _) => 0,
    }
}

fn check_func_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Expr::FuncExpr { body, span, .. } = expr {
        let stmt_count = count_statements(body);
        // Flag if callback has 3+ statements (complex enough to extract)
        if stmt_count >= 3 {
            diags.push(Diagnostic::new(
                "prefer-extracting-callbacks",
                Severity::Warning,
                "Extract complex callback to a named function",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
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
        FunctionBody::Block(b) => {
            scan_stmts(&b.stmts, diags, ctx);
        }
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
        Stmt::Block(b) => {
            scan_stmts(&b.stmts, diags, ctx);
        }
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
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
        Stmt::Switch(sw) => {
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

fn scan_collection_elem(elem: &CollectionElement, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match elem {
        CollectionElement::Expr(e) => scan_expr(e, diags, ctx),
        CollectionElement::Spread { expr: e, .. } => scan_expr(e, diags, ctx),
        CollectionElement::If { condition, then_elem, else_elem, .. } => {
            match condition {
                IfCondition::Expr(e) => scan_expr(e, diags, ctx),
                IfCondition::Case(e, _) => scan_expr(e, diags, ctx),
            }
            scan_collection_elem(then_elem, diags, ctx);
            if let Some(ee) = else_elem {
                scan_collection_elem(ee, diags, ctx);
            }
        }
        CollectionElement::For { iterable, element, .. } => {
            scan_expr(iterable, diags, ctx);
            scan_collection_elem(element, diags, ctx);
        }
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::FuncExpr { body, .. } => {
            check_func_expr(expr, diags, ctx);
            scan_body(body, diags, ctx);
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
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            scan_expr(condition, diags, ctx);
            scan_expr(then_expr, diags, ctx);
            scan_expr(else_expr, diags, ctx);
        }
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Unary { operand, .. } => {
            scan_expr(operand, diags, ctx);
        }
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx);
            scan_expr(value, diags, ctx);
        }
        Expr::New { args, .. } => {
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx);
            }
        }
        Expr::List { elements, .. } => {
            for elem in elements {
                match elem {
                    CollectionElement::Expr(e) => scan_expr(e, diags, ctx),
                    CollectionElement::Spread { expr: e, .. } => scan_expr(e, diags, ctx),
                    CollectionElement::If { condition, then_elem, else_elem, .. } => {
                        match condition {
                            IfCondition::Expr(e) => scan_expr(e, diags, ctx),
                            IfCondition::Case(e, _) => scan_expr(e, diags, ctx),
                        }
                        scan_collection_elem(then_elem, diags, ctx);
                        if let Some(ee) = else_elem {
                            scan_collection_elem(ee, diags, ctx);
                        }
                    }
                    CollectionElement::For { iterable, element, .. } => {
                        scan_expr(iterable, diags, ctx);
                        scan_collection_elem(element, diags, ctx);
                    }
                }
            }
        }
        Expr::Await { expr: e, .. } => scan_expr(e, diags, ctx),
        Expr::Throw { expr: e, .. } => scan_expr(e, diags, ctx),
        Expr::NullAssert { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::Field { object, .. } => scan_expr(object, diags, ctx),
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx);
            scan_expr(index, diags, ctx);
        }
        Expr::Cascade { object, .. } => scan_expr(object, diags, ctx),
        _ => {}
    }
}
