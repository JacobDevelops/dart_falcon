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
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let visitor = DynamicVisitor {
            diagnostics: Vec::new(),
            ctx,
        };
        RulePreparation::Structural(Box::new(visitor))
    }
}

struct DynamicVisitor<'a, 'ctx> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for DynamicVisitor<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_dart_type(&mut self, node: &DartType) -> StructuralTraversal {
        if let DartType::Dynamic { span } = node {
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-dynamic",
                "Avoid using the 'dynamic' type",
                *span,
            ));
        }
        StructuralTraversal::Continue
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        let map_arguments = match parent {
            StructuralNode::NamedType(named) => named
                .segments
                .last()
                .is_some_and(|segment| segment.name == "Map"),
            StructuralNode::Expr(Expr::Map { .. })
            | StructuralNode::MapPattern(_) => true,
            StructuralNode::Expr(Expr::Call { callee, .. }) if is_map_name(callee) => {
                if let StructuralNode::Expr(expr) = child
                    && std::ptr::eq(expr, callee.as_ref())
                {
                    return false;
                }
                true
            }
            StructuralNode::Expr(Expr::GenericInstantiation { target, .. }) if is_map_name(target) => {
                if let StructuralNode::Expr(expr) = child
                    && std::ptr::eq(expr, target.as_ref())
                {
                    return false;
                }
                true
            }
            _ => false,
        };
        !map_arguments || !matches!(child, StructuralNode::DartType(DartType::Dynamic { .. }))
    }
}
}

fn is_map_name(expr: &Expr) -> bool {
    match expr.unparenthesized() {
        Expr::Ident(identifier) => identifier.name == "Map",
        Expr::Field { object, field, .. } => {
            matches!(object.unparenthesized(), Expr::Ident(_)) && field.name == "Map"
        }
        _ => false,
    }
}
