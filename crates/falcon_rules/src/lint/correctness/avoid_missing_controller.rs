//! Require Flutter text fields to expose their updated value.
//!
//! `TextField`, `TextFormField`, `CupertinoTextField`, and `EditableText` need a non-null
//! controller or a value callback supported by that API. Explicit readOnly:true
//! and enabled:false fields are display-only and skipped. Dynamic expressions
//! are accepted; unrelated classes and shadowed constructors are ignored.
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMissingController;

impl Rule for AvoidMissingController {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diags)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if !matches!(node, Expr::Call(_) | Expr::New(_)) {
            return;
        }
        let Some((identity, _, args)) =
            crate::lint::flutter::scoped_constructor_invocation(node, state)
        else {
            return;
        };
        let Some(index) = self.ctx.identities() else {
            return;
        };
        let form = index.is_package_declaration(&identity, "flutter", "TextFormField");
        if !form
            && !["TextField", "CupertinoTextField", "EditableText"]
                .iter()
                .any(|name| index.is_package_declaration(&identity, "flutter", name))
        {
            return;
        }
        if args.named().any(|argument| {
            matches!(argument.value().unparenthesized(), Expr::BoolLit(value)
                if (argument.name().name() == "readOnly" && value.value())
                    || (argument.name().name() == "enabled" && !value.value()))
        }) {
            return;
        }
        let observed = args.named().any(|argument| {
            let name = argument.name().name();
            (matches!(name, "controller" | "onChanged")
                || (form && matches!(name, "onSaved" | "onFieldSubmitted"))
                || (!form && name == "onSubmitted"))
                && !matches!(argument.value().unparenthesized(), Expr::NullLit(_))
        });
        if !observed {
            self.diags.push(self.ctx.diagnostic("Provide a controller or a callback that receives the updated field value.", crate::lint::flutter::constructor_span(node)));
        }
    }
}
