use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidIgnoringReturnValues;

impl Rule for AvoidIgnoringReturnValues {
    fn name(&self) -> &'static str {
        "avoid-ignoring-return-values"
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
        Stmt::Expr(e) => {
            check_ignored_return_value(&e.expr, diags, ctx);
            scan_expr(&e.expr, diags, ctx);
        }
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
        Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx),
        Stmt::If(i) => {
            if let IfCondition::Expr(e) = &i.condition {
                scan_expr(e, diags, ctx);
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
            if let Some(ForInit::VarDecl(lv)) = &f.init {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx);
                    }
                }
            }
            if let Some(cond) = &f.condition {
                scan_expr(cond, diags, ctx);
            }
            for u in &f.update {
                scan_expr(u, diags, ctx);
            }
            scan_stmt(&f.body, diags, ctx);
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
        Stmt::Assert(a) => {
            scan_expr(&a.condition, diags, ctx);
            if let Some(msg) = &a.message {
                scan_expr(msg, diags, ctx);
            }
        }
        Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
        _ => {}
    }
}

/// Method/function names that overwhelmingly denote side effects (their return
/// value, if any, is conventionally discarded). Without type resolution falcon
/// cannot know a call's real return type, so this static list is the best
/// heuristic to suppress the dominant false positives seen on real code:
/// lifecycle hooks, collection mutation, logging, stream/sink writes, disposal,
/// and navigation. Derived from adopter-codebase sampling + dart_code_linter's
/// known-void exemptions. (This rule is off in the recommended preset — see its
/// metadata note — because it is inherently noisy without a type system.)
const SIDE_EFFECT_NAMES: &[&str] = &[
    // logging / debug
    "print",
    "printSync",
    "log",
    "info",
    "warn",
    "warning",
    "debug",
    "error",
    "fine",
    "severe",
    // collection mutation
    "add",
    "addAll",
    "insert",
    "insertAll",
    "remove",
    "removeAt",
    "removeLast",
    "removeWhere",
    "removeRange",
    "retainWhere",
    "clear",
    "sort",
    "shuffle",
    "fillRange",
    "setAll",
    "setRange",
    "forEach",
    "putIfAbsent",
    "update",
    "updateAll",
    // listeners / notifiers / streams / sinks
    "addListener",
    "removeListener",
    "notifyListeners",
    "emit",
    "sink",
    "write",
    "writeln",
    "writeAll",
    "complete",
    "completeError",
    "cancel",
    "close",
    "flush",
    "send",
    "seek",
    // widget / framework lifecycle
    "setState",
    "initState",
    "dispose",
    "didChangeDependencies",
    "didUpdateWidget",
    "deactivate",
    "markNeedsBuild",
    "addPostFrameCallback",
    "unawaited",
    // navigation
    "pop",
    "push",
    "pushNamed",
    "pushReplacement",
    "popUntil",
    "maybePop",
    "showDialog",
    // misc common side effects
    "save",
    "delete",
    "reset",
    "start",
    "stop",
    "play",
    "pause",
    "throwWithStackTrace",
];

fn is_side_effect_name(name: &str) -> bool {
    SIDE_EFFECT_NAMES.contains(&name)
}

/// Base name of a call's callee, for side-effect matching. Handles both bare
/// identifiers (`foo()`) and member calls (`x.foo()`).
fn callee_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(id) => Some(id.name.as_str()),
        Expr::Field { field, .. } => Some(field.name.as_str()),
        _ => None,
    }
}

fn check_ignored_return_value(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::Call { callee, .. } => {
            if !callee_name(callee).is_some_and(is_side_effect_name) {
                diags.push(Diagnostic::new(
                    "avoid-ignoring-return-values",
                    Severity::Warning,
                    "The return value is not being used",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: expr.span().start,
                        end: expr.span().end,
                    },
                ));
            }
        }
        Expr::Field { field, .. } if !is_side_effect_name(field.name.as_str()) => {
            diags.push(Diagnostic::new(
                "avoid-ignoring-return-values",
                Severity::Warning,
                "The return value is not being used",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: expr.span().start,
                    end: expr.span().end,
                },
            ));
        }
        _ => {}
    }
}

fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
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
        Expr::Assign { target, value, .. } => {
            scan_expr(target, diags, ctx);
            scan_expr(value, diags, ctx);
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
        Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
        Expr::Await { expr, .. } => scan_expr(expr, diags, ctx),
        Expr::Binary { left, right, .. } => {
            scan_expr(left, diags, ctx);
            scan_expr(right, diags, ctx);
        }
        _ => {}
    }
}
