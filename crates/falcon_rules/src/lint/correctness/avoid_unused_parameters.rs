//! Flags function and method parameters that are never used
//! (`avoid-unused-parameters`). Originally ported from dart_code_linter and
//! unifies the former pyramid_lint twin `avoid_unused_parameters`. The
//! dart_code_linter behavior wins where the two conflicted (only all-underscore
//! names like `_`/`__` are treated as intentional markers, and `dynamic`-typed
//! parameters are still checked); the pyramid twin's wider declaration reach —
//! mixins, mixin classes, enums, and extension types — is folded in additively.

/// The `avoid-unused-parameters` rule.
pub use dcl::AvoidUnusedParameters;

mod dcl {
    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;
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
                    // Every member-bearing declaration kind is checked. Mixin,
                    // mixin class, enum, and extension type were folded in from
                    // the pyramid twin.
                    TopLevelDecl::Class(c) => check_members(&c.members, &mut diags, ctx),
                    TopLevelDecl::Mixin(m) => check_members(&m.members, &mut diags, ctx),
                    TopLevelDecl::MixinClass(mc) => check_members(&mc.members, &mut diags, ctx),
                    TopLevelDecl::Enum(e) => check_members(&e.members, &mut diags, ctx),
                    TopLevelDecl::Extension(e) => check_members(&e.members, &mut diags, ctx),
                    TopLevelDecl::ExtensionType(e) => check_members(&e.members, &mut diags, ctx),
                    _ => {}
                }
            }

            // Check every method member of a declaration, skipping `@override`
            // methods (parameter list dictated by the supertype) and
            // `noSuchMethod` (receives an `Invocation` it may ignore).
            fn check_members(
                members: &[ClassMember],
                diags: &mut Vec<Diagnostic>,
                ctx: &AnalyzeContext,
            ) {
                for member in members {
                    if let ClassMember::Method(method) = member
                        && let Some(body) = &method.body
                        && method.name.name != "noSuchMethod"
                        && !is_override(&method.annotations)
                    {
                        check_function_params(&method.params, body, diags, ctx);
                    }
                }
            }

            // A parameter whose name is only underscores (`_`, `__`, …) is a
            // conventional "intentionally unused" marker and is never flagged.
            fn is_ignorable_name(name: &str) -> bool {
                !name.is_empty() && name.bytes().all(|b| b == b'_')
            }

            // `@override` methods must keep the parameter list dictated by the
            // supertype, so an unused parameter there is not the author's to remove.
            fn is_override(annotations: &[Annotation]) -> bool {
                annotations
                    .iter()
                    .any(|a| a.name.last().is_some_and(|id| id.name == "override"))
            }

            fn check_function_params(
                params: &FormalParamList,
                body: &FunctionBody,
                diags: &mut Vec<Diagnostic>,
                ctx: &AnalyzeContext,
            ) {
                let mut param_names = HashSet::new();

                for param in params
                    .positional
                    .iter()
                    .chain(&params.optional_positional)
                    .chain(&params.named)
                {
                    if !is_ignorable_name(&param.name.name) && !param.is_field && !param.is_super {
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
                    collect_from_stmt(stmt, names);
                }
            }

            // Complete single-statement walker. Every statement kind that can hold an
            // expression or nested statement must recurse, otherwise a referenced
            // parameter is missed and the rule falsely reports it as unused (e.g. a
            // parameter used only inside a closure body, a switch case, or the
            // single-statement `then` branch of an `if`).
            fn collect_from_stmt(stmt: &Stmt, names: &mut HashSet<String>) {
                match stmt {
                    Stmt::Block(block) => collect_from_stmts(&block.stmts, names),
                    Stmt::If(if_stmt) => {
                        match &if_stmt.condition {
                            IfCondition::Expr(expr) => collect_from_expr(expr, names),
                            IfCondition::Case(expr, _, guard) => {
                                collect_from_expr(expr, names);
                                if let Some(g) = guard {
                                    collect_from_expr(g, names);
                                }
                            }
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
                        for case in &switch_stmt.cases {
                            collect_from_stmts(&case.body, names);
                        }
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
                    Stmt::Throw(throw_stmt) => collect_from_expr(&throw_stmt.value, names),
                    Stmt::LocalVar(local_var) => {
                        for decl in &local_var.declarators {
                            if let Some(init) = &decl.initializer {
                                collect_from_expr(init, names);
                            }
                        }
                    }
                    Stmt::Expr(expr_stmt) => collect_from_expr(&expr_stmt.expr, names),
                    Stmt::Assert(assert_stmt) => {
                        collect_from_expr(&assert_stmt.condition, names);
                        if let Some(msg) = &assert_stmt.message {
                            collect_from_expr(msg, names);
                        }
                    }
                    Stmt::Yield(yield_stmt) => collect_from_expr(&yield_stmt.value, names),
                    Stmt::LocalFunc(local_func) => match &local_func.body {
                        FunctionBody::Block(block) => collect_from_stmts(&block.stmts, names),
                        FunctionBody::Arrow(expr, _) => collect_from_expr(expr, names),
                        FunctionBody::Native(_, _) => {}
                    },
                    _ => {}
                }
            }

            fn collect_from_expr(expr: &Expr, names: &mut HashSet<String>) {
                match expr {
                    Expr::Ident(ident) => {
                        names.insert(ident.name.clone());
                    }
                    Expr::StringLit(lit) => {
                        // The AST does not decompose interpolations into sub-exprs, so
                        // scan the raw text. Handle both simple `$name` and complex
                        // `${ expr }` interpolations — for the latter, collect every
                        // identifier inside the braces (over-collecting is safe: it can
                        // only suppress a report, never invent one).
                        let chars: Vec<char> = lit.raw.chars().collect();
                        let is_ident_start = |c: char| c.is_alphabetic() || c == '_';
                        let is_ident_cont = |c: char| c.is_alphanumeric() || c == '_';
                        let mut i = 0;
                        while i < chars.len() {
                            if chars[i] == '$' && i + 1 < chars.len() && chars[i + 1] == '{' {
                                // Complex `${ ... }` interpolation: gather all identifiers
                                // until the matching closing brace.
                                let mut depth = 1;
                                let mut j = i + 2;
                                while j < chars.len() && depth > 0 {
                                    match chars[j] {
                                        '{' => {
                                            depth += 1;
                                            j += 1;
                                        }
                                        '}' => {
                                            depth -= 1;
                                            j += 1;
                                        }
                                        c if is_ident_start(c) => {
                                            let start = j;
                                            while j < chars.len() && is_ident_cont(chars[j]) {
                                                j += 1;
                                            }
                                            names.insert(chars[start..j].iter().collect());
                                        }
                                        _ => j += 1,
                                    }
                                }
                                i = j;
                                continue;
                            }
                            if chars[i] == '$'
                                && i + 1 < chars.len()
                                && is_ident_start(chars[i + 1])
                            {
                                let start = i + 1;
                                let mut end = start;
                                while end < chars.len() && is_ident_cont(chars[end]) {
                                    end += 1;
                                }
                                names.insert(chars[start..end].iter().collect());
                                i = end;
                                continue;
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
                    Expr::Cascade {
                        object, sections, ..
                    } => {
                        collect_from_expr(object, names);
                        for section in sections {
                            match &section.op {
                                CascadeOp::Field(_, _) => {}
                                CascadeOp::Index(index, _) => collect_from_expr(index, names),
                                CascadeOp::Call(_, _, args) => {
                                    for arg in &args.positional {
                                        collect_from_expr(arg, names);
                                    }
                                    for named_arg in &args.named {
                                        collect_from_expr(&named_arg.value, names);
                                    }
                                }
                                CascadeOp::Assign(target, _, value) => {
                                    collect_from_expr(target, names);
                                    collect_from_expr(value, names);
                                }
                            }
                        }
                    }
                    Expr::List { elements, .. } => {
                        for elem in elements {
                            collect_from_collection_elem(elem, names);
                        }
                    }
                    Expr::Map {
                        entries, elements, ..
                    } => {
                        for entry in entries {
                            collect_from_expr(&entry.key, names);
                            collect_from_expr(&entry.value, names);
                        }
                        // Comprehension maps (`{ for (..) k: v }`) put everything in
                        // `elements` and leave `entries` empty; walk them so a param
                        // used only in a map-comprehension iterable counts as used.
                        for element in elements {
                            collect_from_map_element(element, names);
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
                    Expr::FuncExpr { body, .. } => match body.as_ref() {
                        FunctionBody::Block(block) => collect_from_stmts(&block.stmts, names),
                        FunctionBody::Arrow(expr, _) => collect_from_expr(expr, names),
                        FunctionBody::Native(_, _) => {}
                    },
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
                    CollectionElement::NullAware { expr, .. } => {
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
                    CollectionElement::For {
                        iterable, element, ..
                    } => {
                        collect_from_expr(iterable, names);
                        collect_from_collection_elem(element, names);
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
                                        collect_from_expr(e, names);
                                    }
                                }
                            }
                            Some(ForInit::ForIn { iterable, .. }) => {
                                collect_from_expr(iterable, names);
                            }
                            Some(ForInit::PatternForIn { iterable, .. }) => {
                                collect_from_expr(iterable, names);
                            }
                            Some(ForInit::Exprs(es)) => {
                                for e in es {
                                    collect_from_expr(e, names);
                                }
                            }
                            None => {}
                        }
                        if let Some(c) = condition {
                            collect_from_expr(c, names);
                        }
                        for u in updates {
                            collect_from_expr(u, names);
                        }
                        collect_from_collection_elem(element, names);
                    }
                }
            }

            fn collect_from_map_element(elem: &MapElement, names: &mut HashSet<String>) {
                match elem {
                    MapElement::Entry(entry) => {
                        collect_from_expr(&entry.key, names);
                        collect_from_expr(&entry.value, names);
                    }
                    MapElement::Spread { expr, .. } => {
                        collect_from_expr(expr, names);
                    }
                    MapElement::If {
                        condition,
                        then_entry,
                        else_entry,
                        ..
                    } => {
                        if let IfCondition::Expr(cond_expr) = condition {
                            collect_from_expr(cond_expr, names);
                        }
                        collect_from_map_element(then_entry, names);
                        if let Some(else_el) = else_entry {
                            collect_from_map_element(else_el, names);
                        }
                    }
                    MapElement::For {
                        iterable, entry, ..
                    } => {
                        collect_from_expr(iterable, names);
                        collect_from_map_element(entry, names);
                    }
                    MapElement::CFor {
                        init,
                        condition,
                        updates,
                        entry,
                        ..
                    } => {
                        match init {
                            Some(ForInit::VarDecl(d)) => {
                                for decl in &d.declarators {
                                    if let Some(e) = &decl.initializer {
                                        collect_from_expr(e, names);
                                    }
                                }
                            }
                            Some(ForInit::ForIn { iterable, .. }) => {
                                collect_from_expr(iterable, names);
                            }
                            Some(ForInit::PatternForIn { iterable, .. }) => {
                                collect_from_expr(iterable, names);
                            }
                            Some(ForInit::Exprs(es)) => {
                                for e in es {
                                    collect_from_expr(e, names);
                                }
                            }
                            None => {}
                        }
                        if let Some(c) = condition {
                            collect_from_expr(c, names);
                        }
                        for u in updates {
                            collect_from_expr(u, names);
                        }
                        collect_from_map_element(entry, names);
                    }
                }
            }

            diags
        }
    }
}
