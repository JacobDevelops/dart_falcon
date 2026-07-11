//! Flags function and method parameters that are never used. Co-locates two independent implementations:
//! `avoid-unused-parameters` (dart_code_linter) and `avoid_unused_parameters` (pyramid_lint). These are
//! separate verbatim ports and share no logic.

/// The `avoid-unused-parameters` rule, ported from dart_code_linter.
pub use dcl::AvoidUnusedParameters;
/// The `avoid_unused_parameters` rule, ported from pyramid_lint.
pub use pyramid::AvoidUnusedParameters as AvoidUnusedParametersPyramid;

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
                    TopLevelDecl::Class(class) => {
                        for member in &class.members {
                            if let ClassMember::Method(method) = member
                                && let Some(body) = &method.body
                                && method.name.name != "noSuchMethod"
                                && !is_override(&method.annotations)
                            {
                                check_function_params(&method.params, body, &mut diags, ctx);
                            }
                        }
                    }
                    TopLevelDecl::Extension(ext) => {
                        for member in &ext.members {
                            if let ClassMember::Method(method) = member
                                && let Some(body) = &method.body
                                && !is_override(&method.annotations)
                            {
                                check_function_params(&method.params, body, &mut diags, ctx);
                            }
                        }
                    }
                    _ => {}
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
                            IfCondition::Case(expr, _) => collect_from_expr(expr, names),
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

mod pyramid {
    //! pyramid_lint `avoid_unused_parameters`: flag function/method parameters that
    //! are never referenced in the body. Parameters intentionally unused should be
    //! named with a leading underscore (`_`). `dynamic`-typed parameters are exempt
    //! (commonly required to match a callback signature).

    use std::collections::HashSet;

    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;

    pub struct AvoidUnusedParameters;

