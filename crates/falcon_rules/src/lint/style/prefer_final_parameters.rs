//! Prefer `final` for parameters that are never reassigned.
//!
//! Declaring a parameter `final` documents that the function never rebinds it
//! and lets the compiler enforce that. Every parameter of a function,
//! method, setter, operator, constructor, local function, or function literal
//! is checked, unless it is already `final`, is an initializing formal or an
//! extension-type representation formal (implicitly final), is the wildcard `_`
//! (Dart 3.7+; in older libraries `_` is an ordinary name and is checked), or is written
//! anywhere in the body (including from closures). Function-typed formals
//! (`void f(int callback())`) are checked like any other parameter. Like the
//! official lint, the rule is off in libraries with primary constructors
//! enabled (Dart 3.13+).
//!
//! ```dart
//! void greet(String name) => print(name); // flagged
//! void greet(final String name) => print(name); // ok
//! ```
//!
//! Adopted from the official `prefer_final_parameters` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Feature, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::{
    Visitor, walk_class_member, walk_expr, walk_function_decl, walk_method_decl, walk_setter_decl,
    walk_stmt,
};

const RULE: &str = "prefer-final-parameters";

#[derive(Default)]
pub struct PreferFinalParameters;

impl Rule for PreferFinalParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let version = ctx.language_version();
        if version.supports(Feature::PrimaryConstructors) {
            return Vec::new();
        }
        let mutated = ctx.mutated_declarations(program);
        let mut collector = Collector {
            ctx,
            docs: ctx.syntax.doc_blocks(ctx.source).collect(),
            mutated,
            wildcards: version.supports(Feature::WildcardVariables),
            diagnostics: Vec::new(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    docs: Vec<falcon_syntax::token::DocBlock<'ctx>>,
    mutated: &'a HashSet<usize>,
    wildcards: bool,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check_name(&mut self, name: &Identifier, span: Span) {
        if (self.wildcards && name.name == "_") || self.mutated.contains(&name.span.start) {
            return;
        }
        let doc = self
            .docs
            .binary_search_by_key(&span.start, |doc| doc.next_token)
            .ok()
            .map(|index| &self.docs[index]);
        let span = doc.map_or(span, |doc| Span {
            start: doc.start,
            end: span.end,
        });
        self.diagnostics.push(self.ctx.diagnostic(
            RULE,
            format!("The parameter '{}' should be final.", name.name),
            span,
        ));
    }

    fn check(&mut self, params: &FormalParamList) {
        for param in &params.params {
            if param.is_final || param.initializing.is_some() {
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
        let tokens = self.ctx.tokens();
        let anchor = node
            .param_annotations
            .first()
            .map(|annotation| annotation.span.start)
            .or_else(|| node.param_type.as_ref().map(|ty| ty.span().start))
            .unwrap_or(node.param.span.start);
        let before_anchor = tokens.partition_point(|token| token.offset() < anchor);
        let before_name = tokens.partition_point(|token| token.offset() < node.param.span.start);
        let open = tokens[..before_anchor]
            .iter()
            .rposition(|token| token.kind == TokenKind::LParen)
            .map_or(before_anchor, |index| index + 1);
        let parameter_tokens = &tokens[open..before_name];
        if !parameter_tokens
            .iter()
            .any(|token| token.kind == TokenKind::Final)
        {
            let start = parameter_tokens
                .iter()
                .find(|token| !token.is_trivia())
                .map_or(node.param.span.start, |token| token.offset());
            self.check_name(
                &node.param,
                Span {
                    start,
                    end: node.param.span.end,
                },
            );
        }
        walk_setter_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.check(&operator.params);
        }
        walk_class_member(self, node);
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) {
        // Primary constructors in pre-3.13 libraries are extension-type
        // representation declarations: their parameters declare final fields.
        // In 3.13+ the lint is disabled for the whole library above.
        if !node.is_primary {
            self.check(node.params);
        }
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
