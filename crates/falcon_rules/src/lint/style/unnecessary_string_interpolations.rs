//! Flags a string whose entire content is a single interpolation of a provably
//! non-nullable `String`. Ported from package:lints `unnecessary_string_interpolations`.
//! `'$x'` / `'${x}'` where the whole string is just one interpolated `String`
//! expression is equivalent to writing that expression directly.
//!
//! The type proof is file-local ([`LocalTypes`]), widened by the project index's
//! declared/builtin return types for member accesses and calls, and
//! sound-over-precise: it fires only when the interpolated expression is *known*
//! to be a non-nullable `String` (a string literal, a local/param declared
//! `String`, `String + String`, a call returning `String`, …).
//! `String?` and unknown types never fire — replacing `'${n + 1}'` (an `int`)
//! with `n + 1` would silently change the value's type, the very bug this guards.

use falcon_analyze::{AnalyzeContext, LocalTypes, ProjectIndex, Rule, StaticType};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program, walk_stmt};

pub struct UnnecessaryStringInterpolations;

impl Rule for UnnecessaryStringInterpolations {
    fn name(&self) -> &'static str {
        "unnecessary-string-interpolations"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            lt: LocalTypes::new(),
            project: ctx.project,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct StrLit<'a> {
    is_raw: bool,
    content: &'a str,
}

fn parse_str_lit(raw: &str) -> Option<StrLit<'_>> {
    let is_raw = raw.as_bytes().first() == Some(&b'r');
    let prefix = usize::from(is_raw);
    let rest = &raw[prefix..];
    let dlen = if rest.starts_with("'''") || rest.starts_with("\"\"\"") {
        3
    } else if rest.starts_with('\'') || rest.starts_with('"') {
        1
    } else {
        return None;
    };
    let closing = &rest[..dlen];
    if rest.len() < 2 * dlen || !rest[dlen..].ends_with(closing) {
        return None;
    }
    Some(StrLit {
        is_raw,
        content: &raw[prefix + dlen..raw.len() - dlen],
    })
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

