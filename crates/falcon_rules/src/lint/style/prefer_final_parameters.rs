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

use falcon_analyze::{
    AnalyzeContext, Feature, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
    structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

const RULE: &str = "prefer-final-parameters";

#[derive(Default)]
pub struct PreferFinalParameters;

impl Rule for PreferFinalParameters {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let version = ctx.language_version();
        if version.supports(Feature::PrimaryConstructors) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mutated = ctx.mutated_declarations(program);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            docs: ctx.syntax.doc_blocks(ctx.source).collect(),
            mutated,
            wildcards: version.supports(Feature::WildcardVariables),
            diagnostics: Vec::new(),
        }))
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

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) -> StructuralTraversal {
        self.check(&node.params);
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
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
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check(&operator.params);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'_>) -> StructuralTraversal {
        // Primary constructors in pre-3.13 libraries are extension-type
        // representation declarations: their parameters declare final fields.
        // In 3.13+ the lint is disabled for the whole library above.
        if !node.is_primary {
            self.check(node.params);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check(&function.params);
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::FuncExpr { params, .. } = node {
            self.check(params);
        }
        StructuralTraversal::Continue
    }
}
}
