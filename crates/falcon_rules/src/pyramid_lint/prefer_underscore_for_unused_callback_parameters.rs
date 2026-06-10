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
    name == "_" || name == "__" || name == "___" || (name.starts_with('_') && name.chars().all(|c| c == '_'))
}

fn collect_identifiers_in_expr(expr: &Expr, names: &mut HashSet<String>) {
    match expr {
        Expr::Ident(id) => {
            names.insert(id.name.clone());
        }
        Expr::StringLit(s) => {
            // Check for variable references in string interpolations
            // String interpolations in Dart use $varName or ${expression}
            let raw = &s.raw;
            let bytes = raw.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'$' && i + 1 < bytes.len() {
                    i += 1;
                    if bytes[i] == b'{' {
                        // Skip ${...} expressions - they're complex
                        i += 1;
                        let mut depth = 1;
                        while i < bytes.len() && depth > 0 {
                            if bytes[i] == b'{' {
                                depth += 1;
                            } else if bytes[i] == b'}' {
                                depth -= 1;
                            }
                            i += 1;
                        }
                    } else if (bytes[i] as char).is_alphabetic() || bytes[i] == b'_' {
                        // Collect $identifier
                        let mut ident = String::new();
                        while i < bytes.len() && ((bytes[i] as char).is_alphanumeric() || bytes[i] == b'_') {
                            ident.push(bytes[i] as char);
                            i += 1;
                        }
                        names.insert(ident);
                    } else {
                        i += 1;
                    }
                } else {
                    i += 1;
                }
            }
        }
        Expr::Field { object, .. } => collect_identifiers_in_expr(object, names),
        Expr::Index { object, index, .. } => {
            collect_identifiers_in_expr(object, names);
            collect_identifiers_in_expr(index, names);
        }
        Expr::Call { callee, args, .. } => {
            collect_identifiers_in_expr(callee, names);
            for arg in &args.positional {
                collect_identifiers_in_expr(arg, names);
            }
            for named in &args.named {
                collect_identifiers_in_expr(&named.value, names);
            }
        }
        Expr::Unary { operand, .. } => collect_identifiers_in_expr(operand, names),
        Expr::Binary { left, right, .. } => {
            collect_identifiers_in_expr(left, names);
            collect_identifiers_in_expr(right, names);
        }
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            collect_identifiers_in_expr(condition, names);
            collect_identifiers_in_expr(then_expr, names);
            collect_identifiers_in_expr(else_expr, names);
        }
        Expr::FuncExpr { .. } => {
            // Don't recurse into nested closures
        }
        _ => {}
    }
}

fn collect_identifiers_in_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
    match stmt {
        Stmt::Expr(e) => collect_identifiers_in_expr(&e.expr, names),
        Stmt::Return(r) => {
            if let Some(v) = &r.value {
                collect_identifiers_in_expr(v, names);
            }
        }
        Stmt::LocalVar(lv) => {
            for d in &lv.declarators {
                if let Some(init) = &d.initializer {
                    collect_identifiers_in_expr(init, names);
                }
            }
        }
        Stmt::Block(b) => {
            for s in &b.stmts {
                collect_identifiers_in_stmt(s, names);
            }
        }
        Stmt::If(i) => {
            match &i.condition {
                IfCondition::Expr(e) => collect_identifiers_in_expr(e, names),
                IfCondition::Case(e, _) => collect_identifiers_in_expr(e, names),
            }
            collect_identifiers_in_stmt(&i.then_branch, names);
            if let Some(eb) = &i.else_branch {
                collect_identifiers_in_stmt(eb, names);
            }
        }
        Stmt::While(w) => {
            collect_identifiers_in_expr(&w.condition, names);
            collect_identifiers_in_stmt(&w.body, names);
        }
        Stmt::DoWhile(d) => {
            collect_identifiers_in_stmt(&d.body, names);
            collect_identifiers_in_expr(&d.condition, names);
        }
        Stmt::For(f) => {
            if let Some(cond) = &f.condition {
                collect_identifiers_in_expr(cond, names);
            }
            collect_identifiers_in_stmt(&f.body, names);
        }
        Stmt::Switch(sw) => {
            collect_identifiers_in_expr(&sw.subject, names);
            for case in &sw.cases {
                for s in &case.body {
                    collect_identifiers_in_stmt(s, names);
                }
            }
        }
        Stmt::TryCatch(tc) => {
            for s in &tc.body.stmts {
                collect_identifiers_in_stmt(s, names);
            }
            for catch in &tc.catches {
                for s in &catch.body.stmts {
                    collect_identifiers_in_stmt(s, names);
                }
            }
            if let Some(fin) = &tc.finally {
                for s in &fin.stmts {
                    collect_identifiers_in_stmt(s, names);
                }
            }
        }
        _ => {}
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
        Expr::FuncExpr { params, body, span, .. } => {
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
        Expr::Conditional { condition, then_expr, else_expr, .. } => {
            scan_expr(condition, diags, ctx);
            scan_expr(then_expr, diags, ctx);
            scan_expr(else_expr, diags, ctx);
        }
        _ => {}
    }
}

fn check_closure_params_inline(params: &FormalParamList, body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, closure_span: &Span) {
    let all_params: Vec<_> = params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
        .collect();

    // Don't flag if there are no parameters
    if all_params.is_empty() {
        return;
    }

    // Collect all identifiers used in the closure body
    let mut used_names = HashSet::new();
    match body {
        FunctionBody::Block(b) => {
            for stmt in &b.stmts {
                collect_identifiers_in_stmt(stmt, &mut used_names);
            }
        }
        FunctionBody::Arrow(e, _) => {
            collect_identifiers_in_expr(e, &mut used_names);
        }
        FunctionBody::Native(_, _) => {}
    }

    // Flag first unused parameter (one diagnostic per closure)
    for _param in &all_params {
        let param_name = &_param.name.name;
        if !is_underscore_name(param_name) && !used_names.contains(param_name) {
            diags.push(Diagnostic::new(
                "prefer_underscore_for_unused_callback_parameters",
                Severity::Warning,
                "Unused callback parameter should be named '_'.",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan { start: closure_span.start, end: closure_span.end },
            ));
            break;  // Only flag the first unused parameter
        }
    }
}
