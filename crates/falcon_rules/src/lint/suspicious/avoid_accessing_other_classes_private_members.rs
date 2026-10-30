//! Enforces class-level encapsulation even inside one Dart library.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, ResolvedTypeView, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidAccessingOtherClassesPrivateMembers;

impl Rule for AvoidAccessingOtherClassesPrivateMembers {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
            this_cascade_sections: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
    this_cascade_sections: Vec<Span>,
}

impl Collector<'_> {
    fn check(&mut self, name: Identifier<'_>, receiver: &ResolvedType, state: &SemanticState<'_>) {
        if !name.name().starts_with('_') {
            return;
        }
        let Some(owner) = state
            .signatures
            .resolved_readable_member_owner(receiver, name.name(), &state.model)
            .or_else(|| {
                state
                    .signatures
                    .resolved_writable_member_owner(receiver, name.name(), &state.model)
            })
            .or_else(|| {
                let ResolvedTypeView::Interface { identity, .. } = receiver.view() else {
                    return None;
                };
                state
                    .signatures
                    .declared_member_facts(identity, name.name())
                    .iter()
                    .find(|member| member.is_static)
                    .map(|member| member.declaring_type.clone())
            })
        else {
            return;
        };
        let enclosing_type = match state.enclosing_type.as_ref().map(ResolvedType::view) {
            Some(ResolvedTypeView::Interface { identity, .. }) => Some(identity),
            _ => None,
        };
        let receiver_type = match receiver.view() {
            ResolvedTypeView::Interface { identity, .. } => Some(identity),
            _ => None,
        };
        if enclosing_type.is_some() && receiver_type == enclosing_type {
            return;
        }
        if state.constant_owner.as_ref() != Some(&owner) && enclosing_type != Some(&owner) {
            self.diagnostics.push(self.ctx.diagnostic("Avoid accessing another class's private member.", name.span()));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Cascade(cascade) = node
            && matches!(cascade.object().unparenthesized(), Expr::This(_))
        {
            self.this_cascade_sections
                .extend(cascade.sections().map(CascadeSection::span));
        }
        if let Expr::Field(field) = node {
            let object = field.object();
            if matches!(object.unparenthesized(), Expr::This(_) | Expr::Super(_)) {
                return;
            }
            let mut receiver =
                state.infer_access_receiver(object, field.field().name(), field.is_null_safe());
            // A class literal denotes the static namespace, not an instance of Type.
            if let Expr::Ident(name) = object.unparenthesized()
                && !state.environment.is_bound(name.name())
                && let Some(identity) = state.model.resolve_name(name.name())
            {
                receiver = ResolvedType::new_interface(identity, Vec::new().into(), false, false);
            }
            self.check(field.field(), &receiver, state);
        } else if let Expr::DotShorthand(shorthand) = node
            && let Some(receiver) = state.expected()
        {
            self.check(shorthand.name(), receiver, state);
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        section: Span,
        state: &SemanticState<'a>,
    ) {
        if self.this_cascade_sections.contains(&section) {
            return;
        }
        match operation {
            CascadeOp::Field(field) => self.check(field.name(), receiver, state),
            CascadeOp::Call(call) => self.check(call.name(), receiver, state),
            CascadeOp::Assign(assignment) => {
                if let Expr::Ident(name) = assignment.target().unparenthesized() {
                    self.check(name, receiver, state);
                }
            }
            _ => {}
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
