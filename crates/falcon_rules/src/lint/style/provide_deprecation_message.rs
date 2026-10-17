//! Flags `@deprecated` and `@Deprecated()` annotations that carry no message.
//!
//! A deprecation is only useful if it tells callers what to do instead — which
//! replacement to adopt, or since when the API is going away. The bare
//! `@deprecated` constant can never carry that guidance, and `@Deprecated()`
//! with no argument (or an empty/whitespace-only string) is just as unhelpful,
//! so both should be replaced with `@Deprecated("message")`. A `@Deprecated`
//! whose positional argument is any non-empty expression is accepted. The rule
//! also inspects field-level annotations, which the generic AST walk would
//! otherwise skip.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct ProvideDeprecationMessage;

impl Rule for ProvideDeprecationMessage {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_annotation(&mut self, node: Annotation<'a>) -> StructuralTraversal {
        let Some(last) = node.name().last() else {
            return StructuralTraversal::SkipChildren;
        };
        let flagged = match last.name() {
            // `@deprecated`: the predefined constant, never carries a message.
            "deprecated" => node.args().is_none(),
            // `@Deprecated(...)`: flag a missing argument, or a single positional
            // message that is an empty/whitespace-only string literal.
            "Deprecated" => match node.args() {
                None => true,
                Some(args) => match args.positional().first().map(Expr::unparenthesized) {
                    None => true,
                    Some(Expr::StringLit(s)) => s.value().trim().is_empty(),
                    Some(_) => false,
                },
            },
            _ => false,
        };
        if flagged {
            self.diags.push(self.ctx.diagnostic(
                "provide-deprecation-message",
                "Provide a deprecation message, via @Deprecated(\"message\").",
                node.span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }
}
}
