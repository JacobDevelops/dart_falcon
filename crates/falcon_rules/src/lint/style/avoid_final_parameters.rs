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

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::Feature;
use falcon_syntax::ast::*;
use falcon_syntax::token::{Token, TokenKind};
use falcon_syntax::visitor::{Visitor, walk_class_member, walk_formal_param};

const RULE: &str = "avoid-final-parameters";
const MESSAGE: &str = "Parameters should not be marked as 'final'.";

#[derive(Default)]
pub struct AvoidFinalParameters;

impl Rule for AvoidFinalParameters {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // With primary constructors `final` declares a field, not a final
        // parameter; the official rule does not run.
        if ctx
            .language_version()
            .supports(Feature::PrimaryConstructors)
        {
            return Vec::new();
        }
        let tokens: Vec<Token> = ctx
            .tokens()
            .iter()
            .copied()
            .filter(|token| !token.is_trivia())
            .collect();
        let mut collector = Collector {
            ctx,
            tokens,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    tokens: Vec<Token>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_> {
    /// Reports the first `final` token in `[start, end)`.
    fn report_final_in(&mut self, start: usize, end: usize) {
        let from = self.tokens.partition_point(|token| token.offset < start);
        if let Some(token) = self.tokens[from..]
            .iter()
            .take_while(|token| token.offset < end)
            .find(|token| token.kind == TokenKind::Final)
        {
            self.diags.push(self.ctx.diagnostic(
                RULE,
                MESSAGE,
                DiagSpan {
                    start: token.offset,
                    end: token.offset + token.len,
                },
            ));
        }
    }

    /// A class setter's parameter is not modelled as a formal parameter, so
    /// its `final` is found in the tokens of `set name(...)`.
    fn check_setter(&mut self, setter: &SetterDecl) {
        let from = self
            .tokens
            .partition_point(|token| token.offset < setter.name.span.end);
        let mut tokens = self.tokens[from..].iter();
        if tokens.next().map(|token| token.kind) != Some(TokenKind::LParen) {
            return;
        }
        let param: Vec<&Token> = tokens
            .take_while(|token| token.kind != TokenKind::RParen)
            .collect();
        let function_typed = param.iter().any(|token| token.kind == TokenKind::LParen);
        if !function_typed && let Some(first) = param.first() {
            let start = first.offset;
            let end = param.last().map_or(start, |token| token.offset + token.len);
            self.report_final_in(start, end);
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
