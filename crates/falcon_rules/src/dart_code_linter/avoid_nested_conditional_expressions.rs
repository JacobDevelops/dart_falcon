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

fn visit_expr(
    expr: &Expr,
    inside_nested: bool,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if !inside_nested
        && is_nested_conditional(expr)
        && let Expr::Conditional {
            span,
            condition,
            then_expr,
            else_expr,
        } = expr
    {
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
        visit_expr(condition, true, diagnostics, ctx);
        visit_expr(then_expr, true, diagnostics, ctx);
        visit_expr(else_expr, true, diagnostics, ctx);
        return;
    }

    match expr {
        Expr::Unary { operand, .. } => {
            visit_expr(operand, inside_nested, diagnostics, ctx);
        }
        Expr::PostfixIncDec { operand, .. } => {
            visit_expr(operand, inside_nested, diagnostics, ctx);
        }
        Expr::Binary { left, right, .. } => {
            visit_expr(left, inside_nested, diagnostics, ctx);
            visit_expr(right, inside_nested, diagnostics, ctx);
        }
        Expr::Assign { target, value, .. } => {
            visit_expr(target, inside_nested, diagnostics, ctx);
            visit_expr(value, inside_nested, diagnostics, ctx);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            visit_expr(condition, inside_nested, diagnostics, ctx);
            visit_expr(then_expr, inside_nested, diagnostics, ctx);
            visit_expr(else_expr, inside_nested, diagnostics, ctx);
        }
        Expr::Is { expr, .. } => {
            visit_expr(expr, inside_nested, diagnostics, ctx);
        }
        Expr::As { expr, .. } => {
            visit_expr(expr, inside_nested, diagnostics, ctx);
        }
        Expr::Field { object, .. } => {
            visit_expr(object, inside_nested, diagnostics, ctx);
        }
        Expr::Index { object, index, .. } => {
            visit_expr(object, inside_nested, diagnostics, ctx);
            visit_expr(index, inside_nested, diagnostics, ctx);
        }
        Expr::Call { callee, args, .. } => {
            visit_expr(callee, inside_nested, diagnostics, ctx);
            for arg in &args.positional {
                visit_expr(arg, inside_nested, diagnostics, ctx);
            }
            for named_arg in &args.named {
                visit_expr(&named_arg.value, inside_nested, diagnostics, ctx);
            }
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            visit_expr(object, inside_nested, diagnostics, ctx);
            for section in sections {
                match &section.op {
                    CascadeOp::Index(idx, _) => visit_expr(idx, inside_nested, diagnostics, ctx),
                    CascadeOp::Call(_, _, args) => {
                        for a in &args.positional {
                            visit_expr(a, inside_nested, diagnostics, ctx);
                        }
                        for na in &args.named {
                            visit_expr(&na.value, inside_nested, diagnostics, ctx);
                        }
                    }
                    CascadeOp::Assign(tgt, _, val) => {
                        visit_expr(tgt, inside_nested, diagnostics, ctx);
                        visit_expr(val, inside_nested, diagnostics, ctx);
                    }
                    CascadeOp::Field(_, _) => {}
                }
            }
        }
        Expr::List { elements, .. } => {
            for elem in elements {
                visit_collection_element(elem, inside_nested, diagnostics, ctx);
            }
        }
        Expr::Map { entries, .. } => {
            for entry in entries {
                visit_expr(&entry.key, inside_nested, diagnostics, ctx);
                visit_expr(&entry.value, inside_nested, diagnostics, ctx);
            }
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                visit_collection_element(elem, inside_nested, diagnostics, ctx);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                visit_expr(&field.value, inside_nested, diagnostics, ctx);
            }
        }
        Expr::New { args, .. } => {
            for arg in &args.positional {
                visit_expr(arg, inside_nested, diagnostics, ctx);
            }
            for named_arg in &args.named {
                visit_expr(&named_arg.value, inside_nested, diagnostics, ctx);
            }
        }
        Expr::Await { expr, .. } => {
            visit_expr(expr, inside_nested, diagnostics, ctx);
        }
        Expr::Throw { expr, .. } => {
            visit_expr(expr, inside_nested, diagnostics, ctx);
        }
        Expr::Switch { subject, arms, .. } => {
            visit_expr(subject, inside_nested, diagnostics, ctx);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    visit_expr(guard, inside_nested, diagnostics, ctx);
                }
                visit_expr(&arm.body, inside_nested, diagnostics, ctx);
            }
        }
        Expr::NullAssert { operand, .. } => {
            visit_expr(operand, inside_nested, diagnostics, ctx);
        }
        _ => {}
    }
}

