//! Flags unnamed numeric literals (magic numbers). Co-locates two independent implementations:
//! `no-magic-number` (dart_code_linter) and `no_magic_number` (pyramid_lint). These are
//! separate verbatim ports and share no logic.

/// The `no-magic-number` rule, ported from dart_code_linter.
pub use dcl::NoMagicNumber;
/// The `no_magic_number` rule, ported from pyramid_lint.
pub use pyramid::NoMagicNumber as NoMagicNumberPyramid;

mod dcl {
    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;

    pub struct NoMagicNumber;

    const EPS: f64 = 1e-9;

    /// Resolved options. dcl's `allowed` defaults to `[-1, 0, 1]`.
    struct Cfg {
        allowed: Vec<f64>,
    }

    fn cfg(ctx: &AnalyzeContext) -> Cfg {
        let allowed = crate::meta::meta_for("no-magic-number")
            .and_then(|m| ctx.rule_options(m.group, "no-magic-number"))
            .and_then(|o| o.get("allowed"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![-1.0, 0.0, 1.0]);
        Cfg { allowed }
    }

    impl Rule for NoMagicNumber {
        fn name(&self) -> &'static str {
            "no-magic-number"
        }

        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            let mut diags = Vec::new();
            let cfg = cfg(ctx);
            for decl in &program.declarations {
                scan_top(decl, &mut diags, ctx, &cfg);
            }
            diags
        }
    }

    /// Parse a numeric literal lexeme (decimal, hex, binary, double, with digit
    /// separators) into an `f64` for comparison against the allow-list.
    fn parse_num(value: &str) -> Option<f64> {
        let v = value.replace('_', "");
        if let Some(hex) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
            return i64::from_str_radix(hex, 16).ok().map(|n| n as f64);
        }
        if let Some(bin) = v.strip_prefix("0b").or_else(|| v.strip_prefix("0B")) {
            return i64::from_str_radix(bin, 2).ok().map(|n| n as f64);
        }
        v.parse::<f64>().ok()
    }

    fn is_allowed(value: &str, cfg: &Cfg) -> bool {
        match parse_num(value) {
            Some(n) => cfg.allowed.iter().any(|a| (a - n).abs() < EPS),
            None => true,
        }
    }

    /// Flag a bare numeric literal unless it is in a const context or the allow-list.
    fn flag_literal(
        value: &str,
        span: &Span,
        in_const: bool,
        diags: &mut Vec<Diagnostic>,
        ctx: &AnalyzeContext,
        cfg: &Cfg,
    ) {
        if in_const || is_allowed(value, cfg) {
            return;
        }
        diags.push(Diagnostic::new(
            "no-magic-number",
            Severity::Warning,
            "Avoid using magic numbers. Extract them to named constants or variables.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    /// True for a literal that would be a *direct* element/argument, including a
    /// negated literal (`-1`), so collection and index exemptions can spot it.
    fn is_direct_literal(expr: &Expr) -> bool {
        match expr {
            Expr::IntLit { .. } | Expr::DoubleLit { .. } => true,
            Expr::Unary { operand, .. } => is_direct_literal(operand),
            _ => false,
        }
    }

    /// Base identifier of a constructor/type reference, e.g. `DateTime` for both
    /// `DateTime(...)` and `DateTime.utc(...)`.
    fn root_name(expr: &Expr) -> Option<&str> {
        match expr {
            Expr::Ident(id) => Some(&id.name),
            Expr::Field { object, .. } => root_name(object),
            _ => None,
        }
    }

    fn type_base_name(dart_type: &DartType) -> Option<&str> {
        match dart_type {
            DartType::Named(n) => n.segments.last().map(|s| s.name.as_str()),
            _ => None,
        }
    }

    fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &Cfg) {
        match decl {
            TopLevelDecl::Function(f) => {
                if let Some(body) = &f.body {
                    scan_body(body, diags, ctx, cfg);
                }
            }
            TopLevelDecl::Class(c) => scan_members(&c.members, diags, ctx, cfg),
            TopLevelDecl::Mixin(m) => scan_members(&m.members, diags, ctx, cfg),
            TopLevelDecl::MixinClass(mc) => scan_members(&mc.members, diags, ctx, cfg),
            TopLevelDecl::Enum(e) => scan_members(&e.members, diags, ctx, cfg),
            TopLevelDecl::Extension(ext) => scan_members(&ext.members, diags, ctx, cfg),
            // Any literal inside a variable/field initializer has a
            // VariableDeclaration ancestor, so dcl exempts the whole subtree.
            TopLevelDecl::Variable(_) => {}
            _ => {}
        }
    }

    fn scan_members(
        members: &[ClassMember],
        diags: &mut Vec<Diagnostic>,
        ctx: &AnalyzeContext,
        cfg: &Cfg,
    ) {
        for member in members {
            let body = match member {
                // Field initializers are variable declarations → exempt.
                ClassMember::Field(_) => None,
                ClassMember::Method(m) => m.body.as_ref(),
                ClassMember::Constructor(c) => c.body.as_ref(),
                ClassMember::Getter(g) => g.body.as_ref(),
                ClassMember::Setter(s) => s.body.as_ref(),
                ClassMember::Operator(o) => o.body.as_ref(),
                _ => None,
            };
            if let Some(b) = body {
                scan_body(b, diags, ctx, cfg);
            }
        }
    }

    fn scan_body(
        body: &FunctionBody,
        diags: &mut Vec<Diagnostic>,
        ctx: &AnalyzeContext,
        cfg: &Cfg,
    ) {
        match body {
            FunctionBody::Block(b) => scan_stmts(&b.stmts, diags, ctx, cfg),
            FunctionBody::Arrow(e, _) => scan_expr(e, false, diags, ctx, cfg),
            FunctionBody::Native(_, _) => {}
        }
    }

    fn scan_stmts(stmts: &[Stmt], diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &Cfg) {
        for s in stmts {
            scan_stmt(s, diags, ctx, cfg);
        }
    }

    fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, cfg: &Cfg) {
        match stmt {
            Stmt::Expr(e) => scan_expr(&e.expr, false, diags, ctx, cfg),
            Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    scan_expr(v, false, diags, ctx, cfg);
                }
            }
            // Local variable initializers are exempt (VariableDeclaration ancestor).
            Stmt::LocalVar(_) => {}
            Stmt::Block(b) => scan_stmts(&b.stmts, diags, ctx, cfg),
            Stmt::If(i) => {
                if let IfCondition::Expr(e) = &i.condition {
                    scan_expr(e, false, diags, ctx, cfg);
                }
                scan_stmt(&i.then_branch, diags, ctx, cfg);
                if let Some(eb) = &i.else_branch {
                    scan_stmt(eb, diags, ctx, cfg);
                }
            }
            Stmt::While(w) => {
                scan_expr(&w.condition, false, diags, ctx, cfg);
                scan_stmt(&w.body, diags, ctx, cfg);
            }
            Stmt::DoWhile(d) => {
                scan_stmt(&d.body, diags, ctx, cfg);
                scan_expr(&d.condition, false, diags, ctx, cfg);
            }
            Stmt::For(f) => {
                if let Some(cond) = &f.condition {
                    scan_expr(cond, false, diags, ctx, cfg);
                }
                for u in &f.update {
                    scan_expr(u, false, diags, ctx, cfg);
                }
                scan_stmt(&f.body, diags, ctx, cfg);
            }
            Stmt::TryCatch(tc) => {
                scan_stmts(&tc.body.stmts, diags, ctx, cfg);
                for catch in &tc.catches {
                    scan_stmts(&catch.body.stmts, diags, ctx, cfg);
                }
                if let Some(fin) = &tc.finally {
                    scan_stmts(&fin.stmts, diags, ctx, cfg);
                }
            }
            Stmt::Switch(sw) => {
                scan_expr(&sw.subject, false, diags, ctx, cfg);
                for case in &sw.cases {
                    scan_stmts(&case.body, diags, ctx, cfg);
                }
            }
            Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx, cfg),
            Stmt::Throw(t) => scan_expr(&t.value, false, diags, ctx, cfg),
            _ => {}
        }
    }

    fn scan_expr(
        expr: &Expr,
        in_const: bool,
        diags: &mut Vec<Diagnostic>,
        ctx: &AnalyzeContext,
        cfg: &Cfg,
    ) {
        match expr {
            Expr::IntLit { value, span } | Expr::DoubleLit { value, span } => {
                flag_literal(value, span, in_const, diags, ctx, cfg);
            }
            Expr::Unary { operand, .. } => scan_expr(operand, in_const, diags, ctx, cfg),
            Expr::Binary { left, right, .. } => {
                scan_expr(left, in_const, diags, ctx, cfg);
                scan_expr(right, in_const, diags, ctx, cfg);
            }
            Expr::Call { callee, args, .. } => {
                // Literals inside a `DateTime(...)` constructor are exempt.
                if root_name(callee) == Some("DateTime") {
                    return;
                }
                scan_expr(callee, in_const, diags, ctx, cfg);
                for arg in &args.positional {
                    scan_expr(arg, in_const, diags, ctx, cfg);
                }
                for named in &args.named {
                    scan_expr(&named.value, in_const, diags, ctx, cfg);
                }
            }
            Expr::New {
                is_const,
                dart_type,
                args,
                ..
            } => {
                // A const constructor exempts its whole argument subtree; a DateTime
                // constructor is exempt regardless of constness.
                if type_base_name(dart_type) == Some("DateTime") {
                    return;
                }
                let nested_const = in_const || *is_const;
                for arg in &args.positional {
                    scan_expr(arg, nested_const, diags, ctx, cfg);
                }
                for named in &args.named {
                    scan_expr(&named.value, nested_const, diags, ctx, cfg);
                }
            }
            Expr::Field { object, .. } => scan_expr(object, in_const, diags, ctx, cfg),
            Expr::Index { object, index, .. } => {
                scan_expr(object, in_const, diags, ctx, cfg);
                // A literal used directly as an index is exempt.
                if !is_direct_literal(index) {
                    scan_expr(index, in_const, diags, ctx, cfg);
                }
            }
            Expr::Assign { target, value, .. } => {
                scan_expr(target, in_const, diags, ctx, cfg);
                scan_expr(value, in_const, diags, ctx, cfg);
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                scan_expr(condition, in_const, diags, ctx, cfg);
                scan_expr(then_expr, in_const, diags, ctx, cfg);
                scan_expr(else_expr, in_const, diags, ctx, cfg);
            }
            Expr::List {
                is_const, elements, ..
            }
            | Expr::Set {
                is_const, elements, ..
            } => {
                let nested_const = in_const || *is_const;
                for elem in elements {
                    if let CollectionElement::Expr(e) = elem {
                        // Direct elements of a list/set literal are exempt.
                        if !is_direct_literal(e) {
                            scan_expr(e, nested_const, diags, ctx, cfg);
                        }
                    }
                }
            }
            Expr::Map {
                is_const,
                entries,
                elements,
                ..
            } => {
                // Entries of a const map are exempt; entries of a non-const map are
                // still scanned (a non-const map's keys/values are not exempted).
                let nested_const = in_const || *is_const;
                if !nested_const {
                    for entry in entries {
                        scan_expr(&entry.key, false, diags, ctx, cfg);
                        scan_expr(&entry.value, false, diags, ctx, cfg);
                    }
                    for e in map_element_exprs(elements) {
                        scan_expr(e, false, diags, ctx, cfg);
                    }
                }
            }
            Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx, cfg),
            Expr::Await { expr, .. } => scan_expr(expr, in_const, diags, ctx, cfg),
            Expr::NullAssert { operand, .. } => scan_expr(operand, in_const, diags, ctx, cfg),
            _ => {}
        }
    }
}

