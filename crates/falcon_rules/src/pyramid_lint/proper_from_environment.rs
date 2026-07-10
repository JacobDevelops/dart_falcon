use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt, walk_top_level_decl};

pub struct ProperFromEnvironment;

impl Rule for ProperFromEnvironment {
    fn name(&self) -> &'static str {
        "proper_from_environment"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            in_const: false,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
    /// True while visiting a const context (const initializer / const literal /
    /// const construction), where `*.fromEnvironment` is evaluated at compile time.
    in_const: bool,
}

impl Collector {
    fn visit_initializers(&mut self, declarators: &[VarDeclarator], is_const: bool) {
        let prev = self.in_const;
        self.in_const = self.in_const || is_const;
        for d in declarators {
            if let Some(init) = &d.initializer {
                self.visit_expr(init);
            }
        }
        self.in_const = prev;
    }
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((span, is_const_ctor)) = from_environment_invocation(node)
            && !is_const_ctor
            && !self.in_const
        {
            self.diags.push(Diagnostic::new(
                "proper_from_environment",
                Severity::Warning,
                "*.fromEnvironment must be used in a const context, otherwise it returns the default at runtime.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }

        let prev = self.in_const;
        match node {
            Expr::New { is_const: true, .. }
            | Expr::List { is_const: true, .. }
            | Expr::Set { is_const: true, .. }
            | Expr::Map { is_const: true, .. } => self.in_const = true,
            _ => {}
        }
        walk_expr(self, node);
        self.in_const = prev;
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(lv) = node {
            self.visit_initializers(&lv.declarators, lv.is_const);
            return;
        }
        walk_stmt(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.visit_initializers(&node.declarators, node.is_const);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node {
            self.visit_initializers(&v.declarators, v.is_const);
            return;
        }
        walk_top_level_decl(self, node);
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        // Annotation arguments are always a const context.
        if let Some(args) = &node.args {
            let prev = self.in_const;
            self.in_const = true;
            for a in &args.positional {
                self.visit_expr(a);
            }
            for a in &args.named {
                self.visit_expr(&a.value);
            }
            self.in_const = prev;
        }
    }
}

/// Detect a `String`/`int`/`bool` `.fromEnvironment(...)` invocation, in either
/// the implicit-call form (`String.fromEnvironment(...)`) or the explicit
/// `new`/`const` form. Returns the invocation span and whether it is a `const`
/// construction.
fn from_environment_invocation(expr: &Expr) -> Option<(&Span, bool)> {
    match expr {
        Expr::Call { callee, span, .. } => {
            if let Expr::Field { object, field, .. } = callee.as_ref()
                && field.name == "fromEnvironment"
                && let Expr::Ident(id) = object.as_ref()
                && is_env_type(&id.name)
            {
                return Some((span, false));
            }
            None
        }
        Expr::New {
            dart_type: DartType::Named(nt),
            constructor_name: Some(cn),
            is_const,
            span,
            ..
        } if cn.name == "fromEnvironment"
            && nt.segments.last().is_some_and(|s| is_env_type(&s.name)) =>
        {
            Some((span, *is_const))
        }
        _ => None,
    }
}

fn is_env_type(name: &str) -> bool {
    matches!(name, "String" | "int" | "bool")
}
