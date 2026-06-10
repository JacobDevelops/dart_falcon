use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct NoDuplicateCaseValues;

impl Rule for NoDuplicateCaseValues {
    fn name(&self) -> &'static str {
        "no_duplicate_case_values"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            scan_top(decl, &mut diags, ctx);
        }
        diags
    }
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "no_duplicate_case_values",
        Severity::Warning,
        "Duplicate case value in switch statement.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan { start: span.start, end: span.end },
    ));
}

fn pattern_value_key(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Literal(lit) => match &lit.value {
            LiteralPatternValue::Null => Some("null".to_string()),
            LiteralPatternValue::Bool(b) => Some(b.to_string()),
            LiteralPatternValue::Int(s) => Some(format!("int:{}", s)),
            LiteralPatternValue::Double(s) => Some(format!("double:{}", s)),
            LiteralPatternValue::String(s) => Some(format!("str:{}", s.value)),
            LiteralPatternValue::NegInt(s) => Some(format!("negint:{}", s)),
            LiteralPatternValue::NegDouble(s) => Some(format!("negdouble:{}", s)),
        },
        Pattern::Const(const_pat) => {
            let name = const_pat
                .name
                .iter()
                .map(|id| id.name.as_str())
                .collect::<Vec<_>>()
                .join(".");
            Some(format!("const:{}", name))
        }
        _ => None,
    }
}

fn check_switch_cases(switch_stmt: &SwitchStmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let mut seen_values: std::collections::HashMap<String, (Span, Box<Pattern>)> = std::collections::HashMap::new();

    for case in &switch_stmt.cases {
        for case_kind in &case.cases {
            if let SwitchCaseKind::Pattern(pattern, _) = case_kind
                && let Some(key) = pattern_value_key(pattern) {
                    if let Some((_first_span, _first_pattern)) = seen_values.get(&key) {
                        // This is a duplicate; flag the current occurrence using the case span
                        flag(&case.span, diags, ctx);
                    } else {
                        seen_values.insert(key, (case.span.clone(), pattern.clone()));
                    }
                }
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
            check_switch_cases(sw, diags, ctx);
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
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
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
        _ => {}
    }
}
