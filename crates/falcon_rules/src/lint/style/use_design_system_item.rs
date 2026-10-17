//! Flags construction of a widget or type that should be replaced by its design-system equivalent.
//!
//! Teams building on a Flutter design system often want to steer away from raw
//! framework widgets (a bare `Container`, `Text`, or `ElevatedButton`) toward
//! their own wrappers, which enforce spacing, theming, and accessibility. This
//! rule flags a construction whose class name matches a configured entry, in
//! either constructor form — implicit `Foo(...)`, explicit `new`/`const`, and
//! generic `Foo<T>(...)`. It deliberately ignores named-constructor access like
//! `Foo.named(...)` so static getters and factory helpers are not mistaken for
//! plain construction. The rule is entirely config-driven: with no configured
//! items it is a no-op, so it flags nothing until a design system is described.
//!
//! ## Options
//!
//! `items` (list of objects, default: `[]`) — each entry maps a `class_name`
//! (required) to steer away from, with an optional `use_instead` naming the
//! design-system replacement to suggest in the message.
//!
//! Checks constructions throughout the program, including initializers and annotations.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseDesignSystemItem;

impl Rule for UseDesignSystemItem {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let items = parse_config_items(ctx);
        if items.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }

        RulePreparation::Structural(Box::new(Collector {
            ctx,
            items,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    items: Vec<(&'a str, Option<&'a str>)>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, expr: Expr<'a>) -> StructuralTraversal {
        let construction = match expr.unparenthesized() {
            Expr::New(_) => crate::lint::flutter::widget_construction(expr)
                .map(|(name, _, span)| (name, span)),
            Expr::Call(call) => {
                callee_base_name(call.callee()).map(|name| (name, call.span()))
            }
            _ => None,
        };
        if let Some((name, span)) = construction {
            emit_for_base(name, span, &mut self.diags, self.ctx, &self.items);
        }
        StructuralTraversal::Continue
    }
}
}

fn parse_config_items<'a>(ctx: &'a AnalyzeContext<'_>) -> Vec<(&'a str, Option<&'a str>)> {
    let Some(items_array) = crate::lint::options::for_rule(ctx, "use-design-system-item")
        .and_then(|opts| opts.get("items"))
        .and_then(|v| v.as_array())
    else {
        return Vec::new();
    };

    items_array
        .iter()
        .filter_map(|item| {
            let class_name = item.get("class_name").and_then(|v| v.as_str())?;
            let use_instead = item.get("use_instead").and_then(|v| v.as_str());
            Some((class_name, use_instead))
        })
        .collect()
}

fn callee_base_name(mut callee: Expr<'_>) -> Option<&str> {
    while let Expr::Call(call) = callee.unparenthesized() {
        callee = call.callee();
    }
    match callee.unparenthesized() {
        Expr::Ident(id) => Some(id.name()),
        _ => None,
    }
}

fn emit_for_base(
    base_name: &str,
    span: Span,
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    items: &[(&str, Option<&str>)],
) {
    for (class_name, use_instead) in items {
        if base_name == *class_name {
            let message = if let Some(replacement) = use_instead {
                format!(
                    "Use '{}' from the design system instead of '{}'.",
                    replacement, class_name
                )
            } else {
                format!(
                    "Use the design system equivalent instead of '{}'.",
                    class_name
                )
            };

            diags.push(ctx.diagnostic("use-design-system-item", message, span));
            break;
        }
    }
}
