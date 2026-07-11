//! Flags async callbacks passed where a synchronous function is expected. Ported from dart_code_linter's `avoid-passing-async-when-sync-expected`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use std::collections::HashMap;

pub struct AvoidPassingAsyncWhenSyncExpected;

impl AvoidPassingAsyncWhenSyncExpected {
    fn is_non_future_function_type(dart_type: &DartType) -> bool {
        match dart_type {
            DartType::Function(func_type) => {
                if let Some(return_type) = &func_type.return_type {
                    !Self::is_future_type(return_type)
                } else {
                    true
                }
            }
            DartType::Named(named) => named
                .segments
                .last()
                .is_some_and(|seg| seg.name == "Function"),
            _ => false,
        }
    }

    fn is_future_type(dart_type: &DartType) -> bool {
        match dart_type {
            DartType::Named(named) => named
                .segments
                .last()
                .is_some_and(|seg| seg.name == "Future"),
            _ => false,
        }
    }

    fn collect_function_params(program: &Program) -> HashMap<String, Vec<Option<DartType>>> {
        let mut map = HashMap::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    let param_types = func
                        .params
                        .positional
                        .iter()
                        .map(|p| p.param_type.clone())
                        .chain(
                            func.params
                                .optional_positional
                                .iter()
                                .map(|p| p.param_type.clone()),
                        )
                        .collect();
                    map.insert(func.name.name.clone(), param_types);
                }
                TopLevelDecl::Class(class) => {
                    for member in &class.members {
                        if let ClassMember::Method(method) = member {
                            let param_types: Vec<Option<DartType>> = method
                                .params
                                .positional
                                .iter()
                                .map(|p| p.param_type.clone())
                                .chain(
                                    method
                                        .params
                                        .optional_positional
                                        .iter()
                                        .map(|p| p.param_type.clone()),
                                )
                                .collect();
                            let method_key = format!("{}#{}", class.name.name, method.name.name);
                            map.insert(method_key, param_types.clone());
                            map.entry(method.name.name.clone()).or_insert(param_types);
                        }
                    }
                }
                _ => {}
            }
        }

        map
    }

    fn visit_exprs(expr: &Expr, f: &mut impl FnMut(&Expr)) {
        f(expr);

        match expr {
            Expr::Unary { operand, .. } => Self::visit_exprs(operand, f),
            Expr::PostfixIncDec { operand, .. } => Self::visit_exprs(operand, f),
            Expr::Binary { left, right, .. } => {
                Self::visit_exprs(left, f);
                Self::visit_exprs(right, f);
            }
            Expr::Assign { target, value, .. } => {
                Self::visit_exprs(target, f);
                Self::visit_exprs(value, f);
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                Self::visit_exprs(condition, f);
                Self::visit_exprs(then_expr, f);
                Self::visit_exprs(else_expr, f);
            }
            Expr::Is { expr, .. } => Self::visit_exprs(expr, f),
            Expr::As { expr, .. } => Self::visit_exprs(expr, f),
            Expr::Field { object, .. } => Self::visit_exprs(object, f),
            Expr::Index { object, index, .. } => {
                Self::visit_exprs(object, f);
                Self::visit_exprs(index, f);
            }
            Expr::Call { callee, args, .. } => {
                Self::visit_exprs(callee, f);
                for arg in &args.positional {
                    Self::visit_exprs(arg, f);
                }
                for named_arg in &args.named {
                    Self::visit_exprs(&named_arg.value, f);
                }
            }
            Expr::Cascade {
                object, sections, ..
            } => {
                Self::visit_exprs(object, f);
                for section in sections {
                    match &section.op {
                        CascadeOp::Call(_, _, args) => {
                            for arg in &args.positional {
                                Self::visit_exprs(arg, f);
                            }
                            for named_arg in &args.named {
                                Self::visit_exprs(&named_arg.value, f);
                            }
                        }
                        CascadeOp::Index(index, _) => {
                            Self::visit_exprs(index, f);
                        }
                        CascadeOp::Assign(_, _, value) => {
                            Self::visit_exprs(value, f);
                        }
                        _ => {}
                    }
                }
            }
            Expr::List { elements, .. } => {
                for elem in elements {
                    Self::visit_collection_element(elem, f);
                }
            }
            Expr::Map {
                entries, elements, ..
            } => {
                for entry in entries {
                    Self::visit_exprs(&entry.key, f);
                    Self::visit_exprs(&entry.value, f);
                }
                for e in map_element_exprs(elements) {
                    Self::visit_exprs(e, f);
                }
            }
            Expr::Set { elements, .. } => {
                for elem in elements {
                    Self::visit_collection_element(elem, f);
                }
            }
            Expr::Record { fields, .. } => {
                for field in fields {
                    Self::visit_exprs(&field.value, f);
                }
            }
            Expr::New { args, .. } => {
                for arg in &args.positional {
                    Self::visit_exprs(arg, f);
                }
                for named_arg in &args.named {
                    Self::visit_exprs(&named_arg.value, f);
                }
            }
            Expr::Await { expr, .. } => Self::visit_exprs(expr, f),
            Expr::Throw { expr, .. } => Self::visit_exprs(expr, f),
            Expr::Switch { subject, arms, .. } => {
                Self::visit_exprs(subject, f);
                for arm in arms {
                    Self::visit_exprs(&arm.body, f);
                }
            }
            Expr::NullAssert { operand, .. } => Self::visit_exprs(operand, f),
            _ => {}
        }
    }

    fn visit_collection_element(elem: &CollectionElement, f: &mut impl FnMut(&Expr)) {
        match elem {
            CollectionElement::Expr(expr) => Self::visit_exprs(expr, f),
            CollectionElement::NullAware { expr, .. } => Self::visit_exprs(expr, f),
            CollectionElement::Spread { expr, .. } => Self::visit_exprs(expr, f),
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                if let IfCondition::Expr(cond_expr) = condition {
                    Self::visit_exprs(cond_expr, f);
                }
                Self::visit_collection_element(then_elem, f);
                if let Some(else_e) = else_elem {
                    Self::visit_collection_element(else_e, f);
                }
            }
            CollectionElement::For {
                iterable, element, ..
            } => {
                Self::visit_exprs(iterable, f);
                Self::visit_collection_element(element, f);
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
                                Self::visit_exprs(e, f);
                            }
                        }
                    }
                    Some(ForInit::ForIn { iterable, .. }) => {
                        Self::visit_exprs(iterable, f);
                    }
                    Some(ForInit::PatternForIn { iterable, .. }) => {
                        Self::visit_exprs(iterable, f);
                    }
                    Some(ForInit::Exprs(es)) => {
                        for e in es {
                            Self::visit_exprs(e, f);
                        }
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    Self::visit_exprs(c, f);
                }
                for u in updates {
                    Self::visit_exprs(u, f);
                }
                Self::visit_collection_element(element, f);
            }
        }
    }

    fn visit_stmts(stmts: &[Stmt], f: &mut impl FnMut(&Stmt)) {
        for stmt in stmts {
            f(stmt);
            match stmt {
                Stmt::Block(block) => Self::visit_stmts(&block.stmts, f),
                Stmt::If(if_stmt) => {
                    if let IfCondition::Expr(_) = &if_stmt.condition {}
                    Self::visit_stmts(&[*if_stmt.then_branch.clone()], f);
                    if let Some(else_branch) = &if_stmt.else_branch {
                        Self::visit_stmts(&[*else_branch.clone()], f);
                    }
                }
                Stmt::For(for_stmt) => {
                    Self::visit_stmts(&[*for_stmt.body.clone()], f);
                }
                Stmt::While(while_stmt) => {
                    Self::visit_stmts(&[*while_stmt.body.clone()], f);
                }
                Stmt::DoWhile(do_while_stmt) => {
                    Self::visit_stmts(&[*do_while_stmt.body.clone()], f);
                }
                Stmt::Switch(switch_stmt) => {
                    for switch_case in &switch_stmt.cases {
                        Self::visit_stmts(&switch_case.body, f);
                    }
                }
                Stmt::TryCatch(try_catch) => {
                    Self::visit_stmts(&try_catch.body.stmts, f);
                    for catch_clause in &try_catch.catches {
                        Self::visit_stmts(&catch_clause.body.stmts, f);
                    }
                    if let Some(finally) = &try_catch.finally {
                        Self::visit_stmts(&finally.stmts, f);
                    }
                }
                Stmt::Expr(expr_stmt) => {
                    Self::visit_exprs(&expr_stmt.expr, &mut |_| {});
                }
                _ => {}
            }
        }
    }

    fn get_callee_name(expr: &Expr) -> Option<String> {
        match expr {
            Expr::Ident(id) => Some(id.name.clone()),
            Expr::Field { field, .. } => Some(field.name.clone()),
            _ => None,
        }
    }

    fn check_call(
        callee_expr: &Expr,
        args: &ArgList,
        param_map: &HashMap<String, Vec<Option<DartType>>>,
    ) -> Vec<Span> {
        let mut violations = Vec::new();

        if let Some(callee_name) = Self::get_callee_name(callee_expr)
            && let Some(param_types) = param_map.get(&callee_name)
        {
            for (idx, arg) in args.positional.iter().enumerate() {
                if let Expr::FuncExpr {
                    is_async: true,
                    span: func_span,
                    ..
                } = arg
                    && idx < param_types.len()
                    && let Some(Some(param_type)) = param_types.get(idx)
                    && Self::is_non_future_function_type(param_type)
                {
                    violations.push(func_span.clone());
                }
            }
        }

        violations
    }
}

