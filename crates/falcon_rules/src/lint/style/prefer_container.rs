//! Suggests collapsing compatible nested layout widgets into one `Container`.
//!
//! Matches at least three Flutter widgets in Container's outside-in order:
//! `Transform`, margin `Padding`, `ConstrainedBox`, `ColoredBox`, inner `Padding`,
//! and `Align`. Stages may be omitted, but not repeated or reordered. Only the
//! unnamed constructors and their directly equivalent arguments qualify; inner
//! keys, alignment size factors, transform origins and hit-test overrides are
//! excluded. Decoration and clipping are excluded because their padding and
//! clipping behavior cannot generally be reproduced by a Container. The final
//! child must be provably non-null to preserve Container's empty-child behavior.
//! Constant contexts are excluded because Container has no const constructor.
//! A concrete source-widget expected type is also excluded to preserve typing.
//!
//! ## Options
//!
//! `min-sequence` (integer, default: 3) sets the minimum chain length, with a
//! minimum effective value of 3 to avoid competing with single-widget rules.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferContainer;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Minimum number of compatible nested layout widgets required for a diagnostic; values below three are treated as three.
    #[serde(deserialize_with = "falcon_config::integer")]
    min_sequence: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self { min_sequence: 3 }
    }
}

crate::lint::options::impl_options!(Options);

impl Rule for PreferContainer {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            contexts: ctx.const_contexts(program),
            minimum: crate::lint::options::get::<Options>(ctx, Self::NAME)
                .min_sequence
                .max(3),
            covered: [usize::MAX; 6],
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    contexts: &'a falcon_analyze::ConstContexts,
    minimum: usize,
    covered: [usize; 6],
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let node = node.unparenthesized();
        if self.covered.contains(&node.span().start)
            || self.contexts.contains(&node.span())
            || matches!(node, Expr::New(new) if new.is_const())
        {
            return;
        }
        if let Some(falcon_analyze::ResolvedType::Interface { identity, .. }) = state.expected()
            && [
                "Transform",
                "Padding",
                "ConstrainedBox",
                "ColoredBox",
                "Align",
            ]
            .contains(&identity.name())
            && self.ctx.identities().is_some_and(|index| {
                index.is_package_declaration(identity, "flutter", identity.name())
            })
        {
            return;
        }
        let mut expression = node;
        let mut previous = None;
        let mut count = 0;
        let mut starts = [usize::MAX; 6];
        while let Some((stage, child)) =
            layout_stage(expression, state, self.ctx, previous, count == 0)
        {
            if previous.is_some_and(|previous| stage <= previous) {
                break;
            }
            starts[count] = expression.span().start;
            count += 1;
            previous = Some(stage);
            expression = child.unparenthesized();
            if count == starts.len() {
                break;
            }
        }
        if count >= self.minimum
            && crate::lint::semantic_type_operations::nullability(
                &state.infer(expression),
                &state.signatures,
                self.ctx.types(),
            ) == falcon_analyze::Truth::No
        {
            self.covered = starts;
            self.diags.push(self.ctx.diagnostic("Replace this layout widget sequence with a single `Container`.", crate::lint::flutter::constructor_span(node)));
        }
    }
}

fn layout_stage<'a>(
    expression: Expr<'a>,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext<'a>,
    previous: Option<u8>,
    outermost: bool,
) -> Option<(u8, Expr<'a>)> {
    let (identity, "new", args) =
        crate::lint::flutter::scoped_constructor_invocation(expression, state)?
    else {
        return None;
    };
    let index = ctx.identities()?;
    let (stage, property, extra) =
        if index.is_package_declaration(&identity, "flutter", "Transform") {
            (0, "transform", Some("alignment"))
        } else if index.is_package_declaration(&identity, "flutter", "Padding") {
            (
                if previous.is_some_and(|stage| stage >= 1) {
                    4
                } else {
                    1
                },
                "padding",
                None,
            )
        } else if index.is_package_declaration(&identity, "flutter", "ConstrainedBox") {
            (2, "constraints", None)
        } else if index.is_package_declaration(&identity, "flutter", "ColoredBox") {
            (3, "color", None)
        } else if index.is_package_declaration(&identity, "flutter", "Align") {
            (5, "alignment", None)
        } else {
            return None;
        };
    if !args.positional().is_empty() {
        return None;
    }
    let mut child = None;
    let mut has_property = false;
    for argument in args.named() {
        let name = argument.name().name();
        if name == "child" {
            if matches!(argument.value().unparenthesized(), Expr::NullLit(_)) {
                return None;
            }
            child = Some(argument.value());
        } else if name == property {
            if matches!(argument.value().unparenthesized(), Expr::NullLit(_)) {
                return None;
            }
            has_property = true;
        } else if !(extra == Some(name) || outermost && name == "key") {
            return None;
        }
    }
    // Align's default alignment is also exactly Container's explicit center.
    (has_property || stage == 5).then_some((stage, child?))
}

#[cfg(test)]
mod tests {
    use super::PreferContainer;
    use falcon_config::FalconConfig;
    use std::fs;

    #[test]
    fn respects_minimum_sequence_and_reports_only_the_outermost_chain() {
        let source = r"
import 'package:flutter/widgets.dart';
Widget build(Widget child, dynamic matrix) =>
  Transform(transform: matrix, child: Padding(padding: EdgeInsets.all(4),
    child: ConstrainedBox(constraints: BoxConstraints.tightFor(width: 10),
      child: Align(child: child))));
";
        let project = tempfile::tempdir().unwrap();
        let root = project.path();
        fs::create_dir_all(root.join("flutter/lib")).unwrap();
        fs::write(
            root.join("pubspec.yaml"),
            "name: app\ndependencies:\n  flutter:\n    path: flutter\n",
        )
        .unwrap();
        fs::write(root.join("flutter/pubspec.yaml"), "name: flutter\n").unwrap();
        fs::write(
            root.join("flutter/lib/widgets.dart"),
            "class Widget {}\n\
             class Transform extends Widget { Transform({dynamic transform, Widget? child}); }\n\
             class Padding extends Widget { Padding({dynamic padding, Widget? child}); }\n\
             class ConstrainedBox extends Widget { ConstrainedBox({dynamic constraints, Widget? child}); }\n\
             class Align extends Widget { Align({Widget? child}); }\n\
             class EdgeInsets { EdgeInsets.all(int value); }\n\
             class BoxConstraints { BoxConstraints.tightFor({int? width}); }\n",
        )
        .unwrap();
        let path = root.join("test.dart");
        fs::write(&path, source).unwrap();
        for (minimum, count) in [(3, 1), (4, 1), (5, 0)] {
            let config: FalconConfig = serde_json::from_value(serde_json::json!({
                "linter": { "rules": { "style": {
                    "prefer-container": {
                        "level": "warn",
                        "options": { "min-sequence": minimum }
                    }
                }}}
            }))
            .unwrap();
            let diagnostics = crate::inference::analyze(PreferContainer::NAME, &PreferContainer, &path, source, &config);
            assert_eq!(diagnostics.len(), count, "minimum {minimum}");
            if let Some(diagnostic) = diagnostics.first() {
                assert_eq!(
                    &source[diagnostic.span.start..diagnostic.span.end],
                    "Transform"
                );
            }
        }
    }
}
