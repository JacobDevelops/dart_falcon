//! Require `Expanded` and `Flexible` to sit directly inside a flex widget.
//!
//! Flags an `Expanded` or `Flexible` passed as the `child:` of a widget that is
//! not a `Row`, `Column`, or `Flex`. These widgets report their flex factor to a
//! surrounding flex container through the render tree; under any other parent
//! they have no flex parent to negotiate with and throw a layout error at
//! runtime. Put them in the `children:` of a `Row`, `Column`, or `Flex`, or
//! remove the wrapper. The rule reports only when the illegal parent is directly
//! visible at the construction site.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct ProperExpandedAndFlexible;

impl Rule for ProperExpandedAndFlexible {
    fn name(&self) -> &'static str {
        "proper-expanded-and-flexible"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        // Only flag when we can positively see an illegal parent context: an
        // Expanded/Flexible passed as the `child:` of a non-Flex widget. A Flex
        // widget (Row/Column/Flex) receives its children via `children:`, so an
        // Expanded there is legal and not considered here.
        if let Some((parent, args, _)) = widget_construction(node)
            && !matches!(parent, "Row" | "Column" | "Flex")
        {
            for named in &args.named {
                if named.name.name == "child"
                    && let Some((child, _, child_span)) = widget_construction(&named.value)
                    && matches!(child, "Expanded" | "Flexible")
                {
                    self.diags.push(Diagnostic::new(
                        "proper-expanded-and-flexible",
                        Severity::Warning,
                        "Expanded and Flexible must be direct children of a Row, Column, or Flex.",
                        self.file.clone(),
                        DiagSpan {
                            start: child_span.start,
                            end: child_span.end,
                        },
                    ));
                }
            }
        }
        walk_expr(self, node);
    }
}

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
