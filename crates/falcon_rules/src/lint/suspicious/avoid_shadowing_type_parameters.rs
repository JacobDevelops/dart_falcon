//! Flags a type parameter that shadows a type parameter of an enclosing
//! declaration.
//!
//! When a nested declaration — a method, local function, function expression, or
//! generic function type or typedef — reuses the name of a type parameter from
//! its surrounding class or function, the inner name hides the outer one. Code in
//! the nested scope can no longer refer to the enclosing type, and a reader can
//! easily mistake the two unrelated types for the same one, which invites subtle
//! type errors. Rename the inner parameter to something distinct.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidShadowingTypeParameters;

impl Rule for AvoidShadowingTypeParameters {
    fn name(&self) -> &'static str {
        "avoid-shadowing-type-parameters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut checker = Checker {
            diags: Vec::new(),
            ctx,
            scope: Vec::new(),
        };
        for decl in &program.declarations {
            checker.scan_top(decl);
        }
        checker.diags
    }
}

const MESSAGE: &str = "Avoid shadowing type parameters.";

struct Checker<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
    scope: Vec<String>,
}

impl Checker<'_, '_> {
    /// Report each parameter in `tps` that shadows an enclosing one, then push
    /// all of them onto the scope. Returns the number pushed, for `leave`.
    fn enter(&mut self, tps: &[TypeParam]) -> usize {
        for tp in tps {
            if self.scope.contains(&tp.name.name) {
                self.diags.push(Diagnostic::new(
                    "avoid-shadowing-type-parameters",
                    Severity::Warning,
                    MESSAGE,
                    self.ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: tp.name.span.start,
                        end: tp.name.span.end,
                    },
                ));
            }
        }
        for tp in tps {
            self.scope.push(tp.name.name.clone());
        }
        tps.len()
    }

    fn leave(&mut self, n: usize) {
        self.scope.truncate(self.scope.len() - n);
    }

    fn scan_top(&mut self, decl: &TopLevelDecl) {
        match decl {
            TopLevelDecl::Class(c) => {
                let n = self.enter(&c.type_params);
                for m in &c.members {
                    self.scan_member(m);
                }
                self.leave(n);
            }
            TopLevelDecl::MixinClass(c) => {
                let n = self.enter(&c.type_params);
                for m in &c.members {
                    self.scan_member(m);
                }
                self.leave(n);
            }
            TopLevelDecl::Mixin(m) => {
                let n = self.enter(&m.type_params);
                for member in &m.members {
                    self.scan_member(member);
                }
                self.leave(n);
            }
            TopLevelDecl::Enum(e) => {
                let n = self.enter(&e.type_params);
                for m in &e.members {
                    self.scan_member(m);
                }
                self.leave(n);
            }
            TopLevelDecl::Extension(ext) => {
                let n = self.enter(&ext.type_params);
                for m in &ext.members {
                    self.scan_member(m);
                }
                self.leave(n);
            }
            TopLevelDecl::ExtensionType(x) => {
                let n = self.enter(&x.type_params);
                for m in &x.members {
                    self.scan_member(m);
                }
                self.leave(n);
            }
            TopLevelDecl::Function(f) => {
                let n = self.enter(&f.type_params);
                self.scan_params(&f.params);
                if let Some(t) = &f.return_type {
                    self.scan_type(t);
                }
                self.scan_opt_body(&f.body);
                self.leave(n);
            }
            TopLevelDecl::TypeAlias(t) => {
                let n = self.enter(&t.type_params);
                self.scan_type(&t.aliased);
                self.leave(n);
            }
            TopLevelDecl::Variable(v) => {
                if let Some(t) = &v.var_type {
                    self.scan_type(t);
                }
            }
            _ => {}
        }
    }

    fn scan_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Method(m) => {
                let n = self.enter(&m.type_params);
                self.scan_params(&m.params);
                if let Some(t) = &m.return_type {
                    self.scan_type(t);
                }
                self.scan_opt_body(&m.body);
                self.leave(n);
            }
            ClassMember::Constructor(c) => {
                self.scan_params(&c.params);
                self.scan_opt_body(&c.body);
            }
            ClassMember::Getter(g) => {
                if let Some(t) = &g.return_type {
                    self.scan_type(t);
                }
                self.scan_opt_body(&g.body);
            }
            ClassMember::Setter(s) => {
                if let Some(t) = &s.param_type {
                    self.scan_type(t);
                }
                self.scan_opt_body(&s.body);
            }
            ClassMember::Field(f) => {
                if let Some(t) = &f.field_type {
                    self.scan_type(t);
                }
            }
            _ => {}
        }
    }

    fn scan_params(&mut self, params: &FormalParamList) {
        for p in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            if let Some(t) = &p.param_type {
                self.scan_type(t);
            }
        }
    }

    /// Inspect a type for generic function types (`R Function<T>(...)`), whose
    /// type parameters can shadow an enclosing declaration's.
    fn scan_type(&mut self, ty: &DartType) {
        match ty {
            DartType::Function(ft) => {
                let n = self.enter(&ft.type_params);
                if let Some(ret) = &ft.return_type {
                    self.scan_type(ret);
                }
                for p in &ft.params {
                    self.scan_type(&p.param_type);
                }
                self.leave(n);
            }
            DartType::Named(nt) => {
                for arg in &nt.type_args {
                    self.scan_type(arg);
                }
            }
            DartType::Record(rt) => {
                for t in &rt.positional {
                    self.scan_type(t);
                }
                for f in &rt.named {
                    self.scan_type(&f.field_type);
                }
            }
            _ => {}
        }
    }

    fn scan_opt_body(&mut self, body: &Option<FunctionBody>) {
        if let Some(b) = body {
            self.scan_body(b);
        }
    }

    fn scan_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(b) => {
                for s in &b.stmts {
                    self.scan_stmt(s);
                }
            }
            FunctionBody::Arrow(e, _) => self.scan_expr(e),
            FunctionBody::Native(..) => {}
        }
    }

    fn scan_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::LocalFunc(lf) => {
                let n = self.enter(&lf.type_params);
                self.scan_params(&lf.params);
                if let Some(t) = &lf.return_type {
                    self.scan_type(t);
                }
                self.scan_body(&lf.body);
                self.leave(n);
            }
            Stmt::LocalVar(lv) => {
                if let Some(t) = &lv.var_type {
                    self.scan_type(t);
                }
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        self.scan_expr(init);
                    }
                }
            }
            Stmt::Block(b) => {
                for s in &b.stmts {
                    self.scan_stmt(s);
                }
            }
            Stmt::If(i) => {
                self.scan_stmt(&i.then_branch);
                if let Some(eb) = &i.else_branch {
                    self.scan_stmt(eb);
                }
            }
            Stmt::For(f) => self.scan_stmt(&f.body),
            Stmt::While(w) => self.scan_stmt(&w.body),
            Stmt::DoWhile(d) => self.scan_stmt(&d.body),
            Stmt::Switch(sw) => {
                for case in &sw.cases {
                    for s in &case.body {
                        self.scan_stmt(s);
                    }
                }
            }
            Stmt::TryCatch(tc) => {
                for s in &tc.body.stmts {
                    self.scan_stmt(s);
                }
                for catch in &tc.catches {
                    for s in &catch.body.stmts {
                        self.scan_stmt(s);
                    }
                }
                if let Some(fin) = &tc.finally {
                    for s in &fin.stmts {
                        self.scan_stmt(s);
                    }
                }
            }
            Stmt::Return(r) => {
                if let Some(v) = &r.value {
                    self.scan_expr(v);
                }
            }
            Stmt::Expr(e) => self.scan_expr(&e.expr),
            _ => {}
        }
    }

    fn scan_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::FuncExpr {
                type_params,
                params,
                body,
                ..
            } => {
                let n = self.enter(type_params);
                self.scan_params(params);
                self.scan_body(body);
                self.leave(n);
            }
            Expr::Call { callee, args, .. } => {
                self.scan_expr(callee);
                for a in &args.positional {
                    self.scan_expr(a);
                }
                for a in &args.named {
                    self.scan_expr(&a.value);
                }
            }
            Expr::Binary { left, right, .. } => {
                self.scan_expr(left);
                self.scan_expr(right);
            }
            Expr::Assign { value, .. } => self.scan_expr(value),
            Expr::Await { expr, .. } | Expr::Throw { expr, .. } => self.scan_expr(expr),
            Expr::Conditional {
                then_expr,
                else_expr,
                ..
            } => {
                self.scan_expr(then_expr);
                self.scan_expr(else_expr);
            }
            _ => {}
        }
    }
}
