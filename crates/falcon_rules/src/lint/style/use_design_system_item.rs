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
//! `items` (list of objects, default: `[]`) — each entry maps a `class-name`
//! (required) to steer away from, with an optional `use-instead` naming the
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

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Class names whose construction should be replaced with a design-system equivalent.
    items: Vec<Item>,
}

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct Item {
    /// Class name of the construction to flag.
    class_name: String,
    /// Suggested design-system replacement name, shown in the diagnostic when provided.
    use_instead: Option<String>,
}

crate::lint::options::impl_options!(Options);

impl Rule for UseDesignSystemItem {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let items = &crate::lint::options::get::<Options>(ctx, Self::NAME).items;
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
    items: &'a [Item],
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
            emit_for_base(name, span, &mut self.diags, self.ctx, self.items);
        }
        StructuralTraversal::Continue
    }
}
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
    items: &[Item],
) {
    for item in items {
        let class_name = &item.class_name;
        if base_name == class_name {
            let message = if let Some(replacement) = &item.use_instead {
                format!(
                    "Use `{replacement}` from the design system instead of `{class_name}`."
                )
            } else {
                format!(
                    "Use the design system equivalent instead of `{class_name}`."
                )
            };

            diags.push(ctx.diagnostic(message, span));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{AnalyzeContext, ParsedFile, RuleAnalysis};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::{path::Path, sync::Arc};

    #[test]
    fn use_design_system_item_fires_only_when_configured() {
        let src = r"
    import 'package:flutter/material.dart';
    
    Widget build(BuildContext context) {
      return Container(child: Text('hi'));
    }
    ";
        let falcon_dart_parser::ParseResult { tree, .. } = parse(src);
        let parsed = Arc::new(ParsedFile {
            source: src.into(),
            tree,
        });
        let program = parsed.program();
        let rule = UseDesignSystemItem;
    
        // Without configuration the rule is a no-op.
        let default_cfg = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("t.dart"), &default_cfg, &parsed);
        assert!(
            rule.analyze(UseDesignSystemItem::NAME, program, &ctx)
                .is_empty(),
            "rule must be a no-op without configuration"
        );
    
        // With a configured item it flags the disallowed construction and names the replacement.
        let configured: FalconConfig = serde_json::from_value(serde_json::json!({
            "linter": {
                "rules": {
                    "style": {
                        "use-design-system-item": {
                            "level": "warn",
                            "options": {
                                "items": [
                                    { "class-name": "Container", "use-instead": "AppContainer" }
                                ]
                            }
                        }
                    }
                }
            }
        }))
        .expect("valid config");
        let ctx = AnalyzeContext::new(Path::new("t.dart"), &configured, &parsed);
        let diags = rule.analyze(UseDesignSystemItem::NAME, program, &ctx);
        assert_eq!(
            diags.len(),
            1,
            "expected exactly one diagnostic, got {diags:?}"
        );
        assert_eq!(diags[0].rule, "use-design-system-item");
        assert!(
            diags[0].message.contains("AppContainer") && diags[0].message.contains("Container"),
            "message should name the replacement: {}",
            diags[0].message
        );
    
        // Static calls are not constructions.
        let static_src = "Widget f(BuildContext c) => Container.of(c);";
        let falcon_dart_parser::ParseResult { tree, .. } = parse(static_src);
        let parsed = Arc::new(ParsedFile {
            source: static_src.into(),
            tree,
        });
        let static_program = parsed.program();
        let ctx = AnalyzeContext::new(Path::new("t.dart"), &configured, &parsed);
        assert!(
            rule.analyze(UseDesignSystemItem::NAME, static_program, &ctx)
                .is_empty(),
            "static access `Container.of(...)` must not be flagged"
        );
    }
}
