use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use std::collections::HashSet;

pub struct PreferUnderscoreForUnusedCallbackParameters;

impl Rule for PreferUnderscoreForUnusedCallbackParameters {
    fn name(&self) -> &'static str {
        "prefer_underscore_for_unused_callback_parameters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn is_underscore_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c == '_')
}

// ── Reference collection ─────────────────────────────────────────────────────
// A complete reference collector: every expression/statement form that can hold
// a nested reference is walked, including nested closures, cascades, and
// collection literals. This mirrors the fixed avoid-unused-parameters collector;
// an incomplete walker would miss a used parameter (e.g. one referenced only
// inside a nested closure) and falsely report it as unused.

fn collect_from_stmts(stmts: &[Stmt], names: &mut HashSet<String>) {
    for stmt in stmts {
        collect_from_stmt(stmt, names);
    }
}

fn collect_from_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Block(block) => collect_from_stmts(&block.stmts, names),
        Stmt::If(if_stmt) => {
            match &if_stmt.condition {
                IfCondition::Expr(expr) => collect_from_expr(expr, names),
                IfCondition::Case(expr, _) => collect_from_expr(expr, names),
            }
            collect_from_stmt(&if_stmt.then_branch, names);
            if let Some(else_stmt) = &if_stmt.else_branch {
                collect_from_stmt(else_stmt, names);
            }
        }
        Stmt::For(for_stmt) => {
            match &for_stmt.init {
                Some(ForInit::VarDecl(var)) => {
                    for decl in &var.declarators {
                        if let Some(init) = &decl.initializer {
                            collect_from_expr(init, names);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => collect_from_expr(iterable, names),
                Some(ForInit::PatternForIn { iterable, .. }) => collect_from_expr(iterable, names),
                Some(ForInit::Exprs(exprs)) => {
                    for expr in exprs {
                        collect_from_expr(expr, names);
                    }
                }
                None => {}
            }
            if let Some(cond) = &for_stmt.condition {
                collect_from_expr(cond, names);
            }
            for upd in &for_stmt.update {
                collect_from_expr(upd, names);
            }
            collect_from_stmt(&for_stmt.body, names);
        }
        Stmt::While(while_stmt) => {
            collect_from_expr(&while_stmt.condition, names);
            collect_from_stmt(&while_stmt.body, names);
        }
        Stmt::DoWhile(do_while_stmt) => {
            collect_from_expr(&do_while_stmt.condition, names);
            collect_from_stmt(&do_while_stmt.body, names);
        }
        Stmt::Switch(switch_stmt) => {
            collect_from_expr(&switch_stmt.subject, names);
            for case in &switch_stmt.cases {
                collect_from_stmts(&case.body, names);
            }
        }
        Stmt::TryCatch(try_catch) => {
            collect_from_stmts(&try_catch.body.stmts, names);
            for catch in &try_catch.catches {
                collect_from_stmts(&catch.body.stmts, names);
            }
            if let Some(finally) = &try_catch.finally {
                collect_from_stmts(&finally.stmts, names);
            }
        }
        Stmt::Return(return_stmt) => {
            if let Some(value) = &return_stmt.value {
                collect_from_expr(value, names);
            }
        }
        Stmt::Throw(throw_stmt) => collect_from_expr(&throw_stmt.value, names),
        Stmt::LocalVar(local_var) => {
            for decl in &local_var.declarators {
                if let Some(init) = &decl.initializer {
                    collect_from_expr(init, names);
                }
            }
        }
        Stmt::Expr(expr_stmt) => collect_from_expr(&expr_stmt.expr, names),
        Stmt::Assert(assert_stmt) => {
            collect_from_expr(&assert_stmt.condition, names);
            if let Some(msg) = &assert_stmt.message {
                collect_from_expr(msg, names);
            }
        }
        Stmt::Yield(yield_stmt) => collect_from_expr(&yield_stmt.value, names),
        Stmt::LocalFunc(local_func) => match &local_func.body {
            FunctionBody::Block(block) => collect_from_stmts(&block.stmts, names),
            FunctionBody::Arrow(expr, _) => collect_from_expr(expr, names),
            FunctionBody::Native(_, _) => {}
        },
        _ => {}
    }
}

fn collect_from_expr(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Ident(ident) => {
            names.insert(ident.name.clone());
        }
        Expr::StringLit(lit) => {
            // Interpolations are not decomposed in the AST; scan the raw text
            // for `$name` and `${ ... }` identifiers (over-collecting is safe:
            // it can only suppress a report, never invent one).
            let chars: Vec<char> = lit.raw.chars().collect();
            let is_ident_start = |c: char| c.is_alphabetic() || c == '_';
            let is_ident_cont = |c: char| c.is_alphanumeric() || c == '_';
            let mut i = 0;
            while i < chars.len() {
                if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
                    let mut depth = 1;
                    let mut j = i + 2;
                    while j < chars.len() && depth > 0 {
                        match chars[j] {
                            '{' => {
                                depth += 1;
                                j += 1;
                            }
                            '}' => {
                                depth -= 1;
                                j += 1;
                            }
                            c if is_ident_start(c) => {
                                let start = j;
                                while j < chars.len() && is_ident_cont(chars[j]) {
                                    j += 1;
                                }
                                names.insert(chars[start..j].iter().collect());
                            }
                            _ => j += 1,
                        }
                    }
                    i = j;
                    continue;
                }
                if chars[i] == '$' && i + 1 < chars.len() && is_ident_start(chars[i + 1]) {
                    let start = i + 1;
                    let mut end = start;
                    while end < chars.len() && is_ident_cont(chars[end]) {
                        end += 1;
                    }
                    names.insert(chars[start..end].iter().collect());
                    i = end;
                    continue;
                }
                i += 1;
            }
        }
        Expr::Unary { operand, .. } => collect_from_expr(operand, names),
        Expr::PostfixIncDec { operand, .. } => collect_from_expr(operand, names),
        Expr::Binary { left, right, .. } => {
            collect_from_expr(left, names);
            collect_from_expr(right, names);
        }
        Expr::Assign { target, value, .. } => {
            collect_from_expr(target, names);
            collect_from_expr(value, names);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            collect_from_expr(condition, names);
            collect_from_expr(then_expr, names);
            collect_from_expr(else_expr, names);
        }
        Expr::Is { expr, .. } => collect_from_expr(expr, names),
        Expr::As { expr, .. } => collect_from_expr(expr, names),
        Expr::Field { object, .. } => collect_from_expr(object, names),
        Expr::Index { object, index, .. } => {
            collect_from_expr(object, names);
            collect_from_expr(index, names);
        }
        Expr::Call { callee, args, .. } => {
            collect_from_expr(callee, names);
            for arg in &args.positional {
                collect_from_expr(arg, names);
            }
            for named_arg in &args.named {
                collect_from_expr(&named_arg.value, names);
            }
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            collect_from_expr(object, names);
            for section in sections {
                match &section.op {
                    CascadeOp::Field(_, _) => {}
                    CascadeOp::Index(index, _) => collect_from_expr(index, names),
                    CascadeOp::Call(_, _, args) => {
                        for arg in &args.positional {
                            collect_from_expr(arg, names);
                        }
                        for named_arg in &args.named {
                            collect_from_expr(&named_arg.value, names);
                        }
                    }
                    CascadeOp::Assign(target, _, value) => {
                        collect_from_expr(target, names);
                        collect_from_expr(value, names);
                    }
                }
            }
        }
        Expr::List { elements, .. } => {
            for elem in elements {
                collect_from_collection_elem(elem, names);
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for entry in entries {
                collect_from_expr(&entry.key, names);
                collect_from_expr(&entry.value, names);
            }
            for element in elements {
                collect_from_map_element(element, names);
            }
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                collect_from_collection_elem(elem, names);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                collect_from_expr(&field.value, names);
            }
        }
        Expr::FuncExpr { body, .. } => match body.as_ref() {
            FunctionBody::Block(block) => collect_from_stmts(&block.stmts, names),
            FunctionBody::Arrow(expr, _) => collect_from_expr(expr, names),
            FunctionBody::Native(_, _) => {}
        },
        Expr::New { args, .. } => {
            for arg in &args.positional {
                collect_from_expr(arg, names);
            }
            for named_arg in &args.named {
                collect_from_expr(&named_arg.value, names);
            }
        }
        Expr::Await { expr, .. } => collect_from_expr(expr, names),
        Expr::Throw { expr, .. } => collect_from_expr(expr, names),
        Expr::Switch { subject, arms, .. } => {
            collect_from_expr(subject, names);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_from_expr(guard, names);
                }
                collect_from_expr(&arm.body, names);
            }
        }
        Expr::NullAssert { operand, .. } => collect_from_expr(operand, names),
        _ => {}
    }
}

