use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct AvoidNestedIf;

impl Rule for AvoidNestedIf {
    fn name(&self) -> &'static str {
        "avoid_nested_if"
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
        TopLevelDecl::Function(f) => scan_opt_body(f.body.as_ref(), diags, ctx),
        TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::MixinClass(mc) => mc.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Enum(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        TopLevelDecl::Extension(ext) => ext.members.iter().for_each(|m| scan_member(m, diags, ctx)),
        _ => {}
    }
}

fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match member {
        ClassMember::Method(m) => scan_opt_body(m.body.as_ref(), diags, ctx),
        ClassMember::Constructor(c) => scan_opt_body(c.body.as_ref(), diags, ctx),
        ClassMember::Getter(g) => scan_opt_body(g.body.as_ref(), diags, ctx),
        ClassMember::Setter(s) => scan_opt_body(s.body.as_ref(), diags, ctx),
        ClassMember::Operator(o) => scan_opt_body(o.body.as_ref(), diags, ctx),
        _ => {}
    }
}

fn scan_opt_body(body: Option<&FunctionBody>, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(FunctionBody::Block(b)) = body {
        for s in &b.stmts {
            walk(s, false, diags, ctx);
        }
    }
}

/// True when a branch statement directly contains another `if` (used to detect
/// whether a nested `if` is itself the innermost in its chain).
fn branch_has_if(branch: &Stmt) -> bool {
    match branch {
        Stmt::If(_) => true,
        Stmt::Block(b) => b.stmts.iter().any(|s| matches!(s, Stmt::If(_))),
        _ => false,
    }
}

/// Walk a statement. `nested_in_if` is true when the statement sits inside the
/// then/else block of an enclosing `if`. An `if` is flagged only when it is
/// nested inside another `if` AND is itself the innermost (its then-branch holds
/// no further `if`), matching pyramid_lint's behavior of flagging the deepest
/// `if` of a vertical chain. Loops/switch/try reset the nesting context.
fn walk(stmt: &Stmt, nested_in_if: bool, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::If(if_stmt) => {
            if nested_in_if && !branch_has_if(&if_stmt.then_branch) {
                diags.push(Diagnostic::new(
                    "avoid_nested_if",
                    Severity::Warning,
                    "Avoid nesting if statements. Consider combining conditions or using early returns.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan { start: if_stmt.span.start, end: if_stmt.span.end },
                ));
            }
            walk(&if_stmt.then_branch, true, diags, ctx);
            if let Some(eb) = &if_stmt.else_branch {
                match eb.as_ref() {
                    // `else if` is a sibling chain, not nesting: preserve context.
                    Stmt::If(_) => walk(eb, nested_in_if, diags, ctx),
                    other => walk(other, true, diags, ctx),
                }
            }
        }
        Stmt::Block(b) => b.stmts.iter().for_each(|s| walk(s, nested_in_if, diags, ctx)),
        Stmt::While(w) => walk(&w.body, false, diags, ctx),
        Stmt::DoWhile(d) => walk(&d.body, false, diags, ctx),
        Stmt::For(f) => walk(&f.body, false, diags, ctx),
        Stmt::TryCatch(tc) => {
            tc.body.stmts.iter().for_each(|s| walk(s, false, diags, ctx));
            for catch in &tc.catches {
                catch.body.stmts.iter().for_each(|s| walk(s, false, diags, ctx));
            }
            if let Some(fin) = &tc.finally {
                fin.stmts.iter().for_each(|s| walk(s, false, diags, ctx));
            }
        }
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                case.body.iter().for_each(|s| walk(s, false, diags, ctx));
            }
        }
        Stmt::LocalFunc(lf) => {
            if let FunctionBody::Block(b) = &lf.body {
                b.stmts.iter().for_each(|s| walk(s, false, diags, ctx));
            }
        }
        _ => {}
    }
}
