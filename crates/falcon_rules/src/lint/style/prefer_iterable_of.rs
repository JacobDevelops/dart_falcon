//! Flags `.from` collection constructors in favor of the `.of` constructor.
//!
//! Catches `List.from`, `Set.from`, and `Iterable.from` — whether written as a
//! static call (`List.from(xs)`, `List<int>.from(xs)`) or with `new`. The `.from`
//! constructor takes an `Iterable<dynamic>` and re-types its elements, so a type
//! mismatch surfaces only as a runtime cast failure; the `.of` constructor takes a
//! statically typed `Iterable<E>`, letting the compiler reject bad element types up
//! front. Prefer `.of` unless you specifically need `.from`'s dynamic widening.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program};

pub struct PreferIterableOf;

impl Rule for PreferIterableOf {
    fn name(&self) -> &'static str {
        "prefer-iterable-of"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

fn is_iterable_base(name: &str) -> bool {
    name == "List" || name == "Set" || name == "Iterable"
}

/// True for the `Expr::New` form, `new List.from(...)` / `new Set.from(...)`.
///
/// The parser may represent this two ways depending on how the named constructor binds:
///   * `dart_type = List`, `constructor_name = Some("from")`, or
///   * the `.from` is folded into the qualified type name, giving
///     `dart_type = Named { segments: [.., "List", "from"] }`, `constructor_name = None`.
fn is_from_constructor(dart_type: &DartType, constructor_name: &Option<Identifier>) -> bool {
    let DartType::Named(nt) = dart_type else {
        return false;
    };
    // Case A: `.from` kept as a separate named constructor.
    if let Some(ctor) = constructor_name
        && ctor.name == "from"
        && let Some(last) = nt.segments.last()
    {
        return is_iterable_base(&last.name);
    }
    // Case B: `.from` folded into the qualified type name -> [.., base, "from"].
    let segs = &nt.segments;
    segs.len() >= 2
        && segs.last().is_some_and(|s| s.name == "from")
        && is_iterable_base(&segs[segs.len() - 2].name)
}

/// Resolve the base type name of a receiver expression.
/// `List`            -> `Expr::Ident("List")`
/// `List<int>`       -> `Expr::GenericInstantiation { target: Ident("List"), type_args: [int], .. }`
fn base_type_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Ident(id) => Some(id.name.as_str()),
        Expr::Call { callee, .. } => base_type_name(callee),
        Expr::GenericInstantiation { target, .. } => base_type_name(target),
        _ => None,
    }
}

/// True for `List.from(...)` / `List<int>.from(...)` parsed as `Call(Field(receiver, "from"), args)`.
fn is_from_static_call(callee: &Expr) -> bool {
    if let Expr::Field { object, field, .. } = callee
        && field.name == "from"
        && let Some(base) = base_type_name(object)
    {
        return is_iterable_base(base);
    }
    false
}

fn flag(span: &Span, diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    diags.push(Diagnostic::new(
        "prefer-iterable-of",
        Severity::Warning,
        "Prefer using the 'of' constructor instead of 'from'.",
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::New {
                dart_type,
                constructor_name,
                span,
                ..
            } if is_from_constructor(dart_type, constructor_name) => {
                flag(span, &mut self.diags, self.ctx);
            }
            Expr::Call { callee, span, .. } if is_from_static_call(callee) => {
                flag(span, &mut self.diags, self.ctx);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