fn visit_collection_element(
    elem: &CollectionElement,
    inside_nested: bool,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match elem {
        CollectionElement::Expr(e) => {
            visit_expr(e, inside_nested, diagnostics, ctx);
        }
        CollectionElement::Spread { expr, .. } => {
            visit_expr(expr, inside_nested, diagnostics, ctx);
        }
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            if let IfCondition::Expr(cond) = condition {
                visit_expr(cond, inside_nested, diagnostics, ctx);
            }
            visit_collection_element(then_elem, inside_nested, diagnostics, ctx);
            if let Some(ee) = else_elem {
                visit_collection_element(ee, inside_nested, diagnostics, ctx);
            }
        }
        CollectionElement::For {
            iterable, element, ..
        } => {
            visit_expr(iterable, inside_nested, diagnostics, ctx);
            visit_collection_element(element, inside_nested, diagnostics, ctx);
        }
    }
}

fn is_nested_conditional(expr: &Expr) -> bool {
    if let Expr::Conditional {
        condition,
        then_expr,
        else_expr,
        ..
    } = expr
    {
        contains_conditional(condition)
            || contains_conditional(then_expr)
            || contains_conditional(else_expr)
    } else {
        false
    }
}

fn contains_conditional(expr: &Expr) -> bool {
    match expr {
        Expr::Conditional { .. } => true,
        Expr::Unary { operand, .. } => contains_conditional(operand),
        Expr::PostfixIncDec { operand, .. } => contains_conditional(operand),
        Expr::Binary { left, right, .. } => {
            contains_conditional(left) || contains_conditional(right)
        }
        Expr::Assign { target, value, .. } => {
            contains_conditional(target) || contains_conditional(value)
        }
        Expr::Is { expr, .. } => contains_conditional(expr),
        Expr::As { expr, .. } => contains_conditional(expr),
        Expr::Field { object, .. } => contains_conditional(object),
        Expr::Index { object, index, .. } => {
            contains_conditional(object) || contains_conditional(index)
        }
        Expr::Call { callee, args, .. } => {
            if contains_conditional(callee) {
                return true;
            }
            for arg in &args.positional {
                if contains_conditional(arg) {
                    return true;
                }
            }
            for named_arg in &args.named {
                if contains_conditional(&named_arg.value) {
                    return true;
                }
            }
            false
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            if contains_conditional(object) {
                return true;
            }
            for section in sections {
                match &section.op {
                    CascadeOp::Index(idx, _) => {
                        if contains_conditional(idx) {
                            return true;
                        }
                    }
                    CascadeOp::Call(_, _, args) => {
                        if args.positional.iter().any(contains_conditional) {
                            return true;
                        }
                        if args.named.iter().any(|na| contains_conditional(&na.value)) {
                            return true;
                        }
                    }
                    CascadeOp::Assign(tgt, _, val) => {
                        if contains_conditional(tgt) || contains_conditional(val) {
                            return true;
                        }
                    }
                    CascadeOp::Field(_, _) => {}
                }
            }
            false
        }
        Expr::List { elements, .. } => {
            for elem in elements {
                if contains_conditional_in_elem(elem) {
                    return true;
                }
            }
            false
        }
        Expr::Map { entries, .. } => {
            for entry in entries {
                if contains_conditional(&entry.key) || contains_conditional(&entry.value) {
                    return true;
                }
            }
            false
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                if contains_conditional_in_elem(elem) {
                    return true;
                }
            }
            false
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                if contains_conditional(&field.value) {
                    return true;
                }
            }
            false
        }
        Expr::New { args, .. } => {
            for arg in &args.positional {
                if contains_conditional(arg) {
                    return true;
                }
            }
            for named_arg in &args.named {
                if contains_conditional(&named_arg.value) {
                    return true;
                }
            }
            false
        }
        Expr::Await { expr, .. } => contains_conditional(expr),
        Expr::Throw { expr, .. } => contains_conditional(expr),
        Expr::Switch { subject, arms, .. } => {
            if contains_conditional(subject) {
                return true;
            }
            for arm in arms {
                if let Some(guard) = &arm.guard
                    && contains_conditional(guard)
                {
                    return true;
                }
                if contains_conditional(&arm.body) {
                    return true;
                }
            }
            false
        }
        Expr::NullAssert { operand, .. } => contains_conditional(operand),
        _ => false,
    }
}

fn contains_conditional_in_elem(elem: &CollectionElement) -> bool {
    match elem {
        CollectionElement::Expr(e) => contains_conditional(e),
        CollectionElement::Spread { expr, .. } => contains_conditional(expr),
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            if let IfCondition::Expr(cond) = condition
                && contains_conditional(cond)
            {
                return true;
            }
            if contains_conditional_in_elem(then_elem) {
                return true;
            }
            if let Some(ee) = else_elem
                && contains_conditional_in_elem(ee)
            {
                return true;
            }
            false
        }
        CollectionElement::For {
            iterable, element, ..
        } => contains_conditional(iterable) || contains_conditional_in_elem(element),
    }
}
