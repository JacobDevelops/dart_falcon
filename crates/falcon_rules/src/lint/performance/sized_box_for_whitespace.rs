//! Flags a `Container` used only to add fixed `width`/`height` whitespace.
//!
//! A `Container` sized only by `width`/`height` (optionally with a `child` or
//! `key`) does nothing a `SizedBox` cannot, and `SizedBox` has a const
//! constructor, so it is cheaper and clearer for spacing. The rule fires when a
//! `Container` has no positional arguments, at least one of `width`/`height`,
//! and no named arguments other than `width`, `height`, `child`, or `key`.
//! The diagnostic covers the constructor name, excluding `new` and arguments.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct SizedBoxForWhitespace;

impl Rule for SizedBoxForWhitespace {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((name, args, _)) = crate::lint::flutter::widget_construction(node)
        && name == "Container"
        && args.positional.is_empty()
        // Only sizing/pass-through args, and at least one dimension set: this
        // Container adds whitespace only, so a SizedBox expresses it directly.
        && args
            .named
            .iter()
            .any(|n| matches!(n.name.name.as_str(), "width" | "height"))
        && args
            .named
            .iter()
            .all(|n| matches!(n.name.name.as_str(), "width" | "height" | "child" | "key"))
        {
            self.diags.push(self.ctx.diagnostic(
                "sized-box-for-whitespace",
                "Use a SizedBox to add whitespace to a layout instead of a Container.",
                crate::lint::flutter::constructor_span(node),
            ));
        }
        walk_expr(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::Path;

    #[test]
    fn reports_constructor_name_without_keyword_or_arguments() {
        let source = r"
void build() {
  Container(width: 2, child: text);
  new Container(height: 2, child: text);
  new widgets.Container(width: 2, child: text);
}
";
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
        let diagnostics = SizedBoxForWhitespace.analyze(&parsed.program, &ctx);
        assert_eq!(diagnostics.len(), 3);
        for (diagnostic, invocation) in diagnostics.iter().zip([
            "Container(width:",
            "Container(height:",
            "widgets.Container(width:",
        ]) {
            let expected = invocation.split('(').next().unwrap();
            assert_eq!(diagnostic.span.start, source.find(invocation).unwrap());
            assert_eq!(
                &source[diagnostic.span.start..diagnostic.span.end],
                expected
            );
        }
    }
}
