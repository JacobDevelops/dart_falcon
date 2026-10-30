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
use falcon_syntax::cst::*;
use falcon_syntax::token::TokenKind;

use crate::lint::tokens::significant_tokens_in;

#[derive(Default)]
pub struct PreferFinalParameters;

impl Rule for PreferFinalParameters {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let version = ctx.language_version();
        if version.supports(Feature::PrimaryConstructors) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mutated = ctx.mutated_declarations(program);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            docs: ctx.tree().doc_blocks(ctx.source()).collect(),
            mutated,
            wildcards: version.supports(Feature::WildcardVariables),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    docs: Vec<DocBlock<'ctx>>,
    mutated: &'a HashSet<NodeId>,
    wildcards: bool,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    fn check_name(&mut self, name: Identifier<'_>, span: Span) {
        if (self.wildcards && name.name() == "_") || self.mutated.contains(&name.syntax().id()) {
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
            format!("The parameter `{}` should be `final`.", name.name()),
            span,
        ));
    }

    fn check(&mut self, params: FormalParamList<'_>) {
        for param in params.params() {
            if param.is_final() || param.initializing().is_some() {
                continue;
            }
            self.check_name(param.name(), param.span());
        }
    }
}

structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.check(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.check(node.params());
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, node: SetterDecl<'a>) -> StructuralTraversal {
        let name = node.param();
        let anchor = node
            .param_annotations()
            .next()
            .map(|annotation| annotation.span().start)
            .or_else(|| node.param_type().map(|ty| ty.span().start))
            .unwrap_or(name.span().start);
        let open = self.ctx.tree()
            .tokens_in(self.ctx.source(), Span::new(0, anchor))
            .rev()
            .find(|token| token.kind() == TokenKind::LParen)
            .map_or(anchor, |token| token.span().end);
        let mut parameter_tokens = significant_tokens_in(
            self.ctx.tree(),
            self.ctx.source(),
            Span::new(open, name.span().start),
        );
        if !parameter_tokens.clone().any(|token| token.kind() == TokenKind::Final)
        {
            let start = parameter_tokens
                .next()
                .map_or(name.span().start, SyntaxToken::offset);
            self.check_name(
                name,
                Span {
                    start,
                    end: name.span().end,
                },
            );
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.check(operator.params());
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        // Primary constructors in pre-3.13 libraries are extension-type
        // representation declarations: their parameters declare final fields.
        // In 3.13+ the lint is disabled for the whole library above.
        if !node.is_primary {
            self.check(node.params);
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.check(function.params());
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::FuncExpr(function) = node {
            self.check(function.params());
        }
        StructuralTraversal::Continue
    }
}
}
