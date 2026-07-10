use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct UseDesignSystemItem;

impl Rule for UseDesignSystemItem {
    fn name(&self) -> &'static str {
        "use-design-system-item"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // Config-driven: with no configured items the rule is a no-op.
        let items = parse_config_items(ctx);
        if items.is_empty() {
            return Vec::new();
        }

        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx, &items);
        }
        diags
    }
}

/// Read the rule's `options["items"]` as `(class_name, use_instead)` pairs.
/// Any missing or malformed config yields an empty list (the rule then does nothing).
fn parse_config_items(ctx: &AnalyzeContext) -> Vec<(String, Option<String>)> {
    // Look up options under the rule's own group so a misplaced config entry is
    // ignored, staying consistent with severity resolution.
    let Some(group) = crate::meta::meta_for("use-design-system-item").map(|m| m.group) else {
        return Vec::new();
    };
    let Some(items_array) = ctx
        .config
        .rule_options(group, "use-design-system-item")
        .and_then(|opts| opts.get("items"))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };

    items_array
        .iter()
        .filter_map(|item| {
            let class_name = item.get("class_name").and_then(|v| v.as_str())?;
            let use_instead = item
                .get("use_instead")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            Some((class_name.to_string(), use_instead))
        })
        .collect()
}

fn scan_top(
    decl: &TopLevelDecl,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(String, Option<String>)],
) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                scan_body(body, diags, ctx, items);
            }
        }
        TopLevelDecl::Class(c) => {
            for m in &c.members {
                scan_member(m, diags, ctx, items);
            }
        }
        TopLevelDecl::Mixin(m) => {
            for mem in &m.members {
                scan_member(mem, diags, ctx, items);
            }
        }
        TopLevelDecl::MixinClass(mc) => {
            for m in &mc.members {
                scan_member(m, diags, ctx, items);
            }
        }
        TopLevelDecl::Enum(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx, items);
            }
        }
        TopLevelDecl::Extension(ext) => {
            for m in &ext.members {
                scan_member(m, diags, ctx, items);
            }
        }
        _ => {}
    }
}

fn scan_member(
    member: &ClassMember,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(String, Option<String>)],
) {
    let body = match member {
        ClassMember::Method(m) => m.body.as_ref(),
        ClassMember::Constructor(c) => c.body.as_ref(),
        ClassMember::Getter(g) => g.body.as_ref(),
        ClassMember::Setter(s) => s.body.as_ref(),
        _ => None,
    };
    if let Some(b) = body {
        scan_body(b, diags, ctx, items);
    }
}

fn scan_body(
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(String, Option<String>)],
) {
    match body {
        FunctionBody::Block(b) => {
            scan_stmts(&b.stmts, diags, ctx, items);
        }
        FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx, items),
        FunctionBody::Native(_, _) => {}
    }
}

fn scan_stmts(
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(String, Option<String>)],
) {
    for s in stmts {
        scan_stmt(s, diags, ctx, items);
    }
}

fn scan_stmt(
    stmt: &Stmt,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(String, Option<String>)],
) {
    match stmt {
        Stmt::Block(b) => {
            scan_stmts(&b.stmts, diags, ctx, items);
        }
        Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx, items),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                scan_expr(v, diags, ctx, items);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    scan_expr(init, diags, ctx, items);
                }
            }
        }
        Stmt::If(i) => {
            match &i.condition {
                IfCondition::Expr(e) => scan_expr(e, diags, ctx, items),
                IfCondition::Case(e, _) => scan_expr(e, diags, ctx, items),
            }
            scan_stmt(&i.then_branch, diags, ctx, items);
            if let Some(eb) = &i.else_branch {
                scan_stmt(eb, diags, ctx, items);
            }
        }
        Stmt::While(w) => {
            scan_expr(&w.condition, diags, ctx, items);
            scan_stmt(&w.body, diags, ctx, items);
        }
        Stmt::DoWhile(d) => {
            scan_stmt(&d.body, diags, ctx, items);
            scan_expr(&d.condition, diags, ctx, items);
        }
        Stmt::For(f) => {
            if let Some(cond) = &f.condition {
                scan_expr(cond, diags, ctx, items);
            }
            scan_stmt(&f.body, diags, ctx, items);
        }
        Stmt::Switch(sw) => {
            scan_expr(&sw.subject, diags, ctx, items);
            for case in &sw.cases {
                scan_stmts(&case.body, diags, ctx, items);
            }
        }
        Stmt::TryCatch(tc) => {
            scan_stmts(&tc.body.stmts, diags, ctx, items);
            for catch in &tc.catches {
                scan_stmts(&catch.body.stmts, diags, ctx, items);
            }
            if let Some(fin) = &tc.finally {
                scan_stmts(&fin.stmts, diags, ctx, items);
            }
        }
        Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx, items),
        _ => {}
    }
}

