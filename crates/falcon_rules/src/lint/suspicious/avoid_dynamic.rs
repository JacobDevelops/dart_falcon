//! Flags explicit `dynamic` type annotations.
//!
//! `dynamic` opts an expression out of static type checking, so member accesses
//! and calls on it are resolved at runtime and any mistake surfaces as a crash
//! rather than a compile error. Reaching for it is usually a shortcut where a
//! concrete type, a generic, or `Object?` plus explicit checks would be safer.
//! Prefer a precise type. `dynamic` used directly as a `Map` type argument — the
//! documented JSON escape hatch, `Map<String, dynamic>` — is exempt, but a
//! `dynamic` nested more deeply (such as `Map<String, List<dynamic>>`) is still
//! flagged.
//! All explicit type positions are checked, including record fields, type
//! aliases, generic bounds, patterns, and types inside initializers.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Structural, DynamicVisitor, ctx, diagnostics)
    }
}

struct DynamicVisitor<'a, 'ctx> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for DynamicVisitor<'_, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let DartType::Dynamic(dynamic) = node {
            self.diagnostics.push(self.ctx.diagnostic(
                "Avoid using the `dynamic` type.",
                dynamic.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn route_child(&mut self, parent: StructuralNode<'a>, child: StructuralNode<'a>) -> bool {
        let map_arguments = match parent {
            StructuralNode::NamedType(named) => named
                .segments()
                .last()
                .is_some_and(|segment| segment.name() == "Map"),
            StructuralNode::Expr(Expr::Map(_))
            | StructuralNode::MapPattern(_) => true,
            StructuralNode::Expr(Expr::Call(call)) if is_map_name(call.callee()) => {
                if let StructuralNode::Expr(expr) = &child
                    && expr.syntax().id() == call.callee().syntax().id()
                {
                    return false;
                }
                true
            }
            StructuralNode::Expr(Expr::GenericInstantiation(generic)) if is_map_name(generic.target()) => {
                if let StructuralNode::Expr(expr) = &child
                    && expr.syntax().id() == generic.target().syntax().id()
                {
                    return false;
                }
                true
            }
            _ => false,
        };
        !map_arguments || !matches!(child, StructuralNode::DartType(DartType::Dynamic(_)))
    }
}
}

fn is_map_name(expr: Expr<'_>) -> bool {
    match expr.unparenthesized() {
        Expr::Ident(identifier) => identifier.name() == "Map",
        Expr::Field(field) => {
            matches!(field.object().unparenthesized(), Expr::Ident(_))
                && field.field().name() == "Map"
        }
        _ => false,
    }
}
