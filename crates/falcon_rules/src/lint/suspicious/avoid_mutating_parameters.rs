//! Detects writes through parameter objects, using lexical declaration identities.
//! Parameter rebinding is covered separately by `parameter-assignments`.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMutatingParameters;

impl Rule for AvoidMutatingParameters {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            references: local_declarations(program),
            parameters: HashSet::new(),
            writes: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: HashMap<usize, NodeId>,
    parameters: HashSet<NodeId>,
    writes: Vec<(NodeId, Span)>,
}

impl Collector<'_> {
    fn record(&mut self, object: Expr<'_>, span: Span) {
        let mut root = object.unparenthesized();
        loop {
            root = match root {
                Expr::Field(field) => field.object().unparenthesized(),
                Expr::Index(index) => index.object().unparenthesized(),
                Expr::NullAssert(assertion) => assertion.operand().unparenthesized(),
                Expr::Ident(name) => {
                    if let Some(declaration) = self.references.get(&name.span().start) {
                        self.writes.push((*declaration, span));
                    }
                    return;
                }
                _ => return,
            };
        }
    }

    fn target(&mut self, target: Expr<'_>) {
        match target.unparenthesized() {
            Expr::Field(field) => self.record(field.object(), target.span()),
            Expr::Index(index) => self.record(index.object(), target.span()),
            _ => {}
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_formal_param(&mut self, parameter: FormalParam<'a>) -> StructuralTraversal {
        if parameter.initializing().is_none()
            && !crate::lint::declarations::annotation(parameter.annotations(), "mutated")
        {
            self.parameters.insert(parameter.name().syntax().id());
        }
        StructuralTraversal::Continue
    }

    fn visit_setter_decl(&mut self, setter: SetterDecl<'a>) -> StructuralTraversal {
        if !crate::lint::declarations::annotation(setter.param_annotations(), "mutated") {
            self.parameters.insert(setter.param().syntax().id());
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, expression: Expr<'a>) -> StructuralTraversal {
        match expression {
            Expr::Assign(assignment) => self.target(assignment.target()),
            Expr::PostfixIncDec(increment) => self.target(increment.operand()),
            Expr::Unary(unary) if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                self.target(unary.operand());
            }
            Expr::Cascade(cascade) => {
                for section in cascade.sections() {
                    for operation in section.ops() {
                        match operation {
                            CascadeOp::Assign(_) | CascadeOp::IndexAssign(_) => {
                                self.record(cascade.object(), section.span());
                                break;
                            }
                            CascadeOp::Call(_) | CascadeOp::Invoke(_) => break,
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.writes.iter().filter(|(declaration, _)| self.parameters.contains(declaration))
            .map(|(_, span)| self.ctx.diagnostic(
                "avoid-mutating-parameters",
                "Avoid mutating an object supplied as a parameter.",
                *span,
            )).collect()
    }
}
}
