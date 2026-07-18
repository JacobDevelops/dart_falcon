//! Flags inline callbacks that should be extracted to methods. Ported from dart_code_linter's `prefer-extracting-callbacks`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct PreferExtractingCallbacks;

/// Resolved options. dcl's `allowed-line-count` defaults to `null`, meaning any
/// non-empty block-body callback qualifies regardless of length.
struct Cfg {
    allowed_line_count: Option<usize>,
    ignored: Vec<String>,
}

fn cfg(ctx: &AnalyzeContext) -> Cfg {
    let opts = crate::meta::meta_for("prefer-extracting-callbacks")
        .and_then(|m| ctx.rule_options(m.group, "prefer-extracting-callbacks"));
    let allowed_line_count = opts
        .and_then(|o| o.get("allowed_line_count"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize);
    let ignored = opts
        .and_then(|o| o.get("ignored_named_arguments"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    Cfg {
        allowed_line_count,
        ignored,
    }
}

impl Rule for PreferExtractingCallbacks {
    fn name(&self) -> &'static str {
        "prefer-extracting-callbacks"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let cfg = cfg(ctx);
        for decl in &program.declarations {
            // dcl only visits classes that are a Widget or a Widget State.
            if let TopLevelDecl::Class(c) = decl
                && is_widget_class(c)
            {
                for member in &c.members {
                    if let Some(body) = member_body(member) {
                        scan_body(body, &mut diags, ctx, &cfg);
                    }
                }
            }
        }
        diags
    }
}

fn member_body(member: &ClassMember) -> Option<&FunctionBody> {
    match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    }
}

fn type_base_name(dt: &DartType) -> Option<&str> {
    match dt {
        DartType::Named(n) => n.segments.last().map(|s| s.name.as_str()),
        _ => None,
    }
}

/// A class whose superclass is a Widget or a Widget `State`. Without type
/// resolution this is a name heuristic over the common Flutter base classes
/// (`StatelessWidget`, `StatefulWidget`, `ConsumerWidget`, `State<T>`, …).
fn is_widget_class(c: &ClassDecl) -> bool {
    c.extends
        .as_ref()
        .and_then(type_base_name)
        .is_some_and(|base| base.ends_with("Widget") || base == "State" || base.ends_with("State"))
}

fn root_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(id) => Some(&id.name),
        Expr::Field { object, .. } => root_name(object),
        _ => None,
    }
}

/// True for an instance-creation expression: an explicit `new`/`const`, or a
/// call whose root identifier is upper-cased (a constructor, not a method like
/// `setState(...)`).
fn is_construction(expr: &Expr) -> bool {
    match expr {
        Expr::New { .. } => true,
        Expr::Call { callee, .. } => root_name(callee)
            .and_then(|n| n.chars().next())
            .is_some_and(|c| c.is_uppercase()),
        _ => false,
    }
}

fn is_nonempty_block(body: &FunctionBody) -> bool {
    matches!(body, FunctionBody::Block(b) if !b.stmts.is_empty())
}

/// A Flutter *builder* callback — first parameter typed `BuildContext` — is
/// excluded, as extracting it defeats the builder pattern.
fn is_builder(params: &FormalParamList) -> bool {
    params
        .positional
        .first()
        .or_else(|| params.optional_positional.first())
        .and_then(|p| p.param_type.as_ref())
        .and_then(type_base_name)
        == Some("BuildContext")
}

fn is_long_enough(span: &Span, source: &str, cfg: &Cfg) -> bool {
    match cfg.allowed_line_count {
        None => true,
        Some(limit) => {
            let end = span.end.min(source.len());
            let lines = source[span.start..end]
                .bytes()
                .filter(|&b| b == b'\n')
                .count()
                + 1;
            lines > limit
        }
    }
}

/// Inspect the arguments of a widget construction for extractable callbacks.
fn check_construction_args(
    args: &ArgList,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &Cfg,
) {
    for arg in &args.positional {
        maybe_flag_callback(arg, diags, ctx, cfg);
    }
    for named in &args.named {
        if cfg.ignored.iter().any(|n| n == &named.name.name) {
            continue;
        }
        maybe_flag_callback(&named.value, diags, ctx, cfg);
    }
}