fn scan_expr(
    expr: &Expr,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(String, Option<String>)],
) {
    // Detect construction of a disallowed class, in either form:
    //   `new Container(...)` / `const Container(...)`  -> Expr::New
    //   `Container(...)` / `Container<T>(...)`          -> Expr::Call with an Ident/type-instantiation callee
    match expr {
        Expr::New {
            dart_type, span, ..
        } => {
            if let Some(base) = new_base_name(dart_type) {
                emit_for_base(base, span, diags, ctx, items);
            }
        }
        Expr::Call { callee, span, .. } => {
            if let Some(base) = callee_base_name(callee) {
                emit_for_base(base, span, diags, ctx, items);
            }
        }
        _ => {}
    }

    // Recurse into sub-expressions
    match expr {
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx, items),
        Expr::Call { callee, args, .. } => {
            scan_expr(callee, diags, ctx, items);
            for arg in &args.positional {
                scan_expr(arg, diags, ctx, items);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx, items);
            }
        }
        Expr::Field { object, .. } => scan_expr(object, diags, ctx, items),
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx, items);
            scan_expr(index, diags, ctx, items);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx, items),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx, items);
            scan_expr(right, diags, ctx, items);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            scan_expr(condition, diags, ctx, items);
            scan_expr(then_expr, diags, ctx, items);
            scan_expr(else_expr, diags, ctx, items);
        }
        Expr::New { args, .. } => {
            // Recursively scan constructor args
            for arg in &args.positional {
                scan_expr(arg, diags, ctx, items);
            }
            for named in &args.named {
                scan_expr(&named.value, diags, ctx, items);
            }
        }
        Expr::List { elements, .. } => {
            for elem in elements {
                match elem {
                    CollectionElement::Expr(e) => scan_expr(e, diags, ctx, items),
                    CollectionElement::Spread { expr: e, .. } => scan_expr(e, diags, ctx, items),
                    CollectionElement::NullAware { expr: e, .. } => scan_expr(e, diags, ctx, items),
                    CollectionElement::If {
                        then_elem,
                        else_elem,
                        ..
                    } => {
                        if let CollectionElement::Expr(e) = &**then_elem {
                            scan_expr(e, diags, ctx, items);
                        }
                        if let Some(ee) = else_elem
                            && let CollectionElement::Expr(e) = &**ee
                        {
                            scan_expr(e, diags, ctx, items);
                        }
                    }
                    CollectionElement::For {
                        iterable, element, ..
                    } => {
                        scan_expr(iterable, diags, ctx, items);
                        if let CollectionElement::Expr(e) = &**element {
                            scan_expr(e, diags, ctx, items);
                        }
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
                                        scan_expr(e, diags, ctx, items);
                                    }
                                }
                            }
                            Some(ForInit::ForIn { iterable, .. }) => {
                                scan_expr(iterable, diags, ctx, items);
                            }
                            Some(ForInit::PatternForIn { iterable, .. }) => {
                                scan_expr(iterable, diags, ctx, items);
                            }
                            Some(ForInit::Exprs(es)) => {
                                for e in es {
                                    scan_expr(e, diags, ctx, items);
                                }
                            }
                            None => {}
                        }
                        if let Some(c) = condition {
                            scan_expr(c, diags, ctx, items);
                        }
                        for u in updates {
                            scan_expr(u, diags, ctx, items);
                        }
                        if let CollectionElement::Expr(e) = &**element {
                            scan_expr(e, diags, ctx, items);
                        }
                    }
                }
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for entry in entries {
                scan_expr(&entry.key, diags, ctx, items);
                scan_expr(&entry.value, diags, ctx, items);
            }
            for e in map_element_exprs(elements) {
                scan_expr(e, diags, ctx, items);
            }
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                match elem {
                    CollectionElement::Expr(e) => scan_expr(e, diags, ctx, items),
                    CollectionElement::Spread { expr: e, .. } => scan_expr(e, diags, ctx, items),
                    CollectionElement::NullAware { expr: e, .. } => scan_expr(e, diags, ctx, items),
                    CollectionElement::If {
                        then_elem,
                        else_elem,
                        ..
                    } => {
                        if let CollectionElement::Expr(e) = &**then_elem {
                            scan_expr(e, diags, ctx, items);
                        }
                        if let Some(ee) = else_elem
                            && let CollectionElement::Expr(e) = &**ee
                        {
                            scan_expr(e, diags, ctx, items);
                        }
                    }
                    CollectionElement::For {
                        iterable, element, ..
                    } => {
                        scan_expr(iterable, diags, ctx, items);
                        if let CollectionElement::Expr(e) = &**element {
                            scan_expr(e, diags, ctx, items);
                        }
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
                                        scan_expr(e, diags, ctx, items);
                                    }
                                }
                            }
                            Some(ForInit::ForIn { iterable, .. }) => {
                                scan_expr(iterable, diags, ctx, items);
                            }
                            Some(ForInit::PatternForIn { iterable, .. }) => {
                                scan_expr(iterable, diags, ctx, items);
                            }
                            Some(ForInit::Exprs(es)) => {
                                for e in es {
                                    scan_expr(e, diags, ctx, items);
                                }
                            }
                            None => {}
                        }
                        if let Some(c) = condition {
                            scan_expr(c, diags, ctx, items);
                        }
                        for u in updates {
                            scan_expr(u, diags, ctx, items);
                        }
                        if let CollectionElement::Expr(e) = &**element {
                            scan_expr(e, diags, ctx, items);
                        }
                    }
                }
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                scan_expr(&field.value, diags, ctx, items);
            }
        }
        Expr::Await { expr: e, .. } => scan_expr(e, diags, ctx, items),
        Expr::Throw { expr: e, .. } => scan_expr(e, diags, ctx, items),
        Expr::Switch { subject, arms, .. } => {
            scan_expr(subject, diags, ctx, items);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    scan_expr(guard, diags, ctx, items);
                }
                scan_expr(&arm.body, diags, ctx, items);
            }
        }
        Expr::NullAssert { operand, .. } => scan_expr(operand, diags, ctx, items),
        Expr::Assign { value, .. } => scan_expr(value, diags, ctx, items),
        Expr::As { expr: e, .. } => scan_expr(e, diags, ctx, items),
        Expr::Is { expr: e, .. } => scan_expr(e, diags, ctx, items),
        Expr::Cascade {
            object, sections, ..
        } => {
            scan_expr(object, diags, ctx, items);
            for section in sections {
                match &section.op {
                    CascadeOp::Field(_, _) => {}
                    CascadeOp::Index(e, _) => scan_expr(e, diags, ctx, items),
                    CascadeOp::Call(_, _, args) => {
                        for arg in &args.positional {
                            scan_expr(arg, diags, ctx, items);
                        }
                        for named in &args.named {
                            scan_expr(&named.value, diags, ctx, items);
                        }
                    }
                    CascadeOp::Assign(_, _, value) => scan_expr(value, diags, ctx, items),
                }
            }
        }
        Expr::PostfixIncDec { operand, .. } => scan_expr(operand, diags, ctx, items),
        _ => {}
    }
}

