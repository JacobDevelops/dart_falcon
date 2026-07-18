//! Flags `Map.fromIterable(x, key: ..., value: ...)`, which a map literal with a
//! `for` element expresses more directly. Adopted from package:lints
//! `prefer_for_elements_to_map_fromIterable`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferForElementsToMapFromIterable;

impl Rule for PreferForElementsToMapFromIterable {
    fn name(&self) -> &'static str {
        "prefer-for-elements-to-map-from-iterable"
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

impl Collector {
    fn push(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "prefer-for-elements-to-map-from-iterable",
            Severity::Warning,
            "Use a map literal with a 'for' element instead of 'Map.fromIterable'.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            // `Map.fromIterable(...)` — parsed as a call on a `Map.fromIterable`
            // member reference.
            Expr::Call { callee, args, .. } => {
                if let Expr::Field { object, field, .. } = &**callee
                    && let Expr::Ident(id) = &**object
                    && id.name == "Map"
                    && field.name == "fromIterable"
                    && !args.positional.is_empty()
                    && has_named(args, "key")
                    && has_named(args, "value")
                {
                    self.push(&field.span);
                }
            }
            // `new Map.fromIterable(...)`.
            Expr::New {
                dart_type: DartType::Named(nt),
                constructor_name: Some(ctor),
                args,
                span,
                ..
            } if nt.segments.last().map(|s| s.name.as_str()) == Some("Map")
                && ctor.name == "fromIterable"
                && !args.positional.is_empty()
                && has_named(args, "key")
                && has_named(args, "value") =>
            {
                self.push(span);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

fn has_named(args: &ArgList, name: &str) -> bool {
    args.named.iter().any(|n| n.name.name == name)
}