impl Rule for AvoidPassingAsyncWhenSyncExpected {
    fn name(&self) -> &'static str {
        "avoid-passing-async-when-sync-expected"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        let param_map = Self::collect_function_params(program);
        let file_path = ctx.file_path.to_string_lossy().into_owned();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    if let Some(FunctionBody::Block(block)) = &func.body {
                        Self::visit_stmts(&block.stmts, &mut |stmt| {
                            if let Stmt::Expr(expr_stmt) = stmt {
                                Self::visit_exprs(&expr_stmt.expr, &mut |expr| {
                                    if let Expr::Call { callee, args, .. } = expr {
                                        let violations = Self::check_call(callee, args, &param_map);
                                        for span in violations {
                                            diagnostics.push(Diagnostic::new(
                                                "avoid-passing-async-when-sync-expected",
                                                Severity::Warning,
                                                "Avoid passing an async function where a synchronous callback is expected",
                                                file_path.clone(),
                                                DiagSpan {
                                                    start: span.start,
                                                    end: span.end,
                                                },
                                            ));
                                        }
                                    }
                                });
                            }
                        });
                    }
                }
                TopLevelDecl::Class(class) => {
                    for member in &class.members {
                        if let ClassMember::Method(method) = member
                            && let Some(FunctionBody::Block(block)) = &method.body
                        {
                            Self::visit_stmts(&block.stmts, &mut |stmt| {
                                if let Stmt::Expr(expr_stmt) = stmt {
                                    Self::visit_exprs(&expr_stmt.expr, &mut |expr| {
                                        if let Expr::Call { callee, args, .. } = expr {
                                            let violations =
                                                Self::check_call(callee, args, &param_map);
                                            for span in violations {
                                                diagnostics.push(Diagnostic::new(
                                                        "avoid-passing-async-when-sync-expected",
                                                        Severity::Warning,
                                                        "Avoid passing an async function where a synchronous callback is expected",
                                                        file_path.clone(),
                                                        DiagSpan {
                                                            start: span.start,
                                                            end: span.end,
                                                        },
                                                    ));
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        diagnostics
    }
}
