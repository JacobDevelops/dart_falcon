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

use std::cmp::Reverse;
use std::collections::BinaryHeap;

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
            retained: BinaryHeap::new(),
            barriers: BinaryHeap::new(),
            locals: ctx.local_declarations(program),
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    retained: BinaryHeap<Reverse<(usize, usize)>>,
    barriers: BinaryHeap<Reverse<(usize, usize)>>,
    locals: &'a std::collections::HashMap<usize, NodeId>,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn retained(&self, span: Span) -> bool {
        let enclosing_size = |spans: &BinaryHeap<Reverse<(usize, usize)>>| {
            spans
                .iter()
                .filter(|Reverse((end, start))| *start <= span.start && span.end <= *end)
                .map(|Reverse((end, start))| end - start)
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
        if let Some(value) = node.initializer() {
            let span = value.span();
            self.retained.push(Reverse((span.end, span.start)));
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor_initializer(&mut self, node: ConstructorInitializer<'a>) -> StructuralTraversal {
        if let ConstructorInitializer::FieldInit(field) = node {
            let span = field.value().span();
            self.retained.push(Reverse((span.end, span.start)));
        }
        StructuralTraversal::Continue
    }

    fn visit_arg_list(&mut self, node: ArgList<'a>) -> StructuralTraversal {
        let span = node.span();
        self.retained.push(Reverse((span.end, span.start)));
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::Return(statement) = node && let Some(value) = statement.value() {
            let span = value.span();
            self.retained.push(Reverse((span.end, span.start)));
        }
        StructuralTraversal::Continue
    }

    fn visit_function_body(&mut self, node: FunctionBody<'a>) -> StructuralTraversal {
        if let FunctionBody::Arrow(arrow) = node {
            let span = arrow.expr().span();
            self.retained.push(Reverse((span.end, span.start)));
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        let span = node.span();
        // Expire by end position without rescanning every enclosing receiver.
        for spans in [&mut self.retained, &mut self.barriers] {
            while spans.peek().is_some_and(|Reverse((end, _))| *end <= span.start) {
                spans.pop();
            }
        }
        let barrier = match node {
            Expr::Call(call) => Some(call.callee().span()),
            Expr::Field(field) => Some(field.object().span()),
            Expr::FuncExpr(_) | Expr::StringLit(_) => Some(span),
            _ => None,
        };
        if let Some(span) = barrier {
            self.barriers.push(Reverse((span.end, span.start)));
        }
        if let Expr::Assign(assignment) = node {
            let span = assignment.value().span();
            self.retained.push(Reverse((span.end, span.start)));
        }
        if let Expr::Call(call) = node
            && let Expr::Field(field) = call.callee().unparenthesized()
            && field.field().name() == "dispose"
        {
            let span = field.object().span();
            self.retained.push(Reverse((span.end, span.start)));
        }
        if let Expr::Cascade(cascade) = node
            && (self.retained.iter().any(|entry| *entry == Reverse((span.end, span.start))) || cascade.sections().any(|section| section.ops().any(|operation| matches!(operation, CascadeOp::Call(call) if call.name().name() == "dispose")))) {
            let span = cascade.object().span();
            self.retained.push(Reverse((span.end, span.start)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn retention_barriers_and_expiry_preserve_exact_diagnostics() {
        let source = r#"
class Disposable {
  Disposable.discarded();
  Disposable.kept();
  Disposable.argument();
  Disposable.receiver();
  Disposable.disposed();
  Disposable.cascade();
  Disposable.closure();
  Disposable.arrow();
  Disposable.interpolation();
  Disposable.after();
  Disposable get next => this;
  void dispose() {}
}
void consume(Object value) {}
void f() {
  new Disposable.discarded();
  final kept = new Disposable.kept();
  consume(new Disposable.argument());
  final receiver = (new Disposable.receiver()).next;
  (new Disposable.disposed()).dispose();
  new Disposable.cascade()..dispose();
  final closure = () { new Disposable.closure(); };
  final arrow = () => new Disposable.arrow();
  '${new Disposable.interpolation()}';
  new Disposable.after();
}
"#;
        let diagnostics = crate::lint::test_support::diagnostics(
            "avoid-undisposed-instances",
            &AvoidUndisposedInstances,
            Path::new("/project/lib/test.dart"),
            source,
            false,
        );
        let actual: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| (diagnostic.span.start, diagnostic.span.end))
            .collect();
        // The retained closure initializer equals its function-expression barrier:
        // retention wins equal-size ties, including constructors in its block body.
        let expected: Vec<_> = [
            "new Disposable.discarded()",
            "new Disposable.receiver()",
            "new Disposable.interpolation()",
            "new Disposable.after()",
        ]
        .into_iter()
        .map(|expression| {
            let start = source.find(expression).unwrap();
            (start, start + expression.len())
        })
        .collect();
        assert_eq!(actual, expected);
    }
}