fn maybe_flag_callback(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &Cfg) {
    if let Expr::FuncExpr {
        body, params, span, ..
    } = expr
        && is_nonempty_block(body)
        && !is_builder(params)
        && is_long_enough(span, ctx.source, cfg)
    {
        diags.push(Diagnostic::new(
            "prefer-extracting-callbacks",
            Severity::Warning,
            "Prefer extracting the callback to a separate widget method.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &Cfg) {
    match body {
        FunctionBody::Block(b) => {
            for s in &b.stmts {
                scan_stmt(s, diags, ctx, cfg);
            }
        }
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx, cfg),
        FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &Cfg) {
    match stmt {
        Stmt::Block(b) => b.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx, cfg)),
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx, cfg),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx, cfg);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx, cfg);
                }
            }
        }
        Stmt::If(i) => {
            if let IfCondition::Expr(e) = &i.condition {
                scan_expr(e, diags, ctx, cfg);
            }
            scan_stmt(&i.then_branch, diags, ctx, cfg);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx, cfg);
            }
        }
        Stmt::While(w) => scan_stmt(&w.body, diags, ctx, cfg),
        Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx, cfg),
        Stmt::For(f) => scan_stmt(&f.body, diags, ctx, cfg),
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                case.body.iter().for_each(|s| scan_stmt(s, diags, ctx, cfg));
            }
        }
        Stmt::TryCatch(tc) => {
            tc.body
                .stmts
                .iter()
                .for_each(|s| scan_stmt(s, diags, ctx, cfg));
            for catch in &tc.catches {
                catch
                    .body
                    .stmts
                    .iter()
                    .for_each(|s| scan_stmt(s, diags, ctx, cfg));
            }
            if let Some(fin) = &tc.finally {
                fin.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx, cfg));
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx, cfg),
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx, cfg),
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &Cfg) {
    // At a widget construction, its callback arguments are candidates.
    if is_construction(expr) {
        let args = match expr {
            Expr::New { args, .. } | Expr::Call { args, .. } => Some(args),
            _ => None,
        };
        if let Some(args) = args {
            check_construction_args(args, diags, ctx, cfg);
        }
    }

    // Recurse everywhere so nested constructions and callback bodies are seen.
    match expr {
        Expr::New { args, .. } | Expr::Call { args, .. } => {
            if let Expr::Call { callee, .. } = expr {
                scan_expr(callee, diags, ctx, cfg);
            }
            for arg in &args.positional {
                scan_expr(arg, diags, ctx, cfg);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx, cfg);
            }
        }
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx, cfg),
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            scan_expr(condition, diags, ctx, cfg);
            scan_expr(then_expr, diags, ctx, cfg);
            scan_expr(else_expr, diags, ctx, cfg);
        }
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx, cfg);
            scan_expr(right, diags, ctx, cfg);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx, cfg),
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx, cfg);
            scan_expr(value, diags, ctx, cfg);
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            for elem in elements {
                scan_collection_elem(elem, diags, ctx, cfg);
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for entry in entries {
                scan_expr(&entry.key, diags, ctx, cfg);
                scan_expr(&entry.value, diags, ctx, cfg);
            }
            for e in map_element_exprs(elements) {
                scan_expr(e, diags, ctx, cfg);
            }
        }
        Expr::Await { expr: e, .. }
        | Expr::Throw { expr: e, .. }
        | Expr::NullAssert { operand: e, .. } => scan_expr(e, diags, ctx, cfg),
        Expr::Field { object, .. } => scan_expr(object, diags, ctx, cfg),
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx, cfg);
            scan_expr(index, diags, ctx, cfg);
        }
        Expr::Cascade { object, .. } => scan_expr(object, diags, ctx, cfg),
        _ => {}
    }
}

fn scan_collection_elem(
    elem: &CollectionElement,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    cfg: &Cfg,
) {
    match elem {
        CollectionElement::Expr(e) | CollectionElement::Spread { expr: e, .. } => {
            scan_expr(e, diags, ctx, cfg)
        }
        CollectionElement::NullAware { expr, .. } => scan_expr(expr, diags, ctx, cfg),
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            match condition {
                IfCondition::Expr(e) | IfCondition::Case(e, _) => scan_expr(e, diags, ctx, cfg),
            }
            scan_collection_elem(then_elem, diags, ctx, cfg);
            if let Some(ee) = else_elem {
                scan_collection_elem(ee, diags, ctx, cfg);
            }
        }
        CollectionElement::For {
            iterable, element, ..
        } => {
            scan_expr(iterable, diags, ctx, cfg);
            scan_collection_elem(element, diags, ctx, cfg);
        }
        CollectionElement::CFor {
            condition, element, ..
        } => {
            if let Some(c) = condition {
                scan_expr(c, diags, ctx, cfg);
            }
            scan_collection_elem(element, diags, ctx, cfg);
        }
    }
}