/// Base name of a `new`/`const` construction's type (`new Container()` -> `Container`).
fn new_base_name(dart_type: &DartType) -> Option<&str> {
    match dart_type {
        DartType::Named(named_type) => named_type.segments.last().map(|id| id.name.as_str()),
        _ => None,
    }
}

/// Base name of an implicit constructor call's callee.
/// `Container(...)`    -> `Expr::Ident("Container")`
/// `Container<T>(...)` -> `Expr::Call { callee: Ident("Container"), type_args, .. }` (type instantiation)
///
/// Deliberately does not resolve `Foo.named(...)` (a `Field` callee) so that static
/// method/getter access is not mistaken for construction.
fn callee_base_name(callee: &Expr) -> Option<&str> {
    match callee {
        Expr::Ident(id) => Some(id.name.as_str()),
        Expr::Call { callee, .. } => callee_base_name(callee),
        _ => None,
    }
}

/// Emit a diagnostic if `base_name` matches a configured design-system item.
fn emit_for_base(
    base_name: &str,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(String, Option<String>)],
) {
    for (class_name, use_instead) in items {
        if base_name == class_name {
            let message = if let Some(replacement) = use_instead {
                format!(
                    "Use '{}' from the design system instead of '{}'.",
                    replacement, class_name
                )
            } else {
                format!(
                    "Use the design system equivalent instead of '{}'.",
                    class_name
                )
            };

            diags.push(Diagnostic::new(
                "use-design-system-item",
                Severity::Warning,
                &message,
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
            break; // Only report once per construction
        }
    }
}
