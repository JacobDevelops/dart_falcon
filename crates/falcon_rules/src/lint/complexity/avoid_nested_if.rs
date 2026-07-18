//! Flags unnecessarily nested `if` statements. Ported from pyramid_lint's `avoid_nested_if`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidNestedIf;

/// pyramid_lint's default `max_nesting_level`: an `if` is flagged when its
/// then-branch subtree contains at least this many `if` statements.
const MAX_NESTING_LEVEL: usize = 2;

impl Rule for AvoidNestedIf {
    fn name(&self) -> &'static str {
        "avoid-nested-if"
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
            walk(s, diags, ctx);
        }
    }
}

/// Count `if` statements in the subtree rooted at `stmt`, counting `stmt`
/// itself when it is an `if`. Mirrors `RecursiveIfStatementVisitor`.
fn count_ifs_including(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::If(i) => {
            1 + count_ifs_including(&i.then_branch)
                + i.else_branch.as_deref().map_or(0, count_ifs_including)
        }
        Stmt::Block(b) => b.stmts.iter().map(count_ifs_including).sum(),
        Stmt::While(w) => count_ifs_including(&w.body),
        Stmt::DoWhile(d) => count_ifs_including(&d.body),
        Stmt::For(f) => count_ifs_including(&f.body),
        Stmt::TryCatch(tc) => {
            let body: usize = tc.body.stmts.iter().map(count_ifs_including).sum();
            let catches: usize = tc
                .catches
                .iter()
                .flat_map(|c| c.body.stmts.iter())
                .map(count_ifs_including)
                .sum();
            let finally: usize = tc
                .finally
                .as_ref()
                .map(|f| f.stmts.iter().map(count_ifs_including).sum())
                .unwrap_or(0);
            body + catches + finally
        }
        Stmt::Switch(sw) => sw
            .cases
            .iter()
            .flat_map(|c| c.body.iter())
            .map(count_ifs_including)
            .sum(),
        Stmt::LocalFunc(lf) => match &lf.body {
            FunctionBody::Block(b) => b.stmts.iter().map(count_ifs_including).sum(),
            _ => 0,
        },
        _ => 0,
    }
}

/// Number of `if` statements strictly *within* a then-branch (its descendants),
/// which is what pyramid_lint counts via `thenStatement.visitChildren`.
fn if_descendants_of_then(then_branch: &Stmt) -> usize {
    let including = count_ifs_including(then_branch);
    // `visitChildren` never visits the then-branch node itself, so a then-branch
    // that *is* an `if` should not count toward its own nesting total.
    if matches!(then_branch, Stmt::If(_)) {
        including.saturating_sub(1)
    } else {
        including
    }
}

/// Visit every `if` statement, flagging those whose then-branch contains at
/// least `MAX_NESTING_LEVEL` nested `if` statements.
fn walk(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::If(if_stmt) => {
            if if_descendants_of_then(&if_stmt.then_branch) >= MAX_NESTING_LEVEL {
                diags.push(Diagnostic::new(
                    "avoid-nested-if",
                    Severity::Warning,
                    "Avoid nesting if statements. Consider combining conditions or using early returns.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan { start: if_stmt.span.start, end: if_stmt.span.end },
                ));
            }
            walk(&if_stmt.then_branch, diags, ctx);
            if let Some(eb) = &if_stmt.else_branch {
                walk(eb, diags, ctx);
            }
        }
        Stmt::Block(b) => b.stmts.iter().for_each(|s| walk(s, diags, ctx)),
        Stmt::While(w) => walk(&w.body, diags, ctx),
        Stmt::DoWhile(d) => walk(&d.body, diags, ctx),
        Stmt::For(f) => walk(&f.body, diags, ctx),
        Stmt::TryCatch(tc) => {
            tc.body.stmts.iter().for_each(|s| walk(s, diags, ctx));
            for catch in &tc.catches {
                catch.body.stmts.iter().for_each(|s| walk(s, diags, ctx));
            }
            if let Some(fin) = &tc.finally {
                fin.stmts.iter().for_each(|s| walk(s, diags, ctx));
            }
        }
        Stmt::Switch(sw) => {
            for case in &sw.cases {
                case.body.iter().for_each(|s| walk(s, diags, ctx));
            }
        }
        Stmt::LocalFunc(lf) => {
            if let FunctionBody::Block(b) = &lf.body {
                b.stmts.iter().for_each(|s| walk(s, diags, ctx));
            }
        }
        _ => {}
    }
}
