//! Require `hashCode` and `==` to be overridden together.
//!
//! Flags a class declaration that overrides `==` without also defining
//! `hashCode`, or defines `hashCode` without overriding `==`. The
//! two are bound by a contract: any two objects that compare equal must return
//! the same hash code. Overriding only one breaks that invariant, so the type
//! misbehaves as a `Map` key or `Set` element — lookups miss and duplicates slip
//! through. Override both and derive them from the same fields.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct HashAndEquals;

impl Rule for HashAndEquals {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        let members = match decl {
            TopLevelDecl::Class(class) => class.members(),
            TopLevelDecl::MixinClass(class) => class.members(),
            _ => return StructuralTraversal::SkipChildren,
        };
        check_members(members, self.ctx, &mut self.diags);
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

/// The span of the `==` operator, if the type declares one.
fn equals_span<'a>(mut members: AstChildren<'a, ClassMember<'a>>) -> Option<Span> {
    members.find_map(|m| match m {
        ClassMember::Operator(op) if op.op() == "==" => {
            Some(crate::lint::declarations::operator_token_span(op))
        }
        _ => None,
    })
}

/// The span of a `hashCode` override — as a getter, a field, or (defensively) a
/// method — if the type declares one. A `hashCode` *setter* does not override
/// `Object.hashCode`, so it does not count.
fn hashcode_span<'a>(mut members: AstChildren<'a, ClassMember<'a>>) -> Option<Span> {
    members.find_map(|m| match m {
        ClassMember::Getter(g) if g.name().name() == "hashCode" => Some(g.name().span()),
        ClassMember::Method(method) if method.name().name() == "hashCode" => {
            Some(method.name().span())
        }
        ClassMember::Field(field) => field
            .declarators()
            .find(|declarator| declarator.name().name() == "hashCode")
            .map(|declarator| declarator.name().span()),
        _ => None,
    })
}

fn check_members<'a>(
    members: AstChildren<'a, ClassMember<'a>>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let eq = equals_span(members.clone());
    let hash = hashcode_span(members);
    let (span, message) = match (eq, hash) {
        (Some(eq), None) => (eq, "Override 'hashCode' if you override the '==' operator."),
        (None, Some(hash)) => (
            hash,
            "Override the '==' operator if you override 'hashCode'.",
        ),
        _ => return,
    };
    diags.push(ctx.diagnostic("hash-and-equals", message, span));
}