fn collect_from_collection_elem(elem: &CollectionElement, names: &mut HashSet<String>) {
    match elem {
        CollectionElement::Expr(expr) => collect_from_expr(expr, names),
        CollectionElement::NullAware { expr, .. } => collect_from_expr(expr, names),
        CollectionElement::Spread { expr, .. } => collect_from_expr(expr, names),
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            if let IfCondition::Expr(cond_expr) = condition {
                collect_from_expr(cond_expr, names);
            }
            collect_from_collection_elem(then_elem, names);
            if let Some(else_el) = else_elem {
                collect_from_collection_elem(else_el, names);
            }
        }
        CollectionElement::For {
            iterable, element, ..
        } => {
            collect_from_expr(iterable, names);
            collect_from_collection_elem(element, names);
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
                            collect_from_expr(e, names);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => collect_from_expr(iterable, names),
                Some(ForInit::PatternForIn { iterable, .. }) => collect_from_expr(iterable, names),
                Some(ForInit::Exprs(es)) => {
                    for e in es {
                        collect_from_expr(e, names);
                    }
                }
                None => {}
            }
            if let Some(c) = condition {
                collect_from_expr(c, names);
            }
            for u in updates {
                collect_from_expr(u, names);
            }
            collect_from_collection_elem(element, names);
        }
    }
}

