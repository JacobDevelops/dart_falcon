//! Flags an `Expanded` wrapping an empty `SizedBox` or `Container` that should be a `Spacer`.
//!
//! Flutter's `Spacer` is exactly an `Expanded` whose child is an empty box, so
//! `Expanded(child: SizedBox())` or `Expanded(child: Container())` is a verbose
//! spelling of `Spacer()`. Using `Spacer` states the intent — reserve flexible
//! empty space in a `Row`, `Column`, or `Flex` — and drops a layer of nesting.
//! The rule matches an `Expanded` whose `child:` constructs a `Container` or
//! `SizedBox` with no `child:` argument of its own; other arguments such as
//! `color` do not disqualify the match, only a nested `child` does. Replace it
//! with `Spacer()`, moving any `flex:` across.
//! Checks constructions throughout the program, including initializers and closures.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UseSpacerAsExpandedChild;

impl Rule for UseSpacerAsExpandedChild {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, expr: &Expr) -> StructuralTraversal {
        if is_unqualified_construction(expr)
            && let Some((name, args, span)) = crate::lint::flutter::widget_construction(expr)
        {
            check_expanded(name, args, span, &mut self.diags, self.ctx);
        }
        StructuralTraversal::Continue
    }
}
}

fn check_expanded(
    name: &str,
    args: &ArgList,
    span: &Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if name != "Expanded" {
        return;
    }
    for named in &args.named {
        if named.name.name == "child" && is_empty_spacer_widget(&named.value) {
            diags.push(ctx.diagnostic(
                "use-spacer-as-expanded-child",
                "Use Spacer() instead of an Expanded with an empty Container or SizedBox child.",
                *span,
            ));
            return;
        }
    }
}

fn is_empty_spacer_widget(expr: &Expr) -> bool {
    if !is_unqualified_construction(expr) {
        return false;
    }
    let Some((name, args, _)) = crate::lint::flutter::widget_construction(expr) else {
        return false;
    };
    if name != "Container" && name != "SizedBox" {
        return false;
    }
    !args.named.iter().any(|n| n.name.name == "child")
}

fn is_unqualified_construction(expr: &Expr) -> bool {
    !matches!(expr.unparenthesized(), Expr::New { dart_type: DartType::Named(named), .. }
        if named.segments.len() != 1)
}
