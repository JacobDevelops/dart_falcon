//! Flags instance methods writing resolved static fields, including compound
//! assignments and increments. Static methods, top-level functions, declarations,
//! instance fields, setters, and shadowed names are excluded.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, MemberKind, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidAssigningToStaticField;

impl Rule for AvoidAssigningToStaticField {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let target = match node {
            Expr::Assign(assign) => assign.target(),
            Expr::PostfixIncDec(increment) => increment.operand(),
            Expr::Unary(unary) if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                unary.operand()
            }
            _ => return,
        }
        .unparenthesized();
        if !node
            .syntax()
            .ancestors()
            .find_map(MethodDecl::cast)
            .is_some_and(|method| !method.is_static())
        {
            return;
        }
        // Static fields live in the root environment, but is_own_field uses
        // instance-member lookup and therefore deliberately excludes them.
        let (identity, name) = match target {
            Expr::Ident(id)
                if !state.environment.is_bound(id.name())
                    || state.environment.is_bound_in_root_scope(id.name()) =>
            {
                let Some(ResolvedType::Interface { identity, .. }) = state.enclosing_type.as_ref()
                else {
                    return;
                };
                (identity.clone(), id.name())
            }
            Expr::Field(field) => {
                let Some(segments) = falcon_analyze::expression_segments(field.object(), true)
                else {
                    return;
                };
                if state.environment.is_bound(segments[0]) {
                    return;
                }
                let Some(ResolvedType::Interface { identity, .. }) =
                    state.model.resolve_type_name(&segments)
                else {
                    return;
                };
                (identity, field.field().name())
            }
            _ => return,
        };
        if state
            .signatures
            .declared_member_facts(&identity, name)
            .iter()
            .any(|fact| fact.is_static && matches!(fact.kind, MemberKind::Field { .. }))
        {
            self.diags.push(self.ctx.diagnostic("Avoid assigning to static fields from instance methods; move the state to an instance or make the method static.", target.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