fn collect_from_map_element(elem: &MapElement, names: &mut HashSet<String>) {
    match elem {
        MapElement::Entry(entry) => {
            collect_from_expr(&entry.key, names);
            collect_from_expr(&entry.value, names);
        }
        MapElement::Spread { expr, .. } => collect_from_expr(expr, names),
        MapElement::If {
            condition,
            then_entry,
            else_entry,
            ..
        } => {
            if let IfCondition::Expr(cond_expr) = condition {
                collect_from_expr(cond_expr, names);
            }
            collect_from_map_element(then_entry, names);
            if let Some(else_el) = else_entry {
                collect_from_map_element(else_el, names);
            }
        }
        MapElement::For {
            iterable, entry, ..
        } => {
            collect_from_expr(iterable, names);
            collect_from_map_element(entry, names);
        }
        MapElement::CFor {
            init,
            condition,
            updates,
            entry,
            ..
        } => {
            match init {
                Some(ForInit::VarDecl(d)) => {
                    for decl in &d.declarators {
                        if let Some(e) = &decl.initializer {
                            collect_from_expr(e, names);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => collect_from_expr(iterable, names),
                Some(ForInit::PatternForIn { iterable, .. }) => collect_from_expr(iterable, names),
                Some(ForInit::Exprs(es)) => {
                    for e in es {
                        collect_from_expr(e, names);
                    }
                }
                None => {}
            }
            if let Some(c) = condition {
                collect_from_expr(c, names);
            }
            for u in updates {
                collect_from_expr(u, names);
            }
            collect_from_map_element(entry, names);
        }
    }
}

// ── Closure discovery ────────────────────────────────────────────────────────

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
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
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
            match &i.condition {
                IfCondition::Expr(e) => scan_expr(e, diags, ctx),
                IfCondition::Case(e, _) => scan_expr(e, diags, ctx),
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
            scan_stmt(&f.body, diags, ctx);
        }
        Stmt::Switch(sw) => {
            scan_expr(&sw.subject, diags, ctx);
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

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::FuncExpr {
            params, body, span, ..
        } => {
            check_closure_params_inline(params, body, diags, ctx, span);
            match &**body {
                FunctionBody::Block(_) | FunctionBody::Native(_, _) => {
                    scan_body(body, diags, ctx);
                }
                FunctionBody::Arrow(inner_expr, _) => {
                    scan_expr(inner_expr, diags, ctx);
                }
            }
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
        Expr::Index { object, index, .. } => {
            scan_expr(object, diags, ctx);
            scan_expr(index, diags, ctx);
        }
        Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
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
        _ => {}
    }
}

fn check_closure_params_inline(
    params: &FormalParamList,
    body: &FunctionBody,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    closure_span: &Span,
) {
    let all_params: Vec<_> = params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
        .collect();

    if all_params.is_empty() {
        return;
    }

    let mut used_names = HashSet::new();
    match body {
        FunctionBody::Block(b) => collect_from_stmts(&b.stmts, &mut used_names),
        FunctionBody::Arrow(e, _) => collect_from_expr(e, &mut used_names),
        FunctionBody::Native(_, _) => {}
    }

    // Flag the first unused, non-underscore parameter (one diagnostic per
    // closure).
    for param in &all_params {
        let param_name = &param.name.name;
        if !is_underscore_name(param_name) && !used_names.contains(param_name) {
            diags.push(Diagnostic::new(
                "prefer_underscore_for_unused_callback_parameters",
                Severity::Warning,
                "Unused callback parameter should be named '_'.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: closure_span.start,
                    end: closure_span.end,
                },
            ));
            break;
        }
    }
}
