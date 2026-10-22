//! Flags `.from` collection constructors in favor of the `.of` constructor.
//!
//! Catches `List.from`, `Set.from`, and `Iterable.from` — whether written as a
//! static call (`List.from(xs)`, `List<int>.from(xs)`) or with `new`. The `.from`
//! constructor takes an `Iterable<dynamic>` and re-types its elements, so a type
//! mismatch surfaces only as a runtime cast failure; the `.of` constructor takes a
//! statically typed `Iterable<E>`, letting the compiler reject bad element types up
//! front. Prefer `.of` unless you specifically need `.from`'s dynamic widening.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferIterableOf;

impl Rule for PreferIterableOf {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
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
fn is_from_constructor(dart_type: DartType<'_>, constructor_name: Option<Identifier<'_>>) -> bool {
    let DartType::Named(nt) = dart_type else {
        return false;
    };
    // Case A: `.from` kept as a separate named constructor.
    if let Some(ctor) = constructor_name
        && ctor.name() == "from"
        && let Some(last) = nt.segments().last()
    {
        return is_iterable_base(last.name());
    }
    // Case B: `.from` folded into the qualified type name -> [.., base, "from"].
    let mut segments = nt.segments().rev();
    segments
        .next()
        .is_some_and(|segment| segment.name() == "from")
        && segments
            .next()
            .is_some_and(|segment| is_iterable_base(segment.name()))
}

/// Resolve the base type name of a receiver expression.
/// `List`            -> `Expr::Ident("List")`
/// `List<int>`       -> `Expr::GenericInstantiation { target: Ident("List"), type_args: [int], .. }`
fn base_type_name<'a>(expr: Expr<'a>) -> Option<&'a str> {
    match expr.unparenthesized() {
        Expr::Ident(id) => Some(id.name()),
        Expr::Call(call) => base_type_name(call.callee()),
        Expr::GenericInstantiation(instantiation) => base_type_name(instantiation.target()),
        _ => None,
    }
}

/// True for `List.from(...)` / `List<int>.from(...)` parsed as `Call(Field(receiver, "from"), args)`.
fn is_from_static_call(callee: Expr<'_>) -> bool {
    if let Expr::Field(field) = callee.unparenthesized()
        && field.field().name() == "from"
        && let Some(base) = base_type_name(field.object())
    {
        return is_iterable_base(base);
    }
    false
}

/// Detection runs through the exhaustive shared walker, so a violation cannot
/// hide inside newer syntax the way a hand-rolled `_ => {}` walk allowed.
struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::New(construction) if is_from_constructor(construction.dart_type(), construction.constructor_name()) => {
                self.diags.push(self.ctx.diagnostic(
                    "Prefer using the `of` constructor instead of `from`.",
                    construction.span(),
                ));
            }
            Expr::Call(call) if is_from_static_call(call.callee()) => {
                self.diags.push(self.ctx.diagnostic(
                    "Prefer using the `of` constructor instead of `from`.",
                    call.span(),
                ));
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}
