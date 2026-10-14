//! Flags an `Expanded` wrapping an empty `SizedBox` or `Container` that should be a `Spacer`.
//!
//! Flutter's `Spacer` is exactly an `Expanded` whose child is an empty box, so
//! `Expanded(child: SizedBox())` or `Expanded(child: Container())` is a verbose
//! spelling of `Spacer()`. Using `Spacer` states the intent — reserve flexible
//! empty space in a `Row`, `Column`, or `Flex` — and drops a layer of nesting.
//! The rule matches an `Expanded` whose `child:` constructs a `Container` or
//! `SizedBox` with no `child:` argument of its own; other arguments such as
//! `color` do not disqualify the match, only a nested `child` does. Replace it
//! with `Spacer()`, moving any `flex:` across.
//! Checks functions and members of classes, mixins, enums, extensions, and extension types.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseSpacerAsExpandedChild;

impl Rule for UseSpacerAsExpandedChild {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            scan_top(declaration, &mut diags, ctx);
        }
        diags
    }
}

fn scan_top(declaration: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match declaration {
        TopLevelDecl::Function(function) => {
            if let Some(body) = &function.body {
                scan_body(body, diags, ctx);
            }
        }
        _ => {
            if let Some(members) = crate::lint::declarations::members_of(declaration) {
                for member in members {
                    scan_member(member, diags, ctx);
                }
            }
        }
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
        Stmt::TryCatch(tc) => scan_stmts(&tc.body.stmts, diags, ctx),
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
    if is_unqualified_construction(expr)
        && let Some((name, args, span)) = crate::lint::flutter::widget_construction(expr)
    {
        check_expanded(name, args, span, diags, ctx);
    }
    match expr {
        Expr::New { args, .. } => scan_args(args, diags, ctx),
        Expr::Call { callee, args, .. } => {
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
        Expr::List { .. } | Expr::Set { .. } | Expr::Map { .. } => {
            struct Children<'a, 'ctx> {
                diags: &'a mut Vec<Diagnostic>,
                ctx: &'a AnalyzeContext<'ctx>,
            }
            impl falcon_syntax::visitor::Visitor for Children<'_, '_> {
                fn visit_expr(&mut self, node: &Expr) {
                    scan_expr(node, self.diags, self.ctx);
                }
            }
            falcon_syntax::visitor::walk_expr(&mut Children { diags, ctx }, expr);
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

fn check_expanded(
    name: &str,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if name != "Expanded" {
        return;
    }
    for named in &args.named {
        if named.name.name == "child" && is_empty_spacer_widget(&named.value) {
            diags.push(ctx.diagnostic(
                "use-spacer-as-expanded-child",
                "Use Spacer() instead of an Expanded with an empty Container or SizedBox child.",
                *span,
            ));
            return;
        }
    }
}

fn is_empty_spacer_widget(expr: &Expr) -> bool {
    if !is_unqualified_construction(expr) {
        return false;
    }
    let Some((name, args, _)) = crate::lint::flutter::widget_construction(expr) else {
        return false;
    };
    if name != "Container" && name != "SizedBox" {
        return false;
    }
    !args.named.iter().any(|n| n.name.name == "child")
}

fn is_unqualified_construction(expr: &Expr) -> bool {
    !matches!(expr, Expr::New { dart_type: DartType::Named(named), .. }
        if named.segments.len() != 1)
}
