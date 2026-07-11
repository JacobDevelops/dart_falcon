//! Flags nested ternary conditional expressions. Ported from dart_code_linter's `avoid-nested-conditional-expressions`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidNestedConditionalExpressions;

impl Rule for AvoidNestedConditionalExpressions {
    fn name(&self) -> &'static str {
        "avoid-nested-conditional-expressions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            visit_top_level_decl(decl, &mut diags, ctx);
        }
        diags
    }
}

fn visit_top_level_decl(
    decl: &TopLevelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match decl {
        TopLevelDecl::Class(class) => {
            for member in &class.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::Function(func) => {
            if let Some(body) = &func.body {
                visit_function_body(body, diagnostics, ctx);
            }
        }
        TopLevelDecl::Variable(var) => {
            for declarator in &var.declarators {
                if let Some(expr) = &declarator.initializer {
                    visit_expr(expr, false, diagnostics, ctx);
                }
            }
        }
        TopLevelDecl::Mixin(mixin) => {
            for member in &mixin.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::MixinClass(mixin_class) => {
            for member in &mixin_class.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::Enum(enum_decl) => {
            for member in &enum_decl.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::Extension(ext) => {
            for member in &ext.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::ExtensionType(_) => {}
        TopLevelDecl::TypeAlias(_) => {}
        TopLevelDecl::Error(_) => {}
    }
}

fn visit_class_member(
    member: &ClassMember,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match member {
        ClassMember::Field(field) => {
            for declarator in &field.declarators {
                if let Some(expr) = &declarator.initializer {
                    visit_expr(expr, false, diagnostics, ctx);
                }
            }
        }
        ClassMember::Constructor(ctor) => {
            if let Some(body) = &ctor.body {
                visit_function_body(body, diagnostics, ctx);
            }
        }
        ClassMember::Method(method) => {
            if let Some(body) = &method.body {
                visit_function_body(body, diagnostics, ctx);
            }
        }
        ClassMember::Getter(getter) => {
            if let Some(body) = &getter.body {
                visit_function_body(body, diagnostics, ctx);
            }
        }
        ClassMember::Setter(setter) => {
            if let Some(body) = &setter.body {
                visit_function_body(body, diagnostics, ctx);
            }
        }
        ClassMember::Operator(_) => {}
        ClassMember::Error(_) => {}
    }
}

fn visit_function_body(
    body: &FunctionBody,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match body {
        FunctionBody::Block(block) => {
            for stmt in &block.stmts {
                visit_stmt(stmt, diagnostics, ctx);
            }
        }
        FunctionBody::Arrow(expr, _) => {
            visit_expr(expr, false, diagnostics, ctx);
        }
        FunctionBody::Native(..) => {}
    }
}

fn visit_stmt(stmt: &Stmt, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Block(block) => {
            for s in &block.stmts {
                visit_stmt(s, diagnostics, ctx);
            }
        }
        Stmt::If(if_stmt) => {
            if let IfCondition::Expr(expr) = &if_stmt.condition {
                visit_expr(expr, false, diagnostics, ctx);
            }
            visit_stmt(&if_stmt.then_branch, diagnostics, ctx);
            if let Some(else_branch) = &if_stmt.else_branch {
                visit_stmt(else_branch, diagnostics, ctx);
            }
        }
        Stmt::For(for_stmt) => {
            if let Some(condition) = &for_stmt.condition {
                visit_expr(condition, false, diagnostics, ctx);
            }
            for expr in &for_stmt.update {
                visit_expr(expr, false, diagnostics, ctx);
            }
            visit_stmt(&for_stmt.body, diagnostics, ctx);
        }
        Stmt::While(while_stmt) => {
            visit_expr(&while_stmt.condition, false, diagnostics, ctx);
            visit_stmt(&while_stmt.body, diagnostics, ctx);
        }
        Stmt::DoWhile(do_while_stmt) => {
            visit_stmt(&do_while_stmt.body, diagnostics, ctx);
            visit_expr(&do_while_stmt.condition, false, diagnostics, ctx);
        }
        Stmt::Switch(switch_stmt) => {
            visit_expr(&switch_stmt.subject, false, diagnostics, ctx);
            for case in &switch_stmt.cases {
                for s in &case.body {
                    visit_stmt(s, diagnostics, ctx);
                }
            }
        }
        Stmt::TryCatch(try_catch) => {
            for s in &try_catch.body.stmts {
                visit_stmt(s, diagnostics, ctx);
            }
            for catch_clause in &try_catch.catches {
                for s in &catch_clause.body.stmts {
                    visit_stmt(s, diagnostics, ctx);
                }
            }
            if let Some(finally_block) = &try_catch.finally {
                for s in &finally_block.stmts {
                    visit_stmt(s, diagnostics, ctx);
                }
            }
        }
        Stmt::Return(ret) => {
            if let Some(expr) = &ret.value {
                visit_expr(expr, false, diagnostics, ctx);
            }
        }
        Stmt::Throw(throw_stmt) => {
            visit_expr(&throw_stmt.value, false, diagnostics, ctx);
        }
        Stmt::LocalVar(local_var) => {
            for declarator in &local_var.declarators {
                if let Some(expr) = &declarator.initializer {
                    visit_expr(expr, false, diagnostics, ctx);
                }
            }
        }
        Stmt::LocalFunc(local_func) => {
            visit_function_body(&local_func.body, diagnostics, ctx);
        }
        Stmt::Expr(expr_stmt) => {
            visit_expr(&expr_stmt.expr, false, diagnostics, ctx);
        }
        _ => {}
    }
}

/// Visit an expression, flagging every conditional expression that is nested
/// inside another conditional expression. `cond_ancestor` is true when some
/// enclosing expression is already a conditional — mirroring dart_code_linter's
/// nesting-level assignment with the default `acceptable-level` of 1 (any
/// conditional at nesting level >= 2 is reported).
fn visit_expr(
    expr: &Expr,
    cond_ancestor: bool,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match expr {
        Expr::Conditional {
            span,
            condition,
            then_expr,
            else_expr,
        } => {
            if cond_ancestor {
                diagnostics.push(Diagnostic::new(
                    "avoid-nested-conditional-expressions",
                    Severity::Warning,
                    "Avoid nested conditional expressions",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
            // Everything under this conditional now has a conditional ancestor.
            visit_expr(condition, true, diagnostics, ctx);
            visit_expr(then_expr, true, diagnostics, ctx);
            visit_expr(else_expr, true, diagnostics, ctx);
        }
        Expr::Unary { operand, .. } => visit_expr(operand, cond_ancestor, diagnostics, ctx),
        Expr::PostfixIncDec { operand, .. } => visit_expr(operand, cond_ancestor, diagnostics, ctx),
        Expr::Binary { left, right, .. } => {
            visit_expr(left, cond_ancestor, diagnostics, ctx);
            visit_expr(right, cond_ancestor, diagnostics, ctx);
        }
        Expr::Assign { target, value, .. } => {
            visit_expr(target, cond_ancestor, diagnostics, ctx);
            visit_expr(value, cond_ancestor, diagnostics, ctx);
        }
        Expr::Is { expr, .. } => visit_expr(expr, cond_ancestor, diagnostics, ctx),
        Expr::As { expr, .. } => visit_expr(expr, cond_ancestor, diagnostics, ctx),
        Expr::Field { object, .. } => visit_expr(object, cond_ancestor, diagnostics, ctx),
        Expr::Index { object, index, .. } => {
            visit_expr(object, cond_ancestor, diagnostics, ctx);
            visit_expr(index, cond_ancestor, diagnostics, ctx);
        }
        Expr::Call { callee, args, .. } => {
            visit_expr(callee, cond_ancestor, diagnostics, ctx);
            for arg in &args.positional {
                visit_expr(arg, cond_ancestor, diagnostics, ctx);
            }
            for named_arg in &args.named {
                visit_expr(&named_arg.value, cond_ancestor, diagnostics, ctx);
            }
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            visit_expr(object, cond_ancestor, diagnostics, ctx);
            for section in sections {
                match &section.op {
                    CascadeOp::Index(idx, _) => visit_expr(idx, cond_ancestor, diagnostics, ctx),
                    CascadeOp::Call(_, _, args) => {
                        for a in &args.positional {
                            visit_expr(a, cond_ancestor, diagnostics, ctx);
                        }
                        for na in &args.named {
                            visit_expr(&na.value, cond_ancestor, diagnostics, ctx);
                        }
                    }
                    CascadeOp::Assign(tgt, _, val) => {
                        visit_expr(tgt, cond_ancestor, diagnostics, ctx);
                        visit_expr(val, cond_ancestor, diagnostics, ctx);
                    }
                    CascadeOp::Field(_, _) => {}
                }
            }
        }
        Expr::List { elements, .. } => {
            for elem in elements {
                visit_collection_element(elem, cond_ancestor, diagnostics, ctx);
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for entry in entries {
                visit_expr(&entry.key, cond_ancestor, diagnostics, ctx);
                visit_expr(&entry.value, cond_ancestor, diagnostics, ctx);
            }
            for e in map_element_exprs(elements) {
                visit_expr(e, cond_ancestor, diagnostics, ctx);
            }
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                visit_collection_element(elem, cond_ancestor, diagnostics, ctx);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                visit_expr(&field.value, cond_ancestor, diagnostics, ctx);
            }
        }
        Expr::New { args, .. } => {
            for arg in &args.positional {
                visit_expr(arg, cond_ancestor, diagnostics, ctx);
            }
            for named_arg in &args.named {
                visit_expr(&named_arg.value, cond_ancestor, diagnostics, ctx);
            }
        }
        Expr::Await { expr, .. } => visit_expr(expr, cond_ancestor, diagnostics, ctx),
        Expr::Throw { expr, .. } => visit_expr(expr, cond_ancestor, diagnostics, ctx),
        Expr::Switch { subject, arms, .. } => {
            visit_expr(subject, cond_ancestor, diagnostics, ctx);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    visit_expr(guard, cond_ancestor, diagnostics, ctx);
                }
                visit_expr(&arm.body, cond_ancestor, diagnostics, ctx);
            }
        }
        Expr::NullAssert { operand, .. } => visit_expr(operand, cond_ancestor, diagnostics, ctx),
        _ => {}
    }
}

fn visit_collection_element(
    elem: &CollectionElement,
    cond_ancestor: bool,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match elem {
        CollectionElement::Expr(e) => visit_expr(e, cond_ancestor, diagnostics, ctx),
        CollectionElement::NullAware { expr, .. } => {
            visit_expr(expr, cond_ancestor, diagnostics, ctx)
        }
        CollectionElement::Spread { expr, .. } => visit_expr(expr, cond_ancestor, diagnostics, ctx),
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            if let IfCondition::Expr(cond) = condition {
                visit_expr(cond, cond_ancestor, diagnostics, ctx);
            }
            visit_collection_element(then_elem, cond_ancestor, diagnostics, ctx);
            if let Some(ee) = else_elem {
                visit_collection_element(ee, cond_ancestor, diagnostics, ctx);
            }
        }
        CollectionElement::For {
            iterable, element, ..
        } => {
            visit_expr(iterable, cond_ancestor, diagnostics, ctx);
            visit_collection_element(element, cond_ancestor, diagnostics, ctx);
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
                            visit_expr(e, cond_ancestor, diagnostics, ctx);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => {
                    visit_expr(iterable, cond_ancestor, diagnostics, ctx);
                }
                Some(ForInit::PatternForIn { iterable, .. }) => {
                    visit_expr(iterable, cond_ancestor, diagnostics, ctx);
                }
                Some(ForInit::Exprs(es)) => {
                    for e in es {
                        visit_expr(e, cond_ancestor, diagnostics, ctx);
                    }
                }
                None => {}
            }
            if let Some(c) = condition {
                visit_expr(c, cond_ancestor, diagnostics, ctx);
            }
            for u in updates {
                visit_expr(u, cond_ancestor, diagnostics, ctx);
            }
            visit_collection_element(element, cond_ancestor, diagnostics, ctx);
        }
    }
}