// `$` is excluded so that `'$a$b'` (two interpolations) is not mistaken for one identifier.
fn is_ident_continue(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// True when `content` is exactly one interpolation and nothing else: `${...}` spanning the
/// whole content, or `$identifier` spanning the whole content.
fn is_whole_interpolation(content: &str) -> bool {
    let bytes = content.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'$' {
        return false;
    }
    if bytes[1] == b'{' {
        // `${...}` must close exactly at the final byte, with a non-empty expression.
        let mut depth = 0usize;
        let mut i = 1;
        while i < bytes.len() {
            match bytes[i] {
                b'\\' => i += 1,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return i == bytes.len() - 1 && i > 2;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        false
    } else {
        // `$identifier` covering the whole content.
        is_ident_start(bytes[1]) && bytes[1..].iter().all(|&c| is_ident_continue(c))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    file: String,
    lt: LocalTypes,
    project: Option<&'a ProjectIndex>,
}

impl Collector<'_> {
    /// Whether the interpolated expression is provably a non-nullable `String`.
    ///
    /// [`LocalTypes`] answers for literals, locals and params; it returns
    /// `Unknown` for member accesses and calls, so those fall back to the
    /// project index's declared/builtin return type keyed on the member name
    /// (`toUpperCase` -> `String`). A null-aware access (`a?.b`) is never
    /// proven — its result is nullable regardless of the member's own type.
    fn is_non_nullable_string(&self, expr: &Expr) -> bool {
        if matches!(
            self.lt.of_expr(expr),
            StaticType::String { nullable: false }
        ) {
            return true;
        }
        let Some(index) = self.project else {
            return false;
        };
        let member = match expr {
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Field {
                    field,
                    is_null_safe: false,
                    ..
                } => &field.name,
                Expr::Ident(id) => &id.name,
                _ => return false,
            },
            Expr::Field {
                field,
                is_null_safe: false,
                ..
            } => &field.name,
            _ => return false,
        };
        matches!(
            index.return_type(member),
            StaticType::String { nullable: false }
        )
    }

    /// Walk a function/method/getter/setter/closure body whose signature bindings
    /// already live in the current (innermost) scope.
    fn walk_body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(b) => {
                for s in &b.stmts {
                    self.visit_stmt(s);
                }
            }
            FunctionBody::Arrow(e, _) => self.visit_expr(e),
            FunctionBody::Native(_, _) => {}
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        self.lt.bind_params(&node.params);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        self.lt.bind_params(&node.params);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        self.lt.bind_params(&node.params);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let saved = std::mem::replace(&mut self.lt, LocalTypes::new());
        let ty = node
            .param_type
            .as_ref()
            .map(StaticType::from_dart_type)
            .unwrap_or(StaticType::Unknown);
        self.lt.declare(node.param.name.clone(), ty);
        if let Some(body) = &node.body {
            self.walk_body(body);
        }
        self.lt = saved;
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(lv) => {
                for d in &lv.declarators {
                    if let Some(init) = &d.initializer {
                        self.visit_expr(init);
                    }
                }
                self.lt.declare_local(lv);
            }
            Stmt::PatternDecl(pd) => {
                self.visit_expr(&pd.init);
                self.lt.bind_pattern(&pd.pattern);
            }
            Stmt::Block(b) => {
                self.lt.push_scope();
                for s in &b.stmts {
                    self.visit_stmt(s);
                }
                self.lt.pop_scope();
            }
            Stmt::For(f) => {
                self.lt.push_scope();
                if let Some(init) = &f.init {
                    if let ForInit::VarDecl(lv) = init {
                        for d in &lv.declarators {
                            if let Some(e) = &d.initializer {
                                self.visit_expr(e);
                            }
                        }
                    }
                    self.lt.bind_for_init(init);
                }
                if let Some(cond) = &f.condition {
                    self.visit_expr(cond);
                }
                for u in &f.update {
                    self.visit_expr(u);
                }
                self.visit_stmt(&f.body);
                self.lt.pop_scope();
            }
            Stmt::TryCatch(tc) => {
                self.lt.push_scope();
                for s in &tc.body.stmts {
                    self.visit_stmt(s);
                }
                self.lt.pop_scope();
                for catch in &tc.catches {
                    self.lt.push_scope();
                    self.lt.bind_catch(catch);
                    for s in &catch.body.stmts {
                        self.visit_stmt(s);
                    }
                    self.lt.pop_scope();
                }
                if let Some(fin) = &tc.finally {
                    self.lt.push_scope();
                    for s in &fin.stmts {
                        self.visit_stmt(s);
                    }
                    self.lt.pop_scope();
                }
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::FuncExpr { params, body, .. } => {
                self.lt.push_scope();
                self.lt.bind_params(params);
                self.walk_body(body);
                self.lt.pop_scope();
            }
            Expr::Assign { target, value, .. } => {
                self.visit_expr(target);
                self.visit_expr(value);
                // Track the reassignment so a later string sees the current type.
                if let Expr::Ident(id) = target.as_ref() {
                    let ty = self.lt.of_expr(value);
                    self.lt.reassign(&id.name, ty);
                }
            }
            _ => walk_expr(self, node),
        }
    }

    fn visit_string_lit(&mut self, node: &StringLitNode) {
        let Some(lit) = parse_str_lit(&node.raw) else {
            return;
        };
        // The whole literal must be exactly one interpolation, and that
        // interpolation must have parsed cleanly to a single expression.
        if lit.is_raw || !is_whole_interpolation(lit.content) || node.interpolations.len() != 1 {
            return;
        }
        // Fire only when the interpolated expression is provably a non-nullable
        // `String`; `String?` and unknown types are left alone.
        if !self.is_non_nullable_string(&node.interpolations[0].expr) {
            return;
        }
        self.diags.push(Diagnostic::new(
            "unnecessary-string-interpolations",
            Severity::Warning,
            "Unnecessary string interpolation; use the expression directly.",
            self.file.clone(),
            DiagSpan {
                start: node.span.start,
                end: node.span.end,
            },
        ));
    }
}