    impl Rule for AvoidUnusedParameters {
        fn name(&self) -> &'static str {
            "avoid_unused_parameters"
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
                    check_function(&f.params, body, diags, ctx);
                }
            }
            TopLevelDecl::Class(c) => c.members.iter().for_each(|m| scan_member(m, diags, ctx)),
            TopLevelDecl::Mixin(m) => m.members.iter().for_each(|m| scan_member(m, diags, ctx)),
            TopLevelDecl::MixinClass(mc) => {
                mc.members.iter().for_each(|m| scan_member(m, diags, ctx))
            }
            TopLevelDecl::Enum(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
            TopLevelDecl::Extension(e) => e.members.iter().for_each(|m| scan_member(m, diags, ctx)),
            TopLevelDecl::ExtensionType(e) => {
                e.members.iter().for_each(|m| scan_member(m, diags, ctx))
            }
            _ => {}
        }
    }

    fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        if let ClassMember::Method(m) = member
        && let Some(body) = &m.body
        // `@override` methods must keep the parameter list dictated by the
        // supertype, and `noSuchMethod` receives an `Invocation` it may ignore —
        // an unused parameter in either is not the author's to remove.
        && m.name.name != "noSuchMethod"
        && !is_override(&m.annotations)
        {
            check_function(&m.params, body, diags, ctx);
        }
    }

    /// An `@override` annotation forces the parameter list, so unused parameters
    /// there are dictated by the supertype rather than the author's oversight.
    fn is_override(annotations: &[Annotation]) -> bool {
        annotations
            .iter()
            .any(|a| a.name.last().is_some_and(|id| id.name == "override"))
    }

    /// Check a function/method's parameters against its body, then descend into any
    /// nested local functions.
    fn check_function(
        params: &FormalParamList,
        body: &FunctionBody,
        diags: &mut Vec<Diagnostic>,
        ctx: &AnalyzeContext,
    ) {
        let mut used: HashSet<String> = HashSet::new();
        collect_used_body(body, &mut used);

        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            let name = &param.name.name;
            if name.starts_with('_') {
                continue;
            }
            // `this.x` / `super.x` initializing formals bind straight to a field or
            // the super-constructor — the parameter *is* its use, so never flag them.
            if param.is_field || param.is_super {
                continue;
            }
            if matches!(param.param_type, Some(DartType::Dynamic { .. })) {
                continue;
            }
            if !used.contains(name) {
                diags.push(Diagnostic::new(
                    "avoid_unused_parameters",
                    Severity::Warning,
                    "Parameter is never used. Rename it to `_` or remove it.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: param.name.span.start,
                        end: param.name.span.end,
                    },
                ));
            }
        }

        // Nested local functions have their own parameter scope.
        if let FunctionBody::Block(b) = body {
            for s in &b.stmts {
                scan_nested_fns(s, diags, ctx);
            }
        }
    }

    fn scan_nested_fns(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match stmt {
            Stmt::LocalFunc(lf) => check_function(&lf.params, &lf.body, diags, ctx),
            Stmt::Block(b) => b.stmts.iter().for_each(|s| scan_nested_fns(s, diags, ctx)),
            Stmt::If(i) => {
                scan_nested_fns(&i.then_branch, diags, ctx);
                if let Some(eb) = &i.else_branch {
                    scan_nested_fns(eb, diags, ctx);
                }
            }
            Stmt::While(w) => scan_nested_fns(&w.body, diags, ctx),
            Stmt::DoWhile(d) => scan_nested_fns(&d.body, diags, ctx),
            Stmt::For(f) => scan_nested_fns(&f.body, diags, ctx),
            Stmt::Switch(sw) => {
                for case in &sw.cases {
                    case.body
                        .iter()
                        .for_each(|s| scan_nested_fns(s, diags, ctx));
                }
            }
            Stmt::TryCatch(tc) => {
                tc.body
                    .stmts
                    .iter()
                    .for_each(|s| scan_nested_fns(s, diags, ctx));
                for catch in &tc.catches {
                    catch
                        .body
                        .stmts
                        .iter()
                        .for_each(|s| scan_nested_fns(s, diags, ctx));
                }
                if let Some(fin) = &tc.finally {
                    fin.stmts
                        .iter()
                        .for_each(|s| scan_nested_fns(s, diags, ctx));
                }
            }
            _ => {}
        }
    }

    // ── Usage collection ──────────────────────────────────────────────────────────

    fn collect_used_body(body: &FunctionBody, used: &mut HashSet<String>) {
        match body {
            FunctionBody::Block(b) => b.stmts.iter().for_each(|s| collect_used_stmt(s, used)),
            FunctionBody::Arrow(e, _) => collect_used_expr(e, used),
            FunctionBody::Native(_, _) => {}
        }
    }

    fn collect_used_stmt(stmt: &Stmt, used: &mut HashSet<String>) {
        match stmt {
            Stmt::Block(b) => b.stmts.iter().for_each(|s| collect_used_stmt(s, used)),
            Stmt::Expr(e) => collect_used_expr(&e.expr, used),
            Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    collect_used_expr(v, used);
                }
            }
            Stmt::Throw(t) => collect_used_expr(&t.value, used),
            Stmt::Yield(y) => collect_used_expr(&y.value, used),
            Stmt::LocalVar(lv) => {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        collect_used_expr(init, used);
                    }
                }
            }
            Stmt::If(i) => {
                if let IfCondition::Expr(c) = &i.condition {
                    collect_used_expr(c, used);
                }
                collect_used_stmt(&i.then_branch, used);
                if let Some(eb) = &i.else_branch {
                    collect_used_stmt(eb, used);
                }
            }
            Stmt::While(w) => {
                collect_used_expr(&w.condition, used);
                collect_used_stmt(&w.body, used);
            }
            Stmt::DoWhile(d) => {
                collect_used_stmt(&d.body, used);
                collect_used_expr(&d.condition, used);
            }
            Stmt::For(f) => {
                if let Some(cond) = &f.condition {
                    collect_used_expr(cond, used);
                }
                match &f.init {
                    Some(ForInit::VarDecl(lv)) => {
                        for d in &lv.declarators {
                            if let Some(init) = &d.initializer {
                                collect_used_expr(init, used);
                            }
                        }
                    }
                    Some(ForInit::ForIn { iterable, .. }) => collect_used_expr(iterable, used),
                    Some(ForInit::PatternForIn { iterable, .. }) => {
                        collect_used_expr(iterable, used)
                    }
                    Some(ForInit::Exprs(es)) => es.iter().for_each(|e| collect_used_expr(e, used)),
                    None => {}
                }
                f.update.iter().for_each(|e| collect_used_expr(e, used));
                collect_used_stmt(&f.body, used);
            }
            Stmt::Switch(sw) => {
                collect_used_expr(&sw.subject, used);
                for case in &sw.cases {
                    case.body.iter().for_each(|s| collect_used_stmt(s, used));
                }
            }
            Stmt::TryCatch(tc) => {
                tc.body
                    .stmts
                    .iter()
                    .for_each(|s| collect_used_stmt(s, used));
                for catch in &tc.catches {
                    catch
                        .body
                        .stmts
                        .iter()
                        .for_each(|s| collect_used_stmt(s, used));
                }
                if let Some(fin) = &tc.finally {
                    fin.stmts.iter().for_each(|s| collect_used_stmt(s, used));
                }
            }
            Stmt::Assert(a) => {
                collect_used_expr(&a.condition, used);
                if let Some(m) = &a.message {
                    collect_used_expr(m, used);
                }
            }
            Stmt::LocalFunc(lf) => collect_used_body(&lf.body, used),
            _ => {}
        }
    }

    fn collect_used_expr(expr: &Expr, used: &mut HashSet<String>) {
        match expr {
            Expr::Ident(id) => {
                used.insert(id.name.clone());
            }
            Expr::StringLit(s) => collect_interpolation_idents(&s.raw, used),
            Expr::Unary { operand, .. } => collect_used_expr(operand, used),
            Expr::PostfixIncDec { operand, .. } => collect_used_expr(operand, used),
            Expr::Binary { left, right, .. } => {
                collect_used_expr(left, used);
                collect_used_expr(right, used);
            }
            Expr::Assign { target, value, .. } => {
                collect_used_expr(target, used);
                collect_used_expr(value, used);
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                collect_used_expr(condition, used);
                collect_used_expr(then_expr, used);
                collect_used_expr(else_expr, used);
            }
            Expr::Is { expr, .. } => collect_used_expr(expr, used),
            Expr::As { expr, .. } => collect_used_expr(expr, used),
            Expr::Field { object, .. } => collect_used_expr(object, used),
            Expr::Index { object, index, .. } => {
                collect_used_expr(object, used);
                collect_used_expr(index, used);
            }
            Expr::Call { callee, args, .. } => {
                collect_used_expr(callee, used);
                collect_used_args(args, used);
            }
            Expr::Cascade {
                object, sections, ..
            } => {
                collect_used_expr(object, used);
                for s in sections {
                    match &s.op {
                        CascadeOp::Index(e, _) => collect_used_expr(e, used),
                        CascadeOp::Call(_, _, args) => collect_used_args(args, used),
                        CascadeOp::Assign(t, _, v) => {
                            collect_used_expr(t, used);
                            collect_used_expr(v, used);
                        }
                        CascadeOp::Field(_, _) => {}
                    }
                }
            }
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for e in elements {
                    collect_used_collection_element(e, used);
                }
            }
            Expr::Map {
                entries, elements, ..
            } => {
                for entry in entries {
                    collect_used_expr(&entry.key, used);
                    collect_used_expr(&entry.value, used);
                }
                // Comprehension maps (`{ for (..) k: v }`) leave `entries` empty and
                // put everything in `elements`; walk them so a param used only there
                // is counted as used.
                for e in map_element_exprs(elements) {
                    collect_used_expr(e, used);
                }
            }
            Expr::Record { fields, .. } => fields
                .iter()
                .for_each(|f| collect_used_expr(&f.value, used)),
            Expr::FuncExpr { body, .. } => collect_used_body(body, used),
            Expr::New { args, .. } => collect_used_args(args, used),
            Expr::Await { expr, .. } => collect_used_expr(expr, used),
            Expr::Throw { expr, .. } => collect_used_expr(expr, used),
            Expr::NullAssert { operand, .. } => collect_used_expr(operand, used),
            Expr::Switch { subject, arms, .. } => {
                collect_used_expr(subject, used);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        collect_used_expr(g, used);
                    }
                    collect_used_expr(&arm.body, used);
                }
            }
            _ => {}
        }
    }

    fn collect_used_args(args: &ArgList, used: &mut HashSet<String>) {
        for a in &args.positional {
            collect_used_expr(a, used);
        }
        for n in &args.named {
            collect_used_expr(&n.value, used);
        }
    }

    fn collect_used_collection_element(el: &CollectionElement, used: &mut HashSet<String>) {
        match el {
            CollectionElement::Expr(e) => collect_used_expr(e, used),
            CollectionElement::NullAware { expr, .. } => collect_used_expr(expr, used),
            CollectionElement::Spread { expr, .. } => collect_used_expr(expr, used),
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                // The condition references params too (`[if (isGrouped) ...]`); a
                // param used only there must count as used.
                if let IfCondition::Expr(cond) = condition {
                    collect_used_expr(cond, used);
                }
                collect_used_collection_element(then_elem, used);
                if let Some(ee) = else_elem {
                    collect_used_collection_element(ee, used);
                }
            }
            CollectionElement::For {
                iterable, element, ..
            } => {
                collect_used_expr(iterable, used);
                collect_used_collection_element(element, used);
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
                                collect_used_expr(e, used);
                            }
                        }
                    }
                    Some(ForInit::ForIn { iterable, .. }) => {
                        collect_used_expr(iterable, used);
                    }
                    Some(ForInit::PatternForIn { iterable, .. }) => {
                        collect_used_expr(iterable, used);
                    }
                    Some(ForInit::Exprs(es)) => {
                        for e in es {
                            collect_used_expr(e, used);
                        }
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    collect_used_expr(c, used);
                }
                for u in updates {
                    collect_used_expr(u, used);
                }
                collect_used_collection_element(element, used);
            }
        }
    }

    /// Extract identifiers referenced in string interpolations (`$name`, `${...}`).
    fn collect_interpolation_idents(raw: &str, used: &mut HashSet<String>) {
        let bytes = raw.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'$' {
                i += 1;
                if i < bytes.len() && bytes[i] == b'{' {
                    // ${ ... } — collect every identifier-like token until the closing brace.
                    i += 1;
                    while i < bytes.len() && bytes[i] != b'}' {
                        if is_ident_start(bytes[i]) {
                            let start = i;
                            while i < bytes.len() && is_ident_continue(bytes[i]) {
                                i += 1;
                            }
                            used.insert(raw[start..i].to_string());
                        } else {
                            i += 1;
                        }
                    }
                } else if i < bytes.len() && is_ident_start(bytes[i]) {
                    let start = i;
                    while i < bytes.len() && is_ident_continue(bytes[i]) {
                        i += 1;
                    }
                    used.insert(raw[start..i].to_string());
                }
            } else {
                i += 1;
            }
        }
    }

    fn is_ident_start(b: u8) -> bool {
        b == b'_' || b.is_ascii_alphabetic()
    }

    fn is_ident_continue(b: u8) -> bool {
        b == b'_' || b.is_ascii_alphanumeric()
    }
}
