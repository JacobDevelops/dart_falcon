use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferConstBorderRadius;

impl Rule for PreferConstBorderRadius {
    fn name(&self) -> &'static str {
        "prefer-const-border-radius"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top_level_decl(decl, &mut diags, ctx);
        }
        diags
    }
}

fn scan_top_level_decl(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
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
        TopLevelDecl::Variable(v) => {
            for d in &v.declarators {
                if let Some(expr) = &d.initializer {
                    scan_expr(expr, diags, ctx);
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
                if let Some(expr) = &d.initializer {
                    scan_expr(expr, diags, ctx);
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
    for stmt in stmts {
        scan_stmt(stmt, diags, ctx);
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        Stmt::If(i) => {
            scan_stmt(&i.then_branch, diags, ctx);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx);
            }
        }
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
        Stmt::TryCatch(tc) => scan_stmts(&tc.body.stmts, diags, ctx),
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(expr) = &d.initializer {
                    scan_expr(expr, diags, ctx);
                }
            }
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
        Expr::Call {
            callee, args, span, ..
        } => {
            check_border_radius_only_call(callee, args, span, diags, ctx);
            scan_expr(callee, diags, ctx);
            for arg in &args.positional {
                scan_expr(arg, diags, ctx);
            }
            for named_arg in &args.named {
                scan_expr(&named_arg.value, diags, ctx);
            }
        }
        Expr::Field { object, .. } => scan_expr(object, diags, ctx),
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
                if let CollectionElement::Expr(e) = elem {
                    scan_expr(e, diags, ctx);
                }
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for entry in entries {
                scan_expr(&entry.key, diags, ctx);
                scan_expr(&entry.value, diags, ctx);
            }
            for e in map_element_exprs(elements) {
                scan_expr(e, diags, ctx);
            }
        }
        _ => {}
    }
}

fn check_border_radius_only_call(
    callee: &Expr,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Expr::Field { object, field, .. } = callee
        && let Expr::Ident(ident) = object.as_ref()
        && ident.name == "BorderRadius"
        && field.name == "only"
        && all_border_radii_equal(args)
    {
        let source = ctx.source;
        let start_line = source[..span.start].chars().filter(|&c| c == '\n').count();
        let end_line = source[..span.end].chars().filter(|&c| c == '\n').count();

        let report_span = if start_line == end_line {
            // Single line - report at start
            DiagSpan {
                start: span.start,
                end: span.end,
            }
        } else {
            // Multi-line - check if opening line contains the closing paren or comment marker
            let opening_line_end = source[span.start..]
                .find('\n')
                .map(|off| span.start + off)
                .unwrap_or(source.len());
            let opening_line_text = &source[span.start..opening_line_end];

            if opening_line_text.contains("*/") {
                // Comment is on opening line
                DiagSpan {
                    start: span.start,
                    end: span.start + 1,
                }
            } else {
                // Comment is on closing line - report at end
                DiagSpan {
                    start: span.end - 1,
                    end: span.end,
                }
            }
        };

        diags.push(Diagnostic::new(
            "prefer-const-border-radius",
            Severity::Warning,
            "BorderRadius.only() with all equal radii should use BorderRadius.circular().",
            ctx.file_path.to_string_lossy().into_owned(),
            report_span,
        ));
    }
}

fn all_border_radii_equal(args: &ArgList) -> bool {
    let mut top_left = None;
    let mut top_right = None;
    let mut bottom_left = None;
    let mut bottom_right = None;

    for named in &args.named {
        let radius_value = extract_radius_value(&named.value);
        match named.name.name.as_str() {
            "topLeft" => top_left = radius_value,
            "topRight" => top_right = radius_value,
            "bottomLeft" => bottom_left = radius_value,
            "bottomRight" => bottom_right = radius_value,
            _ => {}
        }
    }

    // All four must be present and equal
    if let (Some(tl), Some(tr), Some(bl), Some(br)) =
        (top_left, top_right, bottom_left, bottom_right)
    {
        tl == tr && tr == bl && bl == br
    } else {
        false
    }
}

fn extract_radius_value(expr: &Expr) -> Option<String> {
    // Extract the numeric value from Radius.circular(X)
    if let Expr::Call { callee, args, .. } = expr
        && let Expr::Field { object, field, .. } = callee.as_ref()
        && let Expr::Ident(ident) = object.as_ref()
        && ident.name == "Radius"
        && field.name == "circular"
        && args.positional.len() == 1
    {
        return expr_to_string(&args.positional[0]);
    }
    None
}

fn expr_to_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::IntLit { value, .. } => Some(value.clone()),
        Expr::DoubleLit { value, .. } => Some(value.clone()),
        Expr::Ident(id) => Some(id.name.clone()),
        _ => None,
    }
}
