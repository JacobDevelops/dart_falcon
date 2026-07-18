//! Flags default `List()`/`Map()`/`Set()`/`LinkedHashMap()`/`LinkedHashSet()`
//! constructor invocations that a collection literal (`[]`/`{}`) would express.
//! Adopted from package:lints `prefer_collection_literals`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferCollectionLiterals;

/// Constructors whose default invocation is expressible as a literal.
const NAMES: [&str; 5] = ["List", "Map", "Set", "LinkedHashMap", "LinkedHashSet"];

impl Rule for PreferCollectionLiterals {
    fn name(&self) -> &'static str {
        "prefer-collection-literals"
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
            "prefer-collection-literals",
            Severity::Warning,
            "Use a collection literal instead of a constructor invocation.",
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
        if let Some(span) = default_collection_ctor(node) {
            self.push(span);
        }
        walk_expr(self, node);
    }
}

/// Span of a *default* constructor invocation of a known collection type with no
/// arguments (`List()`, `Map<K, V>()`, `LinkedHashSet.new()`, `new Set()`), or
/// `None` for named constructors (`List.filled`, `Map.of`) and any invocation
/// that carries arguments.
fn default_collection_ctor(expr: &Expr) -> Option<&Span> {
    match expr {
        Expr::Call {
            callee, args, span, ..
        } => {
            if !args.positional.is_empty() || !args.named.is_empty() {
                return None;
            }
            match &**callee {
                Expr::Ident(id) if NAMES.contains(&id.name.as_str()) => Some(span),
                Expr::Field { object, field, .. } if field.name == "new" => match &**object {
                    Expr::Ident(id) if NAMES.contains(&id.name.as_str()) => Some(span),
                    _ => None,
                },
                _ => None,
            }
        }
        Expr::New {
            dart_type: DartType::Named(nt),
            constructor_name,
            args,
            span,
            ..
        } => {
            if !args.positional.is_empty() || !args.named.is_empty() {
                return None;
            }
            let base = nt.segments.last()?;
            if !NAMES.contains(&base.name.as_str()) {
                return None;
            }
            match constructor_name {
                None => Some(span),
                Some(c) if c.name == "new" => Some(span),
                _ => None,
            }
        }
        _ => None,
    }
}
