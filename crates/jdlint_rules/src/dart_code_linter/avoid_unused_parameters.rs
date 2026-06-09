use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;
use std::collections::HashSet;

pub struct AvoidUnusedParameters;

impl Rule for AvoidUnusedParameters {
    fn name(&self) -> &'static str {
        "avoid-unused-parameters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func) => {
                    if let Some(body) = &func.body {
                        check_function_params(&func.params, body, &mut diags, ctx);
                    }
                }
                TopLevelDecl::Class(class) => {
                    for member in &class.members {
                        match member {
                            ClassMember::Method(method) => {
                                if let Some(body) = &method.body {
                                    if method.name.name != "noSuchMethod" {
                                        check_function_params(&method.params, body, &mut diags, ctx);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                TopLevelDecl::Extension(ext) => {
                    for member in &ext.members {
                        if let ClassMember::Method(method) = member {
                            if let Some(body) = &method.body {
                                check_function_params(&method.params, body, &mut diags, ctx);
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        fn check_function_params(
            params: &FormalParamList,
            body: &FunctionBody,
            diags: &mut Vec<Diagnostic>,
            ctx: &AnalyzeContext,
        ) {
            let mut param_names = HashSet::new();

            for param in &params.positional {
                if param.name.name != "_" && !param.is_field && !param.is_super {
                    param_names.insert((param.name.name.clone(), param.name.span.clone()));
                }
            }

            for param in &params.optional_positional {
                if param.name.name != "_" && !param.is_field && !param.is_super {
                    param_names.insert((param.name.name.clone(), param.name.span.clone()));
                }
            }

            for param in &params.named {
                if param.name.name != "_" && !param.is_field && !param.is_super {
                    param_names.insert((param.name.name.clone(), param.name.span.clone()));
                }
            }

            let referenced = collect_referenced_names(body);

            for (name, span) in param_names {
                if !referenced.contains(name.as_str()) {
                    diags.push(Diagnostic::new(
                        "avoid-unused-parameters",
                        Severity::Warning,
                        format!("Parameter '{}' is declared but not used", name),
                        ctx.file_path.to_string_lossy().into_owned(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
            }
        }

        fn collect_referenced_names(body: &FunctionBody) -> HashSet<String> {
            let mut names = HashSet::new();

            match body {
                FunctionBody::Block(block) => {
                    collect_from_stmts(&block.stmts, &mut names);
                }
                FunctionBody::Arrow(expr, _) => {
                    collect_from_expr(expr, &mut names);
                }
                FunctionBody::Native(_, _) => {}
            }

            names
        }

        fn collect_from_stmts(stmts: &[Stmt], names: &mut HashSet<String>) {
            for stmt in stmts {
                match stmt {
                    Stmt::Block(block) => {
                        collect_from_stmts(&block.stmts, names);
                    }
                    Stmt::If(if_stmt) => {
                        if let IfCondition::Expr(expr) = &if_stmt.condition {
                            collect_from_expr(expr, names);
                        }
                        collect_from_stmt(&if_stmt.then_branch, names);
                        if let Some(else_stmt) = &if_stmt.else_branch {
                            collect_from_stmt(else_stmt, names);
                        }
                    }
                    Stmt::For(for_stmt) => {
                        if let Some(ForInit::VarDecl(var)) = &for_stmt.init {
                            for decl in &var.declarators {
                                if let Some(init) = &decl.initializer {
                                    collect_from_expr(init, names);
                                }
                            }
                        } else if let Some(ForInit::ForIn { iterable, .. }) = &for_stmt.init {
                            collect_from_expr(iterable, names);
                        } else if let Some(ForInit::Exprs(exprs)) = &for_stmt.init {
                            for expr in exprs {
                                collect_from_expr(expr, names);
                            }
                        }
                        if let Some(cond) = &for_stmt.condition {
                            collect_from_expr(cond, names);
                        }
                        for upd in &for_stmt.update {
                            collect_from_expr(upd, names);
                        }
                        collect_from_stmt(&for_stmt.body, names);
                    }
                    Stmt::While(while_stmt) => {
                        collect_from_expr(&while_stmt.condition, names);
                        collect_from_stmt(&while_stmt.body, names);
                    }
                    Stmt::DoWhile(do_while_stmt) => {
                        collect_from_expr(&do_while_stmt.condition, names);
                        collect_from_stmt(&do_while_stmt.body, names);
                    }
                    Stmt::Switch(switch_stmt) => {
                        collect_from_expr(&switch_stmt.subject, names);
                    }
                    Stmt::TryCatch(try_catch) => {
                        collect_from_stmts(&try_catch.body.stmts, names);
                        for catch in &try_catch.catches {
                            collect_from_stmts(&catch.body.stmts, names);
                        }
                        if let Some(finally) = &try_catch.finally {
                            collect_from_stmts(&finally.stmts, names);
                        }
                    }
                    Stmt::Return(return_stmt) => {
                        if let Some(value) = &return_stmt.value {
                            collect_from_expr(value, names);
                        }
                    }
                    Stmt::Throw(throw_stmt) => {
                        collect_from_expr(&throw_stmt.value, names);
                    }
                    Stmt::LocalVar(local_var) => {
                        for decl in &local_var.declarators {
                            if let Some(init) = &decl.initializer {
                                collect_from_expr(init, names);
                            }
                        }
                    }
                    Stmt::Expr(expr_stmt) => {
                        collect_from_expr(&expr_stmt.expr, names);
                    }
                    Stmt::LocalFunc(local_func) => {
                        match &local_func.body {
                            FunctionBody::Block(block) => collect_from_stmts(&block.stmts, names),
                            FunctionBody::Arrow(expr, _) => collect_from_expr(expr, names),
                            FunctionBody::Native(_, _) => {}
                        }
                    }
                    _ => {}
                }
            }
        }

        fn collect_from_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
            match stmt {
                Stmt::Block(block) => {
                    collect_from_stmts(&block.stmts, names);
                }
                Stmt::If(if_stmt) => {
                    if let IfCondition::Expr(expr) = &if_stmt.condition {
                        collect_from_expr(expr, names);
                    }
                    collect_from_stmt(&if_stmt.then_branch, names);
                    if let Some(else_stmt) = &if_stmt.else_branch {
                        collect_from_stmt(else_stmt, names);
                    }
                }
                Stmt::For(for_stmt) => {
                    if let Some(ForInit::VarDecl(var)) = &for_stmt.init {
                        for decl in &var.declarators {
                            if let Some(init) = &decl.initializer {
                                collect_from_expr(init, names);
                            }
                        }
                    } else if let Some(ForInit::ForIn { iterable, .. }) = &for_stmt.init {
                        collect_from_expr(iterable, names);
                    } else if let Some(ForInit::Exprs(exprs)) = &for_stmt.init {
                        for expr in exprs {
                            collect_from_expr(expr, names);
                        }
                    }
                    if let Some(cond) = &for_stmt.condition {
                        collect_from_expr(cond, names);
                    }
                    for upd in &for_stmt.update {
                        collect_from_expr(upd, names);
                    }
                    collect_from_stmt(&for_stmt.body, names);
                }
                Stmt::While(while_stmt) => {
                    collect_from_expr(&while_stmt.condition, names);
                    collect_from_stmt(&while_stmt.body, names);
                }
                Stmt::DoWhile(do_while_stmt) => {
                    collect_from_expr(&do_while_stmt.condition, names);
                    collect_from_stmt(&do_while_stmt.body, names);
                }
                _ => {}
            }
        }

        fn collect_from_expr(expr: &Expr, names: &mut HashSet<String>) {
            match expr {
                Expr::Ident(ident) => {
                    names.insert(ident.name.clone());
                }
                Expr::StringLit(lit) => {
                    let chars: Vec<char> = lit.raw.chars().collect();
                    let mut i = 0;
                    while i < chars.len() {
                        if chars[i] == '$' && i + 1 < chars.len() {
                            if chars[i + 1].is_alphabetic() || chars[i + 1] == '_' {
                                let start = i + 1;
                                let mut end = start;
                                while end < chars.len() && (chars[end].is_alphanumeric() || chars[end] == '_') {
                                    end += 1;
                                }
                                names.insert(chars[start..end].iter().collect());
                                i = end;
                                continue;
                            }
                        }
                        i += 1;
                    }
                }
                Expr::Unary { operand, .. } => {
                    collect_from_expr(operand, names);
                }
                Expr::PostfixIncDec { operand, .. } => {
                    collect_from_expr(operand, names);
                }
                Expr::Binary { left, right, .. } => {
                    collect_from_expr(left, names);
                    collect_from_expr(right, names);
                }
                Expr::Assign { target, value, .. } => {
                    collect_from_expr(target, names);
                    collect_from_expr(value, names);
                }
                Expr::Conditional {
                    condition,
                    then_expr,
                    else_expr,
                    ..
                } => {
                    collect_from_expr(condition, names);
                    collect_from_expr(then_expr, names);
                    collect_from_expr(else_expr, names);
                }
                Expr::Is { expr, .. } => {
                    collect_from_expr(expr, names);
                }
                Expr::As { expr, .. } => {
                    collect_from_expr(expr, names);
                }
                Expr::Field { object, .. } => {
                    collect_from_expr(object, names);
                }
                Expr::Index { object, index, .. } => {
                    collect_from_expr(object, names);
                    collect_from_expr(index, names);
                }
                Expr::Call { callee, args, .. } => {
                    collect_from_expr(callee, names);
                    for arg in &args.positional {
                        collect_from_expr(arg, names);
                    }
                    for named_arg in &args.named {
                        collect_from_expr(&named_arg.value, names);
                    }
                }
                Expr::Cascade { object, .. } => {
                    collect_from_expr(object, names);
                }
                Expr::List { elements, .. } => {
                    for elem in elements {
                        collect_from_collection_elem(elem, names);
                    }
                }
                Expr::Map { entries, .. } => {
                    for entry in entries {
                        collect_from_expr(&entry.key, names);
                        collect_from_expr(&entry.value, names);
                    }
                }
                Expr::Set { elements, .. } => {
                    for elem in elements {
                        collect_from_collection_elem(elem, names);
                    }
                }
                Expr::Record { fields, .. } => {
                    for field in fields {
                        collect_from_expr(&field.value, names);
                    }
                }
                Expr::FuncExpr { body, .. } => {
                    if let FunctionBody::Arrow(expr, _) = body.as_ref() {
                        collect_from_expr(expr, names);
                    }
                }
                Expr::New { args, .. } => {
                    for arg in &args.positional {
                        collect_from_expr(arg, names);
                    }
                    for named_arg in &args.named {
                        collect_from_expr(&named_arg.value, names);
                    }
                }
                Expr::Await { expr, .. } => {
                    collect_from_expr(expr, names);
                }
                Expr::Throw { expr, .. } => {
                    collect_from_expr(expr, names);
                }
                Expr::Switch { subject, arms, .. } => {
                    collect_from_expr(subject, names);
                    for arm in arms {
                        if let Some(guard) = &arm.guard {
                            collect_from_expr(guard, names);
                        }
                        collect_from_expr(&arm.body, names);
                    }
                }
                Expr::NullAssert { operand, .. } => {
                    collect_from_expr(operand, names);
                }
                _ => {}
            }
        }

        fn collect_from_collection_elem(elem: &CollectionElement, names: &mut HashSet<String>) {
            match elem {
                CollectionElement::Expr(expr) => {
                    collect_from_expr(expr, names);
                }
                CollectionElement::Spread { expr, .. } => {
                    collect_from_expr(expr, names);
                }
                CollectionElement::If {
                    condition,
                    then_elem,
                    else_elem,
                    ..
                } => {
                    if let IfCondition::Expr(cond_expr) = condition {
                        collect_from_expr(cond_expr, names);
                    }
                    collect_from_collection_elem(then_elem, names);
                    if let Some(else_el) = else_elem {
                        collect_from_collection_elem(else_el, names);
                    }
                }
                CollectionElement::For { iterable, element, .. } => {
                    collect_from_expr(iterable, names);
                    collect_from_collection_elem(element, names);
                }
            }
        }

        diags
    }
}
