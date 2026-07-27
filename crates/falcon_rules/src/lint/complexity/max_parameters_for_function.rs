//! Flags callable declarations and function types with more than the configured
//! number of parameters.
//!
//! A long parameter list is hard to call correctly and usually signals that
//! some arguments belong together in an object. The rule counts required,
//! optional-positional, and named parameters on functions, local functions,
//! closures, methods, constructors, operators, setters, and function types.
//!
//! ## Options
//!
//! `max_parameters` (integer, default: 5) — flag when the parameter count
//! exceeds this.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_constructor_decl, walk_dart_type, walk_expr, walk_function_decl,
    walk_method_decl, walk_stmt,
};

pub struct MaxParametersForFunction;

impl Rule for MaxParametersForFunction {
    fn name(&self) -> &'static str {
        "max-parameters-for-function"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            threshold: max_parameters_option(ctx),
            file: ctx.file_path.to_string_lossy().into_owned(),
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

fn max_parameters_option(ctx: &AnalyzeContext) -> usize {
    crate::meta::meta_for("max-parameters-for-function")
        .and_then(|m| ctx.rule_options(m.group, "max-parameters-for-function"))
        .and_then(|o| o.get("max_parameters"))
        .and_then(|v| v.as_u64())
        .map(|v| v as usize)
        .unwrap_or(5)
}

struct Collector {
    threshold: usize,
    file: String,
    diags: Vec<Diagnostic>,
}

impl Collector {
    fn check(&mut self, count: usize, span: &Span) {
        if count <= self.threshold {
            return;
        }
        self.diags.push(Diagnostic::new(
            "max-parameters-for-function",
            Severity::Warning,
            format!("Function has too many parameters (max {}).", self.threshold),
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn check_formals(&mut self, params: &FormalParamList, span: &Span) {
        self.check(
            params.positional.len() + params.optional_positional.len() + params.named.len(),
            span,
        );
    }
}

impl Visitor for Collector {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.check_formals(&node.params, &node.span);
        walk_function_decl(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.check_formals(&node.params, &node.span);
        walk_constructor_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check_formals(&node.params, &node.span);
        walk_method_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.check(1, &node.span);
        falcon_syntax::visitor::walk_setter_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.check_formals(&operator.params, &operator.span);
        }
        falcon_syntax::visitor::walk_class_member(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Function(function) = node {
            self.check(function.params.len(), &function.span);
        }
        walk_dart_type(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        if let Some(params) = &node.function_params {
            self.check_formals(params, &node.span);
        }
        falcon_syntax::visitor::walk_formal_param(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node {
            self.check_formals(&function.params, &function.span);
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { params, span, .. } = node {
            self.check_formals(params, span);
        }
        walk_expr(self, node);
    }
}
