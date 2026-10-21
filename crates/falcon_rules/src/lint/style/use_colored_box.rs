//! Prefers `ColoredBox` over a `Container` that only paints a color behind its child.
//!
//! A `Container` whose only arguments are `color` and `child` (plus an optional
//! `key`) can be replaced by the lighter, const-capable `ColoredBox`. Only
//! invocations that resolve to Flutter's `Container` are considered. Matching
//! the analyzer's nullability-suffix check, a color with a nullable static type
//! is excluded, but `dynamic` and `Null` still qualify. When the type is unknown,
//! a constructor invocation or a Flutter `Colors.<name>` access also qualifies.
//! The diagnostic covers the constructor name, excluding `new` and arguments.
//!
//! ```dart
//! Container(color: Colors.blue, child: child);  // BAD
//! ColoredBox(color: Colors.blue, child: child); // GOOD
//! ```
//!
//! Adopted from the official `use_colored_box` lint.

use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UseColoredBox;

impl Rule for UseColoredBox {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
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
        if let Some((identity, "new", args)) =
            crate::lint::flutter::constructor_invocation(node, &state.model)
            && self.ctx.identities().is_some_and(|index| {
                index.is_package_declaration(&identity, "flutter", "Container")
            })
            && only_colors_child(args, state)
        {
            self.diags.push(self.ctx.diagnostic("Use a 'ColoredBox' rather than a 'Container' with only a 'Color'.", crate::lint::flutter::constructor_span(node)));
        }
    }
}

fn only_colors_child<'a>(args: ArgList<'a>, state: &SemanticState<'a>) -> bool {
    let mut has_child = false;
    let mut has_color = false;
    for argument in args.named() {
        match argument.name().name() {
            "child" => has_child = true,
            "color" if non_nullable_color(argument.value(), state) => has_color = true,
            "key" => {}
            _ => return false,
        }
    }
    args.positional().is_empty() && has_child && has_color
}

/// The analyzer's `staticType.nullabilitySuffix != question`: `dynamic` and
/// `Null` (as for a `null` literal) carry no `?` suffix.
fn non_nullable_color<'a>(color: Expr<'a>, state: &SemanticState<'a>) -> bool {
    match state.infer(color) {
        ResolvedType::Unknown => {}
        ty => {
            return !ty.nullable() || matches!(ty, ResolvedType::Dynamic | ResolvedType::Null);
        }
    }
    if crate::lint::flutter::constructor_invocation(color, &state.model).is_some() {
        return true;
    }
    // Every static member of Flutter's `Colors` is a non-nullable color.
    matches!(
        color.unparenthesized(),
        Expr::Field(field) if !field.is_null_safe()
            && matches!(field.object(), Expr::Ident(prefix)
                if !state.environment.is_bound(prefix.name())
                    && state.model.resolve_name(prefix.name())
                        .is_some_and(|identity| identity.is_package("flutter", "Colors")))
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference as span_test_support;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn reports_constructor_name_including_prefix_but_not_new_or_arguments() {
        let source = r"
import 'package:flutter/widgets.dart';
import 'package:flutter/widgets.dart' as w;
void build(dynamic color, Widget child) {
  Container(color: color, child: child);
  w.Container(color: color, child: child);
  new Container(color: color, child: child);
}
";
        let diagnostics = span_test_support::analyze(
            UseColoredBox::NAME,
            &UseColoredBox,
            Path::new("test.dart"),
            source,
            &FalconConfig::default(),
        );
        assert_eq!(diagnostics.len(), 3);
        for (diagnostic, (invocation, expected)) in diagnostics.iter().zip([
            ("Container(color:", "Container"),
            ("w.Container(color:", "w.Container"),
            ("new Container(color:", "Container"),
        ]) {
            let start =
                source.find(invocation).unwrap() + usize::from(invocation.starts_with("new ")) * 4;
            assert_eq!(diagnostic.span.start, start);
            assert_eq!(
                &source[diagnostic.span.start..diagnostic.span.end],
                expected
            );
        }
    }
}
