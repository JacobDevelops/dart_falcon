//! Flags a function whose cyclomatic complexity exceeds the configured threshold.
//!
//! Cyclomatic complexity counts the independent paths through a function; a high
//! value means many branches to understand and test. The rule computes
//! complexity as one plus each decision point — `if`, ternary, `&&`, `||`, `??`,
//! loops (`for`/`while`/`do`), `catch` clauses, non-default `case`s, and pattern
//! `when` guards — counted across the whole body tree, so decision points inside
//! nested closures and local functions count toward the enclosing function. It
//! reports at the function name when the total exceeds the threshold.
//!
//! ## Options
//!
//! `max_complexity` (integer, default: 20) — flag when the computed complexity
//! exceeds this.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct CyclomaticComplexity;

impl Rule for CyclomaticComplexity {
    fn name(&self) -> &'static str {
        "cyclomatic-complexity"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

/// Read the `max_complexity` option (default 20). Malformed/missing → default.
fn max_complexity_option(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("cyclomatic-complexity")
        .and_then(|m| ctx.rule_options(m.group, "cyclomatic-complexity"))
        .and_then(|o| o.get("max_complexity"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(20)
}

/// Compute cyclomatic complexity for one function-like body and emit a
/// diagnostic at `name_span` when it exceeds the configured threshold.
///
/// Complexity = 1 (base) + number of decision points found *anywhere* in the
/// body tree. We take the simple "count everything in the body tree" approach:
/// decision points inside nested closures and local functions are counted
/// toward this enclosing function rather than being reported separately.
fn check_function(
    body: &FunctionBody,
    name_span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    let threshold = max_complexity_option(ctx);
    let mut count = 0;
    count_body(body, &mut count);
    let complexity = count + 1;
    if complexity > threshold {
        diags.push(Diagnostic::new(
            "cyclomatic-complexity",
            Severity::Warning,
            format!("Function has a cyclomatic complexity of {complexity} (max {threshold})."),
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: name_span.start,
                end: name_span.end,
            },
        ));
    }
}

fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match decl {
        TopLevelDecl::Function(f) => {
            if let Some(body) = &f.body {
                check_function(body, &f.name.span, diags, ctx);
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
        TopLevelDecl::Extension(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::ExtensionType(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        TopLevelDecl::Enum(e) => {
            for m in &e.members {
                scan_member(m, diags, ctx);
            }
        }
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Method(m) => {
            if let Some(b) = &m.body {
                check_function(b, &m.name.span, diags, ctx);
            }
        }
        ClassMember::Constructor(c) => {
            if let Some(b) = &c.body {
                check_function(b, &c.name.span, diags, ctx);
            }
        }
        ClassMember::Getter(g) => {
            if let Some(b) = &g.body {
                check_function(b, &g.name.span, diags, ctx);
            }
        }
        ClassMember::Setter(s) => {
            if let Some(b) = &s.body {
                check_function(b, &s.name.span, diags, ctx);
            }
        }
        _ => {}
    }
}

// ── Decision-point counting ───────────────────────────────────────────────────

fn count_body(body: &FunctionBody, count: &mut usize) {
    match body {
        FunctionBody::Block(b) => count_stmts(&b.stmts, count),
        FunctionBody::Arrow(e, _) => count_expr(e, count),
        FunctionBody::Native(_, _) => {}
    }
}

fn count_stmts(stmts: &[Stmt], count: &mut usize) {
    for s in stmts {
        count_stmt(s, count);
    }
}

fn count_stmt(stmt: &Stmt, count: &mut usize) {
    match stmt {
        Stmt::Block(b) => count_stmts(&b.stmts, count),
        Stmt::If(i) => {
            *count += 1;
            count_if_condition(&i.condition, count);
            count_stmt(&i.then_branch, count);
            if let Some(e) = &i.else_branch {
                count_stmt(e, count);
            }
        }
        Stmt::For(f) => {
            *count += 1;
            if let Some(init) = &f.init {
                count_for_init(init, count);
            }
            if let Some(c) = &f.condition {
                count_expr(c, count);
            }
            for u in &f.update {
                count_expr(u, count);
            }
            count_stmt(&f.body, count);
        }
        Stmt::While(w) => {
            *count += 1;
            count_expr(&w.condition, count);
            count_stmt(&w.body, count);
        }
        Stmt::DoWhile(d) => {
            *count += 1;
            count_stmt(&d.body, count);
            count_expr(&d.condition, count);
        }
        Stmt::Switch(sw) => {
            count_expr(&sw.subject, count);
            for case in &sw.cases {
                for kind in &case.cases {
                    if let SwitchCaseKind::Pattern(_, guard) = kind {
                        *count += 1;
                        if guard.is_some() {
                            *count += 1;
                        }
                    }
                }
                count_stmts(&case.body, count);
            }
        }
        Stmt::TryCatch(tc) => {
            count_stmts(&tc.body.stmts, count);
            for catch in &tc.catches {
                *count += 1;
                count_stmts(&catch.body.stmts, count);
            }
            if let Some(fin) = &tc.finally {
                count_stmts(&fin.stmts, count);
            }
        }
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                count_expr(v, count);
            }
        }
        Stmt::Throw(t) => count_expr(&t.value, count),
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    count_expr(init, count);
                }
            }
        }
        Stmt::LocalFunc(lf) => count_body(&lf.body, count),
        Stmt::Yield(y) => count_expr(&y.value, count),
        Stmt::Assert(a) => {
            count_expr(&a.condition, count);
            if let Some(m) = &a.message {
                count_expr(m, count);
            }
        }
        Stmt::Expr(e) => count_expr(&e.expr, count),
        Stmt::Labeled(l) => count_stmt(&l.stmt, count),
        _ => {}
    }
}

