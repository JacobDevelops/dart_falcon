//! Detects writes through named function, method and constructor parameter objects.
//! Closure parameters are exempt; parameter rebinding belongs to `parameter-assignments`.
//! Parameters annotated `@mutated` (dart-code-metrics-annotations) are exempt.

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
    fn parameters(&mut self, params: FormalParamList<'_>) {
        for parameter in params.params() {
            if parameter.initializing().is_none()
                && !crate::lint::declarations::annotation(parameter.annotations(), "mutated")
            {
                self.parameters.insert(parameter.name().syntax().id());
            }
        }
    }

    fn record(&mut self, object: Expr<'_>, span: Span) {
        let mut root = object.unparenthesized();
        loop {
            root = match root {
                Expr::Field(field) => field.object().unparenthesized(),
                Expr::Index(index) => index.object().unparenthesized(),
                Expr::NullAssert(assertion) => assertion.operand().unparenthesized(),
                Expr::As(cast) => cast.expr().unparenthesized(),
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
    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        if let Some(params) = node.params() {
            self.parameters(params);
        }
        StructuralTraversal::Continue
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        self.parameters(node.params());
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        self.parameters(node.params);
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        if let ClassMember::Operator(operator) = node {
            self.parameters(operator.params());
        }
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::LocalFunc(function) = node {
            self.parameters(function.params());
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
                "Avoid mutating an object supplied as a parameter.",
                *span,
            )).collect()
    }
}
}
