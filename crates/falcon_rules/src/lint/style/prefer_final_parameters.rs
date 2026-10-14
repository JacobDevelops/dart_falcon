//! Prefer `final` for parameters that are never reassigned.
//!
//! Declaring a parameter `final` documents that the function never rebinds it
//! and lets the compiler enforce that. Every parameter of a function,
//! method, setter, operator, constructor, local function, or function literal
//! is checked, unless it is already `final`, is an initializing formal
//! (`this.x` / `super.x`, implicitly final), is the wildcard `_`, or is written
//! anywhere in the body (including from closures). Function-typed formals
//! (`void f(int callback())`) are conservatively skipped.
//!
//! This preserves the analyzer's legacy-language behavior. The analyzer disables
//! the deprecated rule when primary constructors are enabled; Falcon currently
//! does not expose that language-feature gate to lint rules.
//!
//! ```dart
//! void greet(String name) => print(name); // flagged
//! void greet(final String name) => print(name); // ok
//! ```
//!
//! Adopted from the official `prefer_final_parameters` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_constructor_decl, walk_expr, walk_function_decl,
    walk_method_decl, walk_setter_decl, walk_stmt,
};

use crate::lint::lexical_usage::{find_word, local_writes};

const RULE: &str = "prefer-final-parameters";

#[derive(Default)]
pub struct PreferFinalParameters;

impl Rule for PreferFinalParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mutated = local_writes(program, ctx.source)
            .into_iter()
            .map(|write| write.declaration)
            .collect();
        let mut collector = Collector {
            ctx,
            mutated,
            diagnostics: Vec::new(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    mutated: HashSet<usize>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check_name(&mut self, name: &Identifier, span: Span) {
        if name.name == "_" || self.mutated.contains(&name.span.start) {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            RULE,
            format!("The parameter '{}' should be final.", name.name),
            span,
        ));
    }

    fn check(&mut self, params: &FormalParamList) {
        for param in &params.params {
            if param.is_final || param.initializing.is_some() || param.function_params.is_some() {
                continue;
            }
            self.check_name(&param.name, param.span);
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        if let Some(params) = node.params() {
            self.check(params);
        }
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check(&node.params);
        walk_method_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        // The setter's single parameter carries no `final` flag in the AST; the
        // keyword can only sit between the opening `(` and the name.
        let open = self.ctx.source[..node.param.span.start]
            .rfind('(')
            .unwrap_or(node.param.span.start);
        if find_word(self.ctx.source, open, node.param.span.start, "final").is_none() {
            let span = node
                .param_type
                .as_ref()
                .map_or(node.param.span, |ty| ty.span().merge(&node.param.span));
            self.check_name(&node.param, span);
        }
        walk_setter_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.check(&operator.params);
        }
        walk_class_member(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.check(&node.params);
        walk_constructor_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node {
            self.check(&function.params);
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::FuncExpr { params, .. } = node {
            self.check(params);
        }
        walk_expr(self, node);
    }
}