fn count_if_condition(cond: &IfCondition, count: &mut usize) {
    match cond {
        IfCondition::Expr(e) => count_expr(e, count),
        IfCondition::Case(e, _, guard) => {
            count_expr(e, count);
            if let Some(g) = guard {
                *count += 1;
                count_expr(g, count);
            }
        }
    }
}

fn count_for_init(init: &ForInit, count: &mut usize) {
    match init {
        ForInit::VarDecl(lv) => {
            for d in &lv.declarators {
                if let Some(i) = &d.initializer {
                    count_expr(i, count);
                }
            }
        }
        ForInit::ForIn { iterable, .. } => count_expr(iterable, count),
        ForInit::PatternForIn { iterable, .. } => count_expr(iterable, count),
        ForInit::Exprs(es) => {
            for e in es {
                count_expr(e, count);
            }
        }
    }
}

fn count_expr(expr: &Expr, count: &mut usize) {
    match expr {
        Expr::Unary { operand, .. } => count_expr(operand, count),
        Expr::PostfixIncDec { operand, .. } => count_expr(operand, count),
        Expr::Binary {
            op, left, right, ..
        } => {
            if matches!(
                op,
                BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce | BinaryOp::IfNull
            ) {
                *count += 1;
            }
            count_expr(left, count);
            count_expr(right, count);
        }
        Expr::Assign { target, value, .. } => {
            count_expr(target, count);
            count_expr(value, count);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            *count += 1;
            count_expr(condition, count);
            count_expr(then_expr, count);
            count_expr(else_expr, count);
        }
        Expr::Is { expr, .. } => count_expr(expr, count),
        Expr::As { expr, .. } => count_expr(expr, count),
        Expr::Field { object, .. } => count_expr(object, count),
        Expr::Index { object, index, .. } => {
            count_expr(object, count);
            count_expr(index, count);
        }
        Expr::Call { callee, args, .. } => {
            count_expr(callee, count);
            count_args(args, count);
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            count_expr(object, count);
            for s in sections {
                count_cascade(s, count);
            }
        }
        Expr::List { elements, .. } | Expr::Set { elements, .. } => {
            for el in elements {
                count_collection_element(el, count);
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for e in entries {
                count_expr(&e.key, count);
                count_expr(&e.value, count);
            }
            for e in map_element_exprs(elements) {
                count_expr(e, count);
            }
        }
        Expr::Record { fields, .. } => {
            for f in fields {
                count_expr(&f.value, count);
            }
        }
        Expr::FuncExpr { body, .. } => count_body(body, count),
        Expr::New { args, .. } => count_args(args, count),
        Expr::Await { expr, .. } => count_expr(expr, count),
        Expr::Throw { expr, .. } => count_expr(expr, count),
        Expr::Switch { subject, arms, .. } => {
            count_expr(subject, count);
            for arm in arms {
                *count += 1;
                if arm.guard.is_some() {
                    *count += 1;
                }
                count_expr(&arm.body, count);
            }
        }
        Expr::NullAssert { operand, .. } => count_expr(operand, count),
        _ => {}
    }
}

fn count_args(args: &ArgList, count: &mut usize) {
    for a in &args.positional {
        count_expr(a, count);
    }
    for n in &args.named {
        count_expr(&n.value, count);
    }
}

fn count_cascade(section: &CascadeSection, count: &mut usize) {
    for op in &section.ops {
        match op {
            CascadeOp::Index(e, _) => count_expr(e, count),
            CascadeOp::Call(_, _, args) => count_args(args, count),
            CascadeOp::Assign(target, _, value) => {
                count_expr(target, count);
                count_expr(value, count);
            }
            CascadeOp::Field(_, _) => {}
        }
    }
}

fn count_collection_element(el: &CollectionElement, count: &mut usize) {
    match el {
        CollectionElement::Expr(e) => count_expr(e, count),
        CollectionElement::NullAware { expr, .. } => count_expr(expr, count),
        CollectionElement::Spread { expr, .. } => count_expr(expr, count),
        CollectionElement::If {
            then_elem,
            else_elem,
            ..
        } => {
            // Collection-level `if` is intentionally not counted as a decision
            // point (kept simple); still recurse to find nested closures.
            count_collection_element(then_elem, count);
            if let Some(e) = else_elem {
                count_collection_element(e, count);
            }
        }
        CollectionElement::For {
            iterable, element, ..
        } => {
            count_expr(iterable, count);
            count_collection_element(element, count);
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
                            count_expr(e, count);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => {
                    count_expr(iterable, count);
                }
                Some(ForInit::PatternForIn { iterable, .. }) => {
                    count_expr(iterable, count);
                }
                Some(ForInit::Exprs(es)) => {
                    for e in es {
                        count_expr(e, count);
                    }
                }
                None => {}
            }
            if let Some(c) = condition {
                count_expr(c, count);
            }
            for u in updates {
                count_expr(u, count);
            }
            count_collection_element(element, count);
        }
    }
}
