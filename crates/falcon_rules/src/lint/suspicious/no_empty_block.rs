//! Flags empty blocks. Co-locates two independent implementations:
//! `no-empty-block` (dart_code_linter) and `no_empty_block` (pyramid_lint). These are
//! separate verbatim ports and share no logic.

/// The `no-empty-block` rule, ported from dart_code_linter.
pub use dcl::NoEmptyBlock;
/// The `no_empty_block` rule, ported from pyramid_lint.
pub use pyramid::NoEmptyBlock as NoEmptyBlockPyramid;

mod dcl {
    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;

    pub struct NoEmptyBlock;

    impl Rule for NoEmptyBlock {
        fn name(&self) -> &'static str {
            "no-empty-block"
        }

        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            let mut diags = Vec::new();
            for decl in &program.declarations {
                scan_top(decl, &mut diags, ctx);
            }
            diags
        }
    }

    fn flag_if_empty(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        if !block.stmts.is_empty() {
            return;
        }
        let end = block.span.end.min(ctx.source.len());
        let src_full = &ctx.source[block.span.start..end];
        // span_from() includes content up to the next token, so rfind('}') finds
        // the actual closing brace rather than trusting span.end directly.
        let close_pos = match src_full.rfind('}') {
            Some(p) => p,
            None => return,
        };
        let inner = &src_full[..=close_pos];
        if inner.contains("//") || inner.contains("/*") {
            return;
        }
        let close_byte = block.span.start + close_pos;
        diags.push(Diagnostic::new(
            "no-empty-block",
            Severity::Warning,
            "Avoid empty blocks — add a comment explaining the intent or remove the block",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: close_byte,
                end: close_byte + 1,
            },
        ));
    }

    fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match decl {
            TopLevelDecl::Function(f) => {
                if let Some(body) = &f.body {
                    scan_body(body, diags, ctx);
                }
            }
            TopLevelDecl::Class(c) => {
                for m in &c.members {
                    scan_member(m, diags, ctx);
                }
            }
            TopLevelDecl::Mixin(m) => {
                for mem in &m.members {
                    scan_member(mem, diags, ctx);
                }
            }
            TopLevelDecl::MixinClass(mc) => {
                for m in &mc.members {
                    scan_member(m, diags, ctx);
                }
            }
            _ => {}
        }
    }

    fn scan_member(member: &ClassMember, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        let body = match member {
            ClassMember::Method(m) => m.body.as_ref(),
            ClassMember::Constructor(c) => c.body.as_ref(),
            ClassMember::Getter(g) => g.body.as_ref(),
            ClassMember::Setter(s) => s.body.as_ref(),
            _ => None,
        };
        if let Some(b) = body {
            scan_body(b, diags, ctx);
        }
    }

    fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match body {
            FunctionBody::Block(b) => {
                flag_if_empty(b, diags, ctx);
                scan_stmts(&b.stmts, diags, ctx);
            }
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
            Stmt::Block(b) => {
                flag_if_empty(b, diags, ctx);
                scan_stmts(&b.stmts, diags, ctx);
            }
            Stmt::Expr(e) => scan_expr(&e.expr, diags, ctx),
            Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    scan_expr(v, diags, ctx);
                }
            }
            Stmt::LocalVar(lv) => {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        scan_expr(init, diags, ctx);
                    }
                }
            }
            Stmt::If(i) => {
                scan_stmt(&i.then_branch, diags, ctx);
                if let Some(eb) = &i.else_branch {
                    scan_stmt(eb, diags, ctx);
                }
            }
            Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
            Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
            Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
            Stmt::TryCatch(tc) => {
                flag_if_empty(&tc.body, diags, ctx);
                scan_stmts(&tc.body.stmts, diags, ctx);
                for catch in &tc.catches {
                    flag_if_empty(&catch.body, diags, ctx);
                    scan_stmts(&catch.body.stmts, diags, ctx);
                }
                if let Some(fin) = &tc.finally {
                    flag_if_empty(fin, diags, ctx);
                    scan_stmts(&fin.stmts, diags, ctx);
                }
            }
            Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
            _ => {}
        }
    }

    fn scan_expr(expr: &Expr, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match expr {
            Expr::FuncExpr { body, .. } => scan_body(body, diags, ctx),
            Expr::Call { callee, args, .. } => {
                scan_expr(callee, diags, ctx);
                for arg in &args.positional {
                    scan_expr(arg, diags, ctx);
                }
                for named in &args.named {
                    scan_expr(&named.value, diags, ctx);
                }
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
            _ => {}
        }
    }
}

mod pyramid {
    //! pyramid_lint `no_empty_block`: forbid empty blocks (catch, method/function
    //! bodies, if/else branches, loops). An empty block is usually a mistake or a
    //! `TODO` left behind; if intentional, add a statement or a comment.

    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;

    pub struct NoEmptyBlock;

    impl Rule for NoEmptyBlock {
        fn name(&self) -> &'static str {
            "no_empty_block"
        }

        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            let mut diags = Vec::new();
            for decl in &program.declarations {
                scan_top(decl, &mut diags, ctx);
            }
            diags
        }
    }

    fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        diags.push(Diagnostic::new(
            "no_empty_block",
            Severity::Warning,
            "Avoid empty blocks. Add a statement or a comment explaining why it is empty.",
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    /// Flag the block if empty, otherwise recurse into its statements.
    fn check_block(block: &Block, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        if block.stmts.is_empty() {
            flag(&block.span, diags, ctx);
        } else {
            for s in &block.stmts {
                scan_stmt(s, diags, ctx);
            }
        }
    }

    fn scan_body(body: &FunctionBody, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        if let FunctionBody::Block(b) = body {
            check_block(b, diags, ctx);
        }
    }

    fn scan_top(decl: &TopLevelDecl, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match decl {
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
        let body = match member {
            ClassMember::Method(m) => m.body.as_ref(),
            ClassMember::Constructor(c) => c.body.as_ref(),
            ClassMember::Getter(g) => g.body.as_ref(),
            ClassMember::Setter(s) => s.body.as_ref(),
            ClassMember::Operator(o) => o.body.as_ref(),
            _ => None,
        };
        if let Some(b) = body {
            scan_body(b, diags, ctx);
        }
    }

    fn scan_stmt(stmt: &Stmt, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
        match stmt {
            Stmt::Block(b) => check_block(b, diags, ctx),
            Stmt::If(i) => {
                scan_stmt(&i.then_branch, diags, ctx);
                if let Some(eb) = &i.else_branch {
                    scan_stmt(eb, diags, ctx);
                }
            }
            Stmt::While(w) => scan_stmt(&w.body, diags, ctx),
            Stmt::DoWhile(d) => scan_stmt(&d.body, diags, ctx),
            Stmt::For(f) => scan_stmt(&f.body, diags, ctx),
            Stmt::Switch(sw) => {
                for case in &sw.cases {
                    for s in &case.body {
                        scan_stmt(s, diags, ctx);
                    }
                }
            }
            Stmt::TryCatch(tc) => {
                check_block(&tc.body, diags, ctx);
                for catch in &tc.catches {
                    check_block(&catch.body, diags, ctx);
                }
                if let Some(fin) = &tc.finally {
                    check_block(fin, diags, ctx);
                }
            }
            Stmt::LocalFunc(lf) => scan_body(&lf.body, diags, ctx),
            _ => {}
        }
    }
}
