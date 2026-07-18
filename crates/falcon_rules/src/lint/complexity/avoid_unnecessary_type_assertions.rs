//! Flags `is`/`is!` checks whose result is statically known. Ported from dart_code_linter's `avoid-unnecessary-type-assertions`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use std::collections::HashMap;

pub struct AvoidUnnecessaryTypeAssertions;

impl Rule for AvoidUnnecessaryTypeAssertions {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-type-assertions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    if let Some(body) = &func.body {
                        analyze_function_body(body, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Class(class_decl) => {
                    let field_scope = collect_class_fields(class_decl);
                    for member in &class_decl.members {
                        match member {
                            ClassMember::Method(method) => {
                                if let Some(body) = &method.body {
                                    analyze_function_body_with_scope(
                                        body,
                                        &field_scope,
                                        &mut diags,
                                        ctx,
                                    );
                                }
                            }
                            ClassMember::Getter(getter) => {
                                if let Some(body) = &getter.body {
                                    analyze_function_body_with_scope(
                                        body,
                                        &field_scope,
                                        &mut diags,
                                        ctx,
                                    );
                                }
                            }
                            ClassMember::Constructor(ctor) => {
                                if let Some(body) = &ctor.body {
                                    analyze_function_body_with_scope(
                                        body,
                                        &field_scope,
                                        &mut diags,
                                        ctx,
                                    );
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        diags
    }
}

fn collect_class_fields(class_decl: &ClassDecl) -> HashMap<String, DartType> {
    let mut map = HashMap::new();
    for member in &class_decl.members {
        if let ClassMember::Field(field) = member
            && let Some(field_type) = &field.field_type
            && let DartType::Named(named) = field_type
            && !named.is_nullable
        {
            for declarator in &field.declarators {
                map.insert(declarator.name.name.clone(), field_type.clone());
            }
        }
    }
    map
}

fn analyze_function_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    analyze_function_body_with_scope(body, &HashMap::new(), diags, ctx);
}

fn analyze_function_body_with_scope(
    body: &FunctionBody,
    initial_scope: &HashMap<String, DartType>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match body {
        FunctionBody::Block(block) => {
            let mut scope_map = initial_scope.clone();
            for stmt in &block.stmts {
                analyze_statement(stmt, &mut scope_map, diags, ctx);
            }
        }
        FunctionBody::Arrow(expr, _) => {
            analyze_expression(expr, initial_scope, diags, ctx);
        }
        FunctionBody::Native(_, _) => {}
    }
}

fn analyze_block(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    let mut scope_map: HashMap<String, DartType> = HashMap::new();
    for stmt in &block.stmts {
        analyze_statement(stmt, &mut scope_map, diags, ctx);
    }
}

fn type_args_match(declared_args: &[DartType], cast_args: &[DartType]) -> bool {
    if cast_args.is_empty() {
        return true;
    }
    declared_args.len() == cast_args.len()
        && declared_args.iter().zip(cast_args.iter()).all(|(x, y)| {
            if let (DartType::Named(xn), DartType::Named(yn)) = (x, y) {
                xn.segments.first().map(|s| s.name.as_str())
                    == yn.segments.first().map(|s| s.name.as_str())
            } else {
                false
            }
        })
}

fn analyze_statement(
    stmt: &Stmt,
    scope_map: &mut HashMap<String, DartType>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match stmt {
        Stmt::LocalVar(local_var) => {
            if let Some(var_type) = &local_var.var_type
                && let DartType::Named(named) = var_type
                && !named.is_nullable
            {
                for declarator in &local_var.declarators {
                    scope_map.insert(declarator.name.name.clone(), var_type.clone());
                }
            }
            for declarator in &local_var.declarators {
                if let Some(init) = &declarator.initializer {
                    analyze_expression(init, scope_map, diags, ctx);
                }
            }
        }
        Stmt::Block(block) => {
            analyze_block(block, diags, ctx);
        }
        Stmt::If(if_stmt) => {
            match &if_stmt.condition {
                IfCondition::Expr(expr) => {
                    analyze_expression(expr, scope_map, diags, ctx);
                }
                IfCondition::Case(expr, _, guard) => {
                    analyze_expression(expr, scope_map, diags, ctx);
                    if let Some(g) = guard {
                        analyze_expression(g, scope_map, diags, ctx);
                    }
                }
            }
            analyze_statement(&if_stmt.then_branch, scope_map, diags, ctx);
            if let Some(else_branch) = &if_stmt.else_branch {
                analyze_statement(else_branch, scope_map, diags, ctx);
            }
        }
        Stmt::For(for_stmt) => {
            if let Some(expr) = &for_stmt.condition {
                analyze_expression(expr, scope_map, diags, ctx);
            }
            for update in &for_stmt.update {
                analyze_expression(update, scope_map, diags, ctx);
            }
            analyze_statement(&for_stmt.body, scope_map, diags, ctx);
        }
        Stmt::While(while_stmt) => {
            analyze_expression(&while_stmt.condition, scope_map, diags, ctx);
            analyze_statement(&while_stmt.body, scope_map, diags, ctx);
        }
        Stmt::DoWhile(do_while_stmt) => {
            analyze_expression(&do_while_stmt.condition, scope_map, diags, ctx);
            analyze_statement(&do_while_stmt.body, scope_map, diags, ctx);
        }
        Stmt::Switch(switch_stmt) => {
            analyze_expression(&switch_stmt.subject, scope_map, diags, ctx);
            for case in &switch_stmt.cases {
                for stmt_in_case in &case.body {
                    analyze_statement(stmt_in_case, scope_map, diags, ctx);
                }
            }
        }
        Stmt::TryCatch(try_catch) => {
            analyze_block(&try_catch.body, diags, ctx);
            for catch_clause in &try_catch.catches {
                analyze_block(&catch_clause.body, diags, ctx);
            }
            if let Some(finally_block) = &try_catch.finally {
                analyze_block(finally_block, diags, ctx);
            }
        }
        Stmt::Return(return_stmt) => {
            if let Some(expr) = &return_stmt.value {
                analyze_expression(expr, scope_map, diags, ctx);
            }
        }
        Stmt::Throw(throw_stmt) => {
            analyze_expression(&throw_stmt.value, scope_map, diags, ctx);
        }
        Stmt::Expr(expr_stmt) => {
            analyze_expression(&expr_stmt.expr, scope_map, diags, ctx);
        }
        Stmt::PatternDecl(pat_decl) => {
            analyze_expression(&pat_decl.init, scope_map, diags, ctx);
        }
        Stmt::PatternAssign(pat_assign) => {
            analyze_expression(&pat_assign.value, scope_map, diags, ctx);
        }
        Stmt::Labeled(labeled) => {
            analyze_statement(&labeled.stmt, scope_map, diags, ctx);
        }
        Stmt::Assert(assert_stmt) => {
            analyze_expression(&assert_stmt.condition, scope_map, diags, ctx);
            if let Some(message) = &assert_stmt.message {
                analyze_expression(message, scope_map, diags, ctx);
            }
        }
        Stmt::Yield(yield_stmt) => {
            analyze_expression(&yield_stmt.value, scope_map, diags, ctx);
        }
        Stmt::LocalFunc(local_func) => {
            analyze_function_body(&local_func.body, diags, ctx);
        }
        Stmt::Break(_) | Stmt::Continue(_) | Stmt::Error(_) => {}
    }
}

fn analyze_expression(
    expr: &Expr,
    scope_map: &HashMap<String, DartType>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match expr {
        Expr::Is {
            expr: operand,
            dart_type,
            negated: false,
            span,
        } => {
            if let Expr::Ident(ident) = &**operand
                && let DartType::Named(cast_named) = dart_type
                && let Some(declared) = scope_map.get(&ident.name)
                && let DartType::Named(declared_named) = declared
                && !declared_named.is_nullable
            {
                let decl_name = declared_named.segments.first().map(|s| s.name.as_str());
                let cast_name = cast_named.segments.first().map(|s| s.name.as_str());
                if decl_name == cast_name
                    && type_args_match(&declared_named.type_args, &cast_named.type_args)
                {
                    diags.push(Diagnostic::new(
                        "avoid-unnecessary-type-assertions",
                        Severity::Warning,
                        "Unnecessary type assertion — variable is already known to be this type",
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
            }

            analyze_expression(operand, scope_map, diags, ctx);
        }
        Expr::Binary { left, right, .. } => {
            analyze_expression(left, scope_map, diags, ctx);
            analyze_expression(right, scope_map, diags, ctx);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            analyze_expression(condition, scope_map, diags, ctx);
            analyze_expression(then_expr, scope_map, diags, ctx);
            analyze_expression(else_expr, scope_map, diags, ctx);
        }
        Expr::Unary { operand, .. } => {
            analyze_expression(operand, scope_map, diags, ctx);
        }
        Expr::PostfixIncDec { operand, .. } => {
            analyze_expression(operand, scope_map, diags, ctx);
        }
        Expr::Assign { target, value, .. } => {
            analyze_expression(target, scope_map, diags, ctx);
            analyze_expression(value, scope_map, diags, ctx);
        }
        Expr::Field { object, .. } => {
            analyze_expression(object, scope_map, diags, ctx);
        }
        Expr::Index { object, index, .. } => {
            analyze_expression(object, scope_map, diags, ctx);
            analyze_expression(index, scope_map, diags, ctx);
        }
        Expr::Call { callee, args, .. } => {
            analyze_expression(callee, scope_map, diags, ctx);
            for arg in &args.positional {
                analyze_expression(arg, scope_map, diags, ctx);
            }
            for named_arg in &args.named {
                analyze_expression(&named_arg.value, scope_map, diags, ctx);
            }
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            analyze_expression(object, scope_map, diags, ctx);
            for section in sections {
                for op in &section.ops {
                    match op {
                        CascadeOp::Index(index, _) => {
                            analyze_expression(index, scope_map, diags, ctx);
                        }
                        CascadeOp::Call(_, _, args) => {
                            for arg in &args.positional {
                                analyze_expression(arg, scope_map, diags, ctx);
                            }
                            for named_arg in &args.named {
                                analyze_expression(&named_arg.value, scope_map, diags, ctx);
                            }
                        }
                        CascadeOp::Assign(_, _, value) => {
                            analyze_expression(value, scope_map, diags, ctx);
                        }
                        _ => {}
                    }
                }
            }
        }
        Expr::List { elements, .. } => {
            for elem in elements {
                analyze_collection_element(elem, scope_map, diags, ctx);
            }
        }
        Expr::Map {
            entries, elements, ..
        } => {
            for entry in entries {
                analyze_expression(&entry.key, scope_map, diags, ctx);
                analyze_expression(&entry.value, scope_map, diags, ctx);
            }
            for e in map_element_exprs(elements) {
                analyze_expression(e, scope_map, diags, ctx);
            }
        }
        Expr::Set { elements, .. } => {
            for elem in elements {
                analyze_collection_element(elem, scope_map, diags, ctx);
            }
        }
        Expr::As { expr, .. } => {
            analyze_expression(expr, scope_map, diags, ctx);
        }
        Expr::Await { expr, .. } => {
            analyze_expression(expr, scope_map, diags, ctx);
        }
        Expr::Throw { expr, .. } => {
            analyze_expression(expr, scope_map, diags, ctx);
        }
        Expr::NullAssert { operand, .. } => {
            analyze_expression(operand, scope_map, diags, ctx);
        }
        Expr::Switch { subject, arms, .. } => {
            analyze_expression(subject, scope_map, diags, ctx);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    analyze_expression(guard, scope_map, diags, ctx);
                }
                analyze_expression(&arm.body, scope_map, diags, ctx);
            }
        }
        _ => {}
    }
}

fn analyze_collection_element(
    elem: &CollectionElement,
    scope_map: &HashMap<String, DartType>,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match elem {
        CollectionElement::Expr(expr) => {
            analyze_expression(expr, scope_map, diags, ctx);
        }
        CollectionElement::Spread { expr, .. } => {
            analyze_expression(expr, scope_map, diags, ctx);
        }
        CollectionElement::NullAware { expr, .. } => {
            analyze_expression(expr, scope_map, diags, ctx);
        }
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            if let IfCondition::Expr(cond_expr) = condition {
                analyze_expression(cond_expr, scope_map, diags, ctx);
            }
            analyze_collection_element(then_elem, scope_map, diags, ctx);
            if let Some(else_e) = else_elem {
                analyze_collection_element(else_e, scope_map, diags, ctx);
            }
        }
        CollectionElement::For {
            iterable, element, ..
        } => {
            analyze_expression(iterable, scope_map, diags, ctx);
            analyze_collection_element(element, scope_map, diags, ctx);
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
                            analyze_expression(e, scope_map, diags, ctx);
                        }
                    }
                }
                Some(ForInit::ForIn { iterable, .. }) => {
                    analyze_expression(iterable, scope_map, diags, ctx);
                }
                Some(ForInit::PatternForIn { iterable, .. }) => {
                    analyze_expression(iterable, scope_map, diags, ctx);
                }
                Some(ForInit::Exprs(es)) => {
                    for e in es {
                        analyze_expression(e, scope_map, diags, ctx);
                    }
                }
                None => {}
            }
            if let Some(c) = condition {
                analyze_expression(c, scope_map, diags, ctx);
            }
            for u in updates {
                analyze_expression(u, scope_map, diags, ctx);
            }
            analyze_collection_element(element, scope_map, diags, ctx);
        }
    }
}
