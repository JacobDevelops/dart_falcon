use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferCorrectEdgeInsetsConstructor;

impl Rule for PreferCorrectEdgeInsetsConstructor {
    fn name(&self) -> &'static str {
        "prefer-correct-edge-insets-constructor"
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
        Expr::New {
            dart_type,
            args,
            span,
            ..
        } => {
            check_edge_insets_only(dart_type, args, span, diags, ctx);
            scan_args(args, diags, ctx);
        }
        Expr::Call {
            callee, args, span, ..
        } => {
            check_edge_insets_only_call(callee, args, span, diags, ctx);
            scan_expr(callee, diags, ctx);
            scan_args(args, diags, ctx);
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
                scan_collection_elem(elem, diags, ctx);
            }
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                scan_collection_elem(elem, diags, ctx);
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
        CollectionElement::CFor {
            init,
            condition,
            updates,
            element,
            ..
        } => {
            match init {
                Some(ForInit::VarDecl(d)) => {
                    for decl in &d.declarators {
                        if let Some(e) = &decl.initializer {
                            scan_expr(e, diags, ctx);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => {
                    scan_expr(iterable, diags, ctx);
                }
                Some(ForInit::Exprs(es)) => {
                    for e in es {
                        scan_expr(e, diags, ctx);
                    }
                }
                None => {}
            }
            if let Some(c) = condition {
                scan_expr(c, diags, ctx);
            }
            for u in updates {
                scan_expr(u, diags, ctx);
            }
            scan_collection_elem(element, diags, ctx);
        }
    }
}

fn check_edge_insets_only_call(
    callee: &Expr,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Expr::Field { object, field, .. } = callee
        && let Expr::Ident(ident) = object.as_ref()
        && ident.name == "EdgeInsets"
        && field.name == "only"
    {
        let should_flag = should_use_better_constructor(args);
        if should_flag {
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
                // Multi-line - check if opening line contains comment marker
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
                    // Comment is on closing line
                    DiagSpan {
                        start: span.end - 1,
                        end: span.end,
                    }
                }
            };

            diags.push(Diagnostic::new(
                "prefer-correct-edge-insets-constructor",
                Severity::Warning,
                "EdgeInsets.only() should use EdgeInsets.symmetric() or EdgeInsets.all().",
                ctx.file_path.to_string_lossy().into_owned(),
                report_span,
            ));
        }
    }
}

fn check_edge_insets_only(
    dart_type: &DartType,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let DartType::Named(nt) = dart_type
        && nt.segments.len() == 1
        && nt.segments[0].name == "EdgeInsets"
    {
        let should_flag = should_use_better_constructor(args);
        if should_flag {
            diags.push(Diagnostic::new(
                "prefer-correct-edge-insets-constructor",
                Severity::Warning,
                "EdgeInsets.only() should use EdgeInsets.symmetric() or EdgeInsets.all().",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }
}

fn should_use_better_constructor(args: &ArgList) -> bool {
    let mut left = None;
    let mut right = None;
    let mut top = None;
    let mut bottom = None;

    for named in &args.named {
        let value_str = expr_to_string(&named.value);
        match named.name.name.as_str() {
            "left" => left = value_str,
            "right" => right = value_str,
            "top" => top = value_str,
            "bottom" => bottom = value_str,
            _ => {}
        }
    }

    // Check if all four are present and equal -> should use .all()
    if let (Some(l), Some(r), Some(t), Some(b)) =
        (left.clone(), right.clone(), top.clone(), bottom.clone())
        && l == r
        && r == t
        && t == b
    {
        return true;
    }

    // Check if only top and bottom are present and equal -> should use .symmetric(vertical: ...)
    if left.is_none()
        && right.is_none()
        && let (Some(t), Some(b)) = (top.clone(), bottom.clone())
        && t == b
    {
        return true;
    }

    // Check if only left and right are present and equal -> should use .symmetric(horizontal: ...)
    if top.is_none()
        && bottom.is_none()
        && let (Some(l), Some(r)) = (left.clone(), right.clone())
        && l == r
    {
        return true;
    }

    false
}

fn expr_to_string(expr: &Expr) -> Option<String> {
    match expr {
        Expr::IntLit { value, .. } => Some(value.clone()),
        Expr::DoubleLit { value, .. } => Some(value.clone()),
        Expr::Ident(id) => Some(id.name.clone()),
        _ => None,
    }
}