mod pyramid {
    //! pyramid_lint `no_magic_number`: flag numeric literals other than the allowed
    //! set (0, 1, 2, -1). Top-level `const` declarations are exempt — they are the
    //! named-constant definitions that magic numbers should be extracted into.

    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;

    pub struct NoMagicNumber;

    impl Rule for NoMagicNumber {
        fn name(&self) -> &'static str {
            "no_magic_number"
        }

        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            let mut diags = Vec::new();
            for decl in &program.declarations {
                scan_top(decl, &mut diags, ctx);
            }
            diags
        }
    }

    /// `0`, `1`, `2` are allowed. `-1` parses as a unary negation of the literal `1`,
    /// so the inner literal is already covered.
    fn is_allowed(value: &str) -> bool {
        matches!(value, "0" | "1" | "2")
    }

    fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        diags.push(Diagnostic::new(
            "no_magic_number",
            Severity::Warning,
            "Avoid magic numbers. Extract this value into a named constant.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match decl {
            // Top-level `const` declarations are the canonical place for literals.
            TopLevelDecl::Variable(v) if v.is_const => {}
            TopLevelDecl::Variable(v) => {
                for d in &v.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx);
                    }
                }
            }
            TopLevelDecl::Function(f) => {
                if let Some(body) = &f.body {
                    scan_body(body, diags, ctx);
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
        match member {
            ClassMember::Field(f) => {
                for d in &f.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx);
                    }
                }
            }
            ClassMember::Method(m) => {
                if let Some(b) = &m.body {
                    scan_body(b, diags, ctx);
                }
            }
            ClassMember::Constructor(c) => {
                if let Some(b) = &c.body {
                    scan_body(b, diags, ctx);
                }
            }
            ClassMember::Getter(g) => {
                if let Some(b) = &g.body {
                    scan_body(b, diags, ctx);
                }
            }
            ClassMember::Setter(s) => {
                if let Some(b) = &s.body {
                    scan_body(b, diags, ctx);
                }
            }
            ClassMember::Operator(o) => {
                if let Some(b) = &o.body {
                    scan_body(b, diags, ctx);
                }
            }
            ClassMember::Error(_) => {}
        }
    }

    fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match body {
            FunctionBody::Block(b) => b.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx)),
            FunctionBody::Arrow(e, _) => scan_expr(e, diags, ctx),
            FunctionBody::Native(_, _) => {}
        }
    }

    fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match stmt {
            Stmt::Block(b) => b.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx)),
            Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx),
            Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    scan_expr(v, diags, ctx);
                }
            }
            Stmt::Throw(t) => scan_expr(&t.value, diags, ctx),
            Stmt::Yield(y) => scan_expr(&y.value, diags, ctx),
            Stmt::LocalVar(lv) => {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx);
                    }
                }
            }
            Stmt::If(i) => {
                if let IfCondition::Expr(c) = &i.condition {
                    scan_expr(c, diags, ctx);
                }
                scan_stmt(&i.then_branch, diags, ctx);
                if let Some(eb) = &i.else_branch {
                    scan_stmt(eb, diags, ctx);
                }
            }
            Stmt::While(w) => {
                scan_expr(&w.condition, diags, ctx);
                scan_stmt(&w.body, diags, ctx);
            }
            Stmt::DoWhile(d) => {
                scan_stmt(&d.body, diags, ctx);
                scan_expr(&d.condition, diags, ctx);
            }
            Stmt::For(f) => {
                if let Some(cond) = &f.condition {
                    scan_expr(cond, diags, ctx);
                }
                match &f.init {
                    Some(ForInit::VarDecl(lv)) => {
                        for d in &lv.declarators {
                            if let Some(init) = &d.initializer {
                                scan_expr(init, diags, ctx);
                            }
                        }
                    }
                    Some(ForInit::ForIn { iterable, .. }) => scan_expr(iterable, diags, ctx),
                    Some(ForInit::PatternForIn { iterable, .. }) => scan_expr(iterable, diags, ctx),
                    Some(ForInit::Exprs(es)) => es.iter().for_each(|e| scan_expr(e, diags, ctx)),
                    None => {}
                }
                f.update.iter().for_each(|e| scan_expr(e, diags, ctx));
                scan_stmt(&f.body, diags, ctx);
            }
            Stmt::Switch(sw) => {
                scan_expr(&sw.subject, diags, ctx);
                for case in &sw.cases {
                    case.body.iter().for_each(|s| scan_stmt(s, diags, ctx));
                }
            }
            Stmt::TryCatch(tc) => {
                tc.body.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx));
                for catch in &tc.catches {
                    catch
                        .body
                        .stmts
                        .iter()
                        .for_each(|s| scan_stmt(s, diags, ctx));
                }
                if let Some(fin) = &tc.finally {
                    fin.stmts.iter().for_each(|s| scan_stmt(s, diags, ctx));
                }
            }
            Stmt::Assert(a) => {
                scan_expr(&a.condition, diags, ctx);
                if let Some(m) = &a.message {
                    scan_expr(m, diags, ctx);
                }
            }
            Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
            _ => {}
        }
    }

    fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match expr {
            Expr::IntLit { value, span } | Expr::DoubleLit { value, span } => {
                if !is_allowed(value) {
                    flag(span, diags, ctx);
                }
            }
            Expr::Unary { operand, .. } => scan_expr(operand, diags, ctx),
            Expr::PostfixIncDec { operand, .. } => scan_expr(operand, diags, ctx),
            Expr::Binary { left, right, .. } => {
                scan_expr(left, diags, ctx);
                scan_expr(right, diags, ctx);
            }
            Expr::Assign { target, value, .. } => {
                scan_expr(target, diags, ctx);
                scan_expr(value, diags, ctx);
            }
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
            Expr::Is { expr, .. } => scan_expr(expr, diags, ctx),
            Expr::As { expr, .. } => scan_expr(expr, diags, ctx),
            Expr::Field { object, .. } => scan_expr(object, diags, ctx),
            Expr::Index { object, index, .. } => {
                scan_expr(object, diags, ctx);
                scan_expr(index, diags, ctx);
            }
            Expr::Call { callee, args, .. } => {
                scan_expr(callee, diags, ctx);
                scan_args(args, diags, ctx);
            }
            Expr::Cascade {
                object, sections, ..
            } => {
                scan_expr(object, diags, ctx);
                for s in sections {
                    match &s.op {
                        CascadeOp::Index(e, _) => scan_expr(e, diags, ctx),
                        CascadeOp::Call(_, _, args) => scan_args(args, diags, ctx),
                        CascadeOp::Assign(t, _, v) => {
                            scan_expr(t, diags, ctx);
                            scan_expr(v, diags, ctx);
                        }
                        CascadeOp::Field(_, _) => {}
                    }
                }
            }
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                for e in elements {
                    scan_collection_element(e, diags, ctx);
                }
            }
            Expr::Map {
                entries, elements, ..
            } => {
                for entry in entries {
                    scan_expr(&entry.key, diags, ctx);
                    scan_expr(&entry.value, diags, ctx);
                }
                for e in map_element_exprs(elements) {
                    scan_expr(e, diags, ctx);
                }
            }
            Expr::Record { fields, .. } => {
                fields.iter().for_each(|f| scan_expr(&f.value, diags, ctx))
            }
            Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
            Expr::New { args, .. } => scan_args(args, diags, ctx),
            Expr::Await { expr, .. } => scan_expr(expr, diags, ctx),
            Expr::Throw { expr, .. } => scan_expr(expr, diags, ctx),
            Expr::NullAssert { operand, .. } => scan_expr(operand, diags, ctx),
            Expr::Switch { subject, arms, .. } => {
                scan_expr(subject, diags, ctx);
                for arm in arms {
                    if let Some(g) = &arm.guard {
                        scan_expr(g, diags, ctx);
                    }
                    scan_expr(&arm.body, diags, ctx);
                }
            }
            _ => {}
        }
    }

    fn scan_args(args: &ArgList, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        for a in &args.positional {
            scan_expr(a, diags, ctx);
        }
        for n in &args.named {
            scan_expr(&n.value, diags, ctx);
        }
    }

    fn scan_collection_element(
        el: &CollectionElement,
        diags: &mut Vec<Diagnostic>,
        ctx: &AnalyzeContext,
    ) {
        match el {
            CollectionElement::Expr(e) => scan_expr(e, diags, ctx),
            CollectionElement::NullAware { expr, .. } => scan_expr(expr, diags, ctx),
            CollectionElement::Spread { expr, .. } => scan_expr(expr, diags, ctx),
            CollectionElement::If {
                then_elem,
                else_elem,
                ..
            } => {
                scan_collection_element(then_elem, diags, ctx);
                if let Some(ee) = else_elem {
                    scan_collection_element(ee, diags, ctx);
                }
            }
            CollectionElement::For {
                iterable, element, ..
            } => {
                scan_expr(iterable, diags, ctx);
                scan_collection_element(element, diags, ctx);
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
                                scan_expr(e, diags, ctx);
                            }
                        }
                    }
                    Some(ForInit::ForIn { iterable, .. }) => {
                        scan_expr(iterable, diags, ctx);
                    }
                    Some(ForInit::PatternForIn { iterable, .. }) => {
                        scan_expr(iterable, diags, ctx);
                    }
                    Some(ForInit::Exprs(es)) => {
                        for e in es {
                            scan_expr(e, diags, ctx);
                        }
                    }
                    None => {}
                }
                if let Some(c) = condition {
                    scan_expr(c, diags, ctx);
                }
                for u in updates {
                    scan_expr(u, diags, ctx);
                }
                scan_collection_element(element, diags, ctx);
            }
        }
    }
}
