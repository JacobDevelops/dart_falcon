//! Flags `child`/`children` not being the last argument of a widget. Ported from package:lints `sort_child_properties_last`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct SortChildPropertiesLast;

impl Rule for SortChildPropertiesLast {
    fn name(&self) -> &'static str {
        "sort-child-properties-last"
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
        if let Some((name, args)) = widget_construction(node)
            // Widget constructors are PascalCase; requiring that keeps this off
            // ordinary function calls that happen to take a `child:` argument.
            && name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && !args.named.is_empty()
        {
            let last = args.named.len() - 1;
            for (i, named) in args.named.iter().enumerate() {
                if matches!(named.name.name.as_str(), "child" | "children") && i != last {
                    self.diags.push(Diagnostic::new(
                        "sort-child-properties-last",
                        Severity::Warning,
                        "The child/children argument should be last in a widget constructor call.",
                        self.file.clone(),
                        DiagSpan {
                            start: named.span.start,
                            end: named.span.end,
                        },
                    ));
                }
            }
        }
        walk_expr(self, node);
    }
}

/// Resolve a widget construction expressed as either an implicit call
/// (`Foo(...)`) or an explicit `new`/`const` (`Foo(...)`).
fn widget_construction(expr: &Expr) -> Option<(&str, &ArgList)> {
    match expr {
        Expr::New {
            dart_type: DartType::Named(nt),
            args,
            ..
        } => nt.segments.last().map(|s| (s.name.as_str(), args)),
        Expr::Call { callee, args, .. } => {
            if let Expr::Ident(id) = callee.as_ref() {
                Some((id.name.as_str(), args))
            } else {
                None
            }
        }
        _ => None,
    }
}
