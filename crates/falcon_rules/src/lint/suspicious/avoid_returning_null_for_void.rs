//! Flags `return null;` (and `=> null` bodies) inside functions whose declared
//! return type is `void` or `Future<void>`. Ported from package:lints'
//! `avoid_returning_null_for_void`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_constructor_decl, walk_expr, walk_function_decl, walk_getter_decl,
    walk_method_decl, walk_setter_decl, walk_stmt,
};

pub struct AvoidReturningNullForVoid;

impl Rule for AvoidReturningNullForVoid {
    fn name(&self) -> &'static str {
        "avoid-returning-null-for-void"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            void_stack: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
    /// Void-ness of each enclosing function boundary. The innermost frame decides
    /// whether a `return null;` is flagged; a nested function pushes its own frame
    /// so it is judged by its own return type.
    void_stack: Vec<bool>,
}

impl Collector {
    fn flag(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "avoid-returning-null-for-void",
            Severity::Warning,
            "Don't return null from a function with a void return type.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    /// An arrow body `=> null` of a void function is the expression form of
    /// `return null;`, so check it at the function boundary.
    fn check_arrow(&mut self, body: Option<&FunctionBody>, is_void: bool) {
        if !is_void {
            return;
        }
        if let Some(FunctionBody::Arrow(expr, _)) = body
            && matches!(expr.as_ref(), Expr::NullLit { .. })
        {
            self.flag(expr.span());
        }
    }
}

fn is_void_return(rt: &Option<DartType>) -> bool {
    match rt {
        Some(DartType::Void { .. }) => true,
        Some(DartType::Named(n)) => {
            n.segments.last().is_some_and(|s| s.name == "Future")
                && n.type_args.len() == 1
                && matches!(n.type_args[0], DartType::Void { .. })
        }
        _ => false,
    }
}

impl Visitor for Collector {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        let is_void = is_void_return(&node.return_type);
        self.check_arrow(node.body.as_ref(), is_void);
        self.void_stack.push(is_void);
        walk_function_decl(self, node);
        self.void_stack.pop();
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        let is_void = is_void_return(&node.return_type);
        self.check_arrow(node.body.as_ref(), is_void);
        self.void_stack.push(is_void);
        walk_method_decl(self, node);
        self.void_stack.pop();
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        let is_void = is_void_return(&node.return_type);
        self.check_arrow(node.body.as_ref(), is_void);
        self.void_stack.push(is_void);
        walk_getter_decl(self, node);
        self.void_stack.pop();
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        // Setters are always void-returning.
        self.check_arrow(node.body.as_ref(), true);
        self.void_stack.push(true);
        walk_setter_decl(self, node);
        self.void_stack.pop();
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        // Constructors don't return values; never flag inside one.
        self.void_stack.push(false);
        walk_constructor_decl(self, node);
        self.void_stack.pop();
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::Return(r) => {
                if self.void_stack.last() == Some(&true)
                    && matches!(&r.value, Some(Expr::NullLit { .. }))
                {
                    self.flag(&r.span);
                }
                walk_stmt(self, node);
            }
            Stmt::LocalFunc(lf) => {
                let is_void = is_void_return(&lf.return_type);
                self.check_arrow(Some(&lf.body), is_void);
                self.void_stack.push(is_void);
                walk_stmt(self, node);
                self.void_stack.pop();
            }
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { body, .. } = node {
            // Function expressions have no written return type, so they can never
            // be declared `void`.
            self.check_arrow(Some(body), false);
            self.void_stack.push(false);
            walk_expr(self, node);
            self.void_stack.pop();
        } else {
            walk_expr(self, node);
        }
    }
}
