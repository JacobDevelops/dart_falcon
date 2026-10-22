//! Warns when a disposable constructor result is discarded.
//!
//! Only resolved constructors with an instance dispose method are checked.
//! Passing a value as a call or constructor argument may transfer ownership:
//! without a callee ownership contract, reporting it would be speculative.
//! Reading a member or interpolating an instance does not transfer the instance.
//! Assignments, variable initializers, returns, and immediately disposed
//! expressions are also ignored. This conservative rule checks temporaries only;
//! it does not prove that retained locals are eventually disposed. Owned fields
//! belong to dispose-fields, dispose-focus-node-and-timer, or
//! proper-controller-dispose. Subscription and sink release obligations remain
//! with cancel-subscriptions and close-sinks.

use crate::lint::support::controller_disposal::{constructed_identity, has_dispose};
use falcon_analyze::{
    AnalyzeContext, ResolvedType, Rule, RulePreparation, SemanticModel, SignatureIndex,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
#[derive(Default)]
pub struct AvoidUndisposedInstances;

impl Rule for AvoidUndisposedInstances {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
            signatures,
            retained: Vec::new(),
            barriers: Vec::new(),
            locals: falcon_analyze::resolve::lexical_usage::local_declarations(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    retained: Vec<Span>,
    barriers: Vec<Span>,
    locals: std::collections::HashMap<usize, NodeId>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn retained(&self, span: Span) -> bool {
        let enclosing_size = |spans: &[Span]| {
            spans
                .iter()
                .filter(|outer| outer.contains(span))
                .map(|outer| outer.end - outer.start)
                .min()
        };
        match (
            enclosing_size(&self.retained),
            enclosing_size(&self.barriers),
        ) {
            (Some(retained), Some(barrier)) => retained <= barrier,
            (Some(_), None) => true,
            _ => false,
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_var_declarator(&mut self, node: VarDeclarator<'a>) -> StructuralTraversal {
        if let Some(value) = node.initializer() { self.retained.push(value.span()); }
        StructuralTraversal::Continue
    }

    fn visit_constructor_initializer(&mut self, node: ConstructorInitializer<'a>) -> StructuralTraversal {
        if let ConstructorInitializer::FieldInit(field) = node {
            self.retained.push(field.value().span());
        }
        StructuralTraversal::Continue
    }

    fn visit_arg_list(&mut self, node: ArgList<'a>) -> StructuralTraversal {
        self.retained.push(node.span());
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Return(statement) = node && let Some(value) = statement.value() {
            self.retained.push(value.span());
        }
        StructuralTraversal::Continue
    }

    fn visit_function_body(&mut self, node: FunctionBody<'a>) -> StructuralTraversal {
        if let FunctionBody::Arrow(arrow) = node { self.retained.push(arrow.expr().span()); }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.retained.retain(|span| span.end > node.span().start);
        self.barriers.retain(|span| span.end > node.span().start);
        match node {
            Expr::Call(call) => self.barriers.push(call.callee().span()),
            Expr::Field(field) => self.barriers.push(field.object().span()),
            Expr::FuncExpr(_) | Expr::StringLit(_) => self.barriers.push(node.span()),
            _ => {}
        }
        if let Expr::Assign(assignment) = node { self.retained.push(assignment.value().span()); }
        if let Expr::Call(call) = node
            && let Expr::Field(field) = call.callee().unparenthesized()
            && field.field().name() == "dispose"
        {
            self.retained.push(field.object().span());
        }
        if let Expr::Cascade(cascade) = node
            && (self.retained.contains(&node.span()) || cascade.sections().any(|section| section.ops().any(|operation| matches!(operation, CascadeOp::Call(call) if call.name().name() == "dispose")))) {
            self.retained.push(cascade.object().span());
        }
        if !matches!(node, Expr::Call(_) | Expr::New(_)) || self.retained(node.span()) {
            return StructuralTraversal::Continue;
        }
        if let Expr::Call(call) = node
            && self.locals.contains_key(&call.callee().unparenthesized().span().start)
        {
            return StructuralTraversal::Continue;
        }
        let Some(identity) = constructed_identity(node, &self.model, self.signatures) else { return StructuralTraversal::Continue; };
        let ty = ResolvedType::interface_identity(identity);
        if has_dispose(&ty, self.signatures, &self.model) {
            self.diagnostics.push(self.ctx.diagnostic("Retain this disposable instance and dispose it when its owner is released.", node.span()));
        }
        StructuralTraversal::Continue
    }
}
}
