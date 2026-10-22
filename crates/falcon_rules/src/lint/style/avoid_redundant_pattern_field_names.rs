//! Flags redundant field names in object and record patterns.
//!
//! When a pattern binds a field to a variable of the same name — `Point(x: x)`
//! or `(name: name)` — the explicit label merely repeats the getter name for no
//! benefit. The shorthand `Point(:x)` / `(:name)` is shorter and equivalent.
//! Only a field whose label matches its bound variable is flagged; a genuine
//! rename such as `Point(x: px)` is left alone.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use falcon_syntax::token::TokenKind;

#[derive(Default)]
pub struct AvoidRedundantPatternFieldNames;

impl Rule for AvoidRedundantPatternFieldNames {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    ctx: &'s AnalyzeContext<'s>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        match node {
            Pattern::Object(obj) => {
                for field in obj.fields() {
                    if field.source_name().is_some() && is_redundant_field(self.ctx, field.span()) {
                        self.diags.push(self.ctx.diagnostic(
                            "Remove the redundant field name from this pattern and use shorthand such as `:x`.",
                            field.span(),
                        ));
                    }
                }
            }
            Pattern::Record(rec) => {
                for field in rec.fields() {
                    if field.source_name().is_some() && is_redundant_field(self.ctx, field.span()) {
                        self.diags.push(self.ctx.diagnostic(
                            "Remove the redundant field name from this pattern and use shorthand such as `:x`.",
                            field.span(),
                        ));
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}

fn is_redundant_field(ctx: &AnalyzeContext, span: Span) -> bool {
    let mut tokens = ctx
        .tree()
        .tokens_in(ctx.source(), span)
        .filter(|token| !matches!(token.kind(), TokenKind::Whitespace | TokenKind::Newline));
    let (Some(left), Some(colon), Some(right)) = (tokens.next(), tokens.next(), tokens.next())
    else {
        return false;
    };
    colon.kind() == TokenKind::Colon
        && tokens.next().is_none()
        && left.text() == right.text()
        && left.kind().is_identifier_like()
        && left.text().is_ascii()
        && !left.text().contains('$')
}
