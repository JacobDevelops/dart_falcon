//! Flags parameters declared `final`.
//!
//! Parameters are rarely reassigned, and marking them `final` adds noise to
//! every signature without telling readers much. The `final` keyword is
//! reported on any parameter of a function, method, constructor, setter,
//! function literal, or function-typed parameter:
//!
//! ```dart
//! void f(final int x) {}        // flagged
//! void g(int x) {}              // fine
//! ```
//!
//! Initializing formals (`final this.x`, `final super.x`) and function-typed
//! parameters (`final void cb()`) are not reported, and `Function(...)` types
//! cannot declare `final` parameters at all. From Dart 3.13, where `final`
//! marks a declaring primary-constructor parameter, the rule is inactive.
//!
//! Adopted from the official `avoid_final_parameters` lint.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Feature;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;

const RULE: &str = "avoid-final-parameters";
const MESSAGE: &str = "Parameters should not be marked as 'final'.";

#[derive(Default)]
pub struct AvoidFinalParameters;

impl Rule for AvoidFinalParameters {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        // With primary constructors `final` declares a field, not a final
        // parameter; the official rule does not run.
        if ctx
            .language_version()
            .supports(Feature::PrimaryConstructors)
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    /// Reports the first `final` token in `[start, end)`.
    fn report_final_in(&mut self, start: usize, end: usize) {
        if let Some(token) = self
            .ctx
            .syntax
            .keywords_in(Span::new(start, end), TokenKind::Final)
            .next()
        {
            self.diags.push(self.ctx.diagnostic(
                RULE,
                MESSAGE,
                Span::new(token.offset(), token.offset() + token.len()),
            ));
        }
    }

    /// A class setter's parameter is not modelled as a formal parameter, so
    /// its `final` is found in the tokens of `set name(...)`.
    fn check_setter(&mut self, setter: &SetterDecl) {
        let mut tokens = self
            .ctx
            .syntax
            .tokens_in(Span::new(setter.name.span.end, setter.span.end))
            .filter(|token| !token.is_trivia());
        if tokens.next().map(|token| token.kind) != Some(TokenKind::LParen) {
            return;
        }
        let mut final_span = None;
        for token in tokens.take_while(|token| token.kind != TokenKind::RParen) {
            if token.kind == TokenKind::LParen {
                return;
            }
            if token.kind == TokenKind::Final && final_span.is_none() {
                final_span = Some(Span::new(token.offset(), token.offset() + token.len()));
            }
        }
        if let Some(span) = final_span {
            self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_formal_param(&mut self, node: &FormalParam) -> StructuralTraversal {
        if node.is_final && node.function_params.is_none() && node.initializing.is_none() {
            self.report_final_in(node.span.start, node.name.span.start);
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Setter(setter) = node {
            self.check_setter(setter);
        }
        StructuralTraversal::Continue
    }
}
}
