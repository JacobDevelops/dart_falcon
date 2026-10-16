//! Flags redundant field names in object and record patterns.
//!
//! When a pattern binds a field to a variable of the same name — `Point(x: x)`
//! or `(name: name)` — the explicit label merely repeats the getter name for no
//! benefit. The shorthand `Point(:x)` / `(:name)` is shorter and equivalent.
//! Only a field whose label matches its bound variable is flagged; a genuine
//! rename such as `Point(x: px)` is left alone.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::{Visitor, walk_pattern};

#[derive(Default)]
pub struct AvoidRedundantPatternFieldNames;

impl Rule for AvoidRedundantPatternFieldNames {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
}

impl Visitor for Collector<'_> {
    fn visit_pattern(&mut self, node: &Pattern) {
        match node {
            Pattern::Object(obj) => {
                for field in &obj.fields {
                    if field.source_name().is_some() && is_redundant_field(self.ctx, field.span) {
                        self.diags.push(self.ctx.diagnostic(
                            "avoid-redundant-pattern-field-names",
                            "Redundant field name in pattern; use the shorthand (e.g. `:x`).",
                            field.span,
                        ));
                    }
                }
            }
            Pattern::Record(rec) => {
                for field in &rec.fields {
                    if field.source_name().is_some() && is_redundant_field(self.ctx, field.span) {
                        self.diags.push(self.ctx.diagnostic(
                            "avoid-redundant-pattern-field-names",
                            "Redundant field name in pattern; use the shorthand (e.g. `:x`).",
                            field.span,
                        ));
                    }
                }
            }
            _ => {}
        }
        walk_pattern(self, node);
    }
}

fn is_redundant_field(ctx: &AnalyzeContext, span: Span) -> bool {
    let mut tokens = ctx
        .syntax
        .tokens_in(span)
        .filter(|token| !matches!(token.kind, TokenKind::Whitespace | TokenKind::Newline));
    let (Some(left), Some(colon), Some(right)) = (tokens.next(), tokens.next(), tokens.next())
    else {
        return false;
    };
    colon.kind == TokenKind::Colon
        && tokens.next().is_none()
        && left.text(ctx.source) == right.text(ctx.source)
        && left.kind.is_identifier_like()
        && left.text(ctx.source).is_ascii()
        && !left.text(ctx.source).contains('$')
}
