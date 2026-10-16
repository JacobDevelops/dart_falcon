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

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Feature;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::{Visitor, walk_class_member, walk_formal_param};

const RULE: &str = "avoid-final-parameters";
const MESSAGE: &str = "Parameters should not be marked as 'final'.";

#[derive(Default)]
pub struct AvoidFinalParameters;

impl Rule for AvoidFinalParameters {
    fn prepare<'a>(
        &self,
        program: &'a Program,
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
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
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

impl Visitor for Collector<'_, '_> {
    fn visit_formal_param(&mut self, node: &FormalParam) {
        if node.is_final && node.function_params.is_none() && node.initializing.is_none() {
            self.report_final_in(node.span.start, node.name.span.start);
        }
        walk_formal_param(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Setter(setter) = node {
            self.check_setter(setter);
        }
        walk_class_member(self, node);
    }
}
