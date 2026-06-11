use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferDedicatedMediaQueryMethods;

impl Rule for PreferDedicatedMediaQueryMethods {
    fn name(&self) -> &'static str {
        "prefer_dedicated_media_query_methods"
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
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx);
                }
            }
        }
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx);
        }
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx),
        Stmt::Return(r) => {
            if let Some(expr) = &r.value {
                scan_expr(expr, diags, ctx);
            }
        }
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        Stmt::Yield(y) => scan_expr(&y.value, diags, ctx),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Field {
            object,
            field,
            span,
            ..
        } => {
            check_media_query_size_field(object, field, span, diags, ctx);
            scan_expr(object, diags, ctx);
        }
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, ctx);
            scan_args(args, diags, ctx);
        }
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx);
            scan_expr(index, diags, ctx);
        }
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
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
        Expr::List { elements, .. } => {
            for elem in elements {
                scan_collection_elem(elem, diags, ctx);
            }
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                scan_collection_elem(elem, diags, ctx);
            }
        }
        Expr::Map { entries, .. } => {
            for entry in entries {
                scan_expr(&entry.key, diags, ctx);
                scan_expr(&entry.value, diags, ctx);
            }
        }
        Expr::New { args, .. } => {
            scan_args(args, diags, ctx);
        }
        _ => {}
    }
}

fn scan_args(args: &ArgList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for arg in &args.positional {
        scan_expr(arg, diags, ctx);
    }
    for named in &args.named {
        scan_expr(&named.value, diags, ctx);
    }
}

fn scan_collection_elem(
    elem: &CollectionElement,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match elem {
        CollectionElement::Expr(e) => scan_expr(e, diags, ctx),
        CollectionElement::Spread { expr, .. } => scan_expr(expr, diags, ctx),
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            if let IfCondition::Expr(e) = condition {
                scan_expr(e, diags, ctx);
            }
            scan_collection_elem(then_elem, diags, ctx);
            if let Some(ee) = else_elem {
                scan_collection_elem(ee, diags, ctx);
            }
        }
        CollectionElement::For {
            iterable, element, ..
        } => {
            scan_expr(iterable, diags, ctx);
            scan_collection_elem(element, diags, ctx);
        }
    }
}

fn check_media_query_size_field(
    object: &Expr,
    field: &Identifier,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    // Check if accessing .width or .height on MediaQuery.of(context).size
    if (field.name == "width" || field.name == "height") && is_media_query_size(object) {
        diags.push(Diagnostic::new(
            "prefer_dedicated_media_query_methods",
            Severity::Warning,
            "Use MediaQuery.sizeOf(context) instead of MediaQuery.of(context).size.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn is_media_query_size(expr: &Expr) -> bool {
    if let Expr::Field { object, field, .. } = expr
        && field.name == "size"
        && let Expr::Call { callee, args, .. } = object.as_ref()
        && let Expr::Field {
            object: mq_obj,
            field: method,
            ..
        } = callee.as_ref()
        && let Expr::Ident(ident) = mq_obj.as_ref()
    {
        return ident.name == "MediaQuery" && method.name == "of" && args.positional.len() == 1;
    }
    false
}
