//! Flags a `Column`, `Row`, or `Flex` widget built with a single child.
//!
//! A multi-child layout widget that wraps exactly one child adds a layout node
//! and an allocation without changing what is rendered; the child can be used
//! directly, or swapped for `Align`, `Center`, or `Padding` if the axis
//! behavior actually mattered. The rule fires only when the `children` list
//! holds exactly one expression element, and it tolerates partially-parsed Dart
//! 3 collection elements by requiring a complete, `)`-terminated construction.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct AvoidSingleChildColumnOrRow;

impl Rule for AvoidSingleChildColumnOrRow {
    fn name(&self) -> &'static str {
        "avoid-single-child-column-or-row"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            source: ctx.source,
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'s> {
    diags: Vec<Diagnostic>,
    file: String,
    source: &'s str,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((name, args, span)) = widget_construction(node)
            && matches!(name, "Column" | "Row" | "Flex")
            // Guard against partially-parsed constructions (a complete call ends
            // with `)`); Dart 3 pattern collection elements can truncate the node.
            && self.source.get(span.start..span.end).is_some_and(|s| s.trim_end().ends_with(')'))
        {
            for named in &args.named {
                if named.name.name == "children"
                    && let Expr::List { elements, .. } = &named.value
                    && elements.len() == 1
                    && matches!(elements[0], CollectionElement::Expr(_))
                {
                    self.diags.push(Diagnostic::new(
                        "avoid-single-child-column-or-row",
                        Severity::Warning,
                        "Avoid a Column/Row/Flex with a single child; use the child directly.",
                        self.file.clone(),
                        DiagSpan {
                            start: span.start,
                            end: span.end,
                        },
                    ));
                }
            }
        }
        walk_expr(self, node);
    }
}

/// Resolve a widget construction expressed as either an implicit call
/// (`Column(...)`) or an explicit `new`/`const` (`Column(...)`).
fn widget_construction(expr: &Expr) -> Option<(&str, &ArgList, &Span)> {
    match expr {
        Expr::New {
            dart_type: DartType::Named(nt),
            args,
            span,
            ..
        } => nt.segments.last().map(|s| (s.name.as_str(), args, span)),
        Expr::Call {
            callee, args, span, ..
        } => {
            if let Expr::Ident(id) = callee.as_ref() {
                Some((id.name.as_str(), args, span))
            } else {
                None
            }
        }
        _ => None,
    }
}
