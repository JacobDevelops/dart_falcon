//! Requires explicit arrange, act, and assert sections in test callbacks.
//!
//! Checks inline `test` and `testWidgets` callbacks in `_test.dart` files when
//! the registration resolves to a test-framework package. Section markers are
//! standalone line or block comments, matched case-insensitively, immediately
//! before a top-level callback statement. Comments inside nested statements or
//! local functions do not structure the enclosing test. Each act section needs
//! an immediately preceding arrange section, and each assert section needs an
//! immediately preceding act section. Missing sections and repeated rounds are
//! reported; test groups and callback tear-offs are not inspected.
//!
//! ## Options
//!
//! `use-alternative-comments` (default: `false`) replaces `arrange`, `act`, and
//! `assert` with `given`, `when`, and `then`.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::{options, support::test_assertions};

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Use `given`, `when`, and `then` section comments instead of arrange, act, and assert.
    use_alternative_comments: bool,
}

options::impl_options!(Options);

/// Requires ordered arrange/act/assert comments in framework test callbacks.
///
/// Recognizes resolved `test` and `testWidgets` registrations in `_test.dart`
/// files. Section comments precede top-level callback statements; nested helper
/// comments cannot satisfy the enclosing test. Missing sections, out-of-order
/// predecessors, and repeated arrange/act rounds produce diagnostics.
///
/// ## Options
///
/// `use-alternative-comments` defaults to `false`. Set it to `true` to require
/// given/when/then instead. Markers are exact, case-insensitive standalone line
/// or block comments. Groups and callback tear-offs are not inspected.
#[derive(Default)]
pub struct PreferTestStructure;

impl Rule for PreferTestStructure {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !test_assertions::enabled(program, ctx) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let sections = if options::get::<Options>(ctx, Self::NAME).use_alternative_comments {
            ["given", "when", "then"]
        } else {
            ["arrange", "act", "assert"]
        };
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            sections,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    sections: [&'static str; 3],
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, callback: FuncExpr<'_>) {
        let FunctionBody::Block(block) = callback.body() else {
            self.missing_sections(callback.span());
            return;
        };
        let mut previous = None;
        let mut start = block.span().start;
        let mut arranged = false;
        for statement in block.stmts() {
            for token in self
                .ctx
                .tree()
                .tokens_in(self.ctx.source(), Span::new(start, statement.span().start))
            {
                let text = token.text();
                let comment = text.strip_prefix("//").or_else(|| {
                    text.strip_prefix("/*")
                        .and_then(|text| text.strip_suffix("*/"))
                });
                let Some(section) = comment.and_then(|comment| {
                    self.sections
                        .iter()
                        .position(|section| comment.trim().eq_ignore_ascii_case(section))
                }) else {
                    continue;
                };
                let preceding_line = self.ctx.source()[..token.offset()]
                    .rsplit('\n')
                    .next()
                    .unwrap_or_default();
                if !preceding_line.trim().is_empty() {
                    continue;
                }
                if section > 0 && previous != Some(section - 1) {
                    self.diagnostics.push(self.ctx.diagnostic(
                        format!(
                            "This test is missing the `{}` section before this `{}` section. Try refactoring this test.",
                            self.sections[section - 1], self.sections[section],
                        ),
                        statement.span(),
                    ));
                } else if section == 0 && arranged {
                    self.diagnostics.push(self.ctx.diagnostic(
                        "Avoid multiple rounds of test sections. Try splitting this test.",
                        statement.span(),
                    ));
                }
                previous = Some(section);
                arranged |= section == 0;
            }
            start = statement.span().end;
        }
        match previous {
            None => self.missing_sections(callback.span()),
            Some(section) if section < 2 => {
                self.diagnostics.push(self.ctx.diagnostic(
                    format!(
                        "This test is missing the `{}` section after its `{}` section. Try refactoring this test.",
                        self.sections[section + 1], self.sections[section],
                    ),
                    callback.span(),
                ));
            }
            _ => {}
        }
    }

    fn missing_sections(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(
            format!(
                "Separate this test into `{}`, `{}`, and `{}` sections with comments.",
                self.sections[0], self.sections[1], self.sections[2],
            ),
            span,
        ));
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        if let Some(("test" | "testWidgets", package)) = test_assertions::package_call(call, state)
            && test_assertions::is_framework(package)
            && let Some(Expr::FuncExpr(callback)) =
                call.args().positional().nth(1).map(Expr::unparenthesized)
        {
            self.check(callback);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
