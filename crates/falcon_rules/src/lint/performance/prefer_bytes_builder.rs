//! Prefer `BytesBuilder` over repeated concatenation, mutation, or spreading of
//! byte lists. This rule treats SDK `List<int>` and `Uint8List` values as bytes,
//! including operations outside loops, and ignores constant collection spreads.
//! Report each concatenation chain once, or only its enclosing byte spread.
//!
//! Scope this rule to byte-processing files when integer lists represent other data.

use falcon_analyze::{
    AnalyzeContext, CascadeAssignment, ConstContexts, ResolvedType, ResolvedTypeView, Rule,
    RulePreparation, SemanticRuleVisitor, SemanticState,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

/// Prefer `BytesBuilder` when composing integer lists with concatenation,
/// `addAll`, or spreads. All SDK `List<int>` values and `Uint8List` values
/// are treated as bytes, even outside loops; scope the rule to byte-processing
/// files when other integer lists should retain these operations.
#[derive(Default)]
pub struct PreferBytesBuilder;

impl Rule for PreferBytesBuilder {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            contexts: ctx.const_contexts(program),
            diagnostics: Vec::new(),
            spreads: Vec::new(),
            concatenations: Vec::new(),
            nested_concatenations: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    contexts: &'a ConstContexts,
    diagnostics: Vec<Diagnostic>,
    spreads: Vec<NodeId>,
    concatenations: Vec<(NodeId, Span)>,
    nested_concatenations: Vec<NodeId>,
}

fn is_bytes(ty: &ResolvedType) -> bool {
    let ResolvedTypeView::Interface {
        identity,
        arguments,
        ..
    } = ty.view()
    else {
        return false;
    };
    identity.is_sdk("dart:typed_data", "Uint8List")
        || (identity.is_sdk("dart:core", "List") && arguments.first().is_some_and(|argument| {
            matches!(argument.view(), ResolvedTypeView::Interface { identity, nullable: false, .. }
                    if identity.is_sdk("dart:core", "int"))
        }))
}

fn is_byte_composition<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    if is_bytes(&state.infer(expression)) {
        return true;
    }
    // Recover byte composition from its receiver when SDK operator signatures are unavailable.
    let mut receiver = expression.unparenthesized();
    let Expr::Binary(binary) = receiver else {
        return false;
    };
    if binary.op() != BinaryOp::Add {
        return false;
    }
    while let Expr::Binary(binary) = receiver
        && binary.op() == BinaryOp::Add
    {
        receiver = binary.left().unparenthesized();
    }
    is_bytes(&state.infer(receiver))
}

fn collect_spreads(element: CollectionElement<'_>, spreads: &mut Vec<NodeId>) {
    match element {
        CollectionElement::Spread(spread) => spreads.push(spread.expr().syntax().id()),
        CollectionElement::If(element) => {
            collect_spreads(element.then_elem(), spreads);
            if let Some(element) = element.else_elem() {
                collect_spreads(element, spreads);
            }
        }
        CollectionElement::For(element) => collect_spreads(element.element(), spreads),
        CollectionElement::Expr(_) | CollectionElement::NullAware(_) => {}
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let spread = self
            .spreads
            .iter()
            .position(|id| *id == expression.syntax().id());
        if let Some(index) = spread {
            self.spreads.swap_remove(index);
        }
        let spread_report = spread
            .filter(|_| is_byte_composition(expression, state))
            .map(|_| expression.unparenthesized().span());
        if spread_report.is_some()
            && let Expr::Paren(paren) = expression
            && let Expr::Binary(binary) = paren.expr().unparenthesized()
            && binary.op() == BinaryOp::Add
        {
            self.nested_concatenations.push(binary.syntax().id());
        }
        let report = match expression {
            Expr::List(list) => {
                if !self.contexts.contains_including_explicit(&list.span()) {
                    for element in list.elements() {
                        collect_spreads(element, &mut self.spreads);
                    }
                }
                spread_report
            }
            Expr::Binary(binary)
                if binary.op() == BinaryOp::Add && is_byte_composition(binary.left(), state) =>
            {
                for operand in [binary.left(), binary.right()] {
                    if let Expr::Binary(child) = operand.unparenthesized()
                        && child.op() == BinaryOp::Add
                    {
                        self.nested_concatenations.push(child.syntax().id());
                    }
                }
                if spread_report.is_none() {
                    self.concatenations
                        .push((binary.syntax().id(), binary.span()));
                }
                spread_report
            }
            Expr::Assign(assignment)
                if assignment.op() == AssignOp::PlusEq
                    && is_bytes(&state.infer(assignment.target())) =>
            {
                Some(assignment.span())
            }
            Expr::Call(call) => {
                if let Expr::Field(field) = call.callee().unparenthesized()
                    && field.field().name() == "addAll"
                    && is_bytes(&state.infer(field.object()))
                {
                    Some(field.field().span())
                } else {
                    spread_report
                }
            }
            _ => spread_report,
        };
        if let Some(span) = report {
            self.report(span);
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        _state: &SemanticState<'a>,
    ) {
        if let CascadeOp::Call(call) = operation
            && call.name().name() == "addAll"
            && is_bytes(receiver)
        {
            self.report(call.name().span());
        }
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        if assignment.operator != AssignOp::PlusEq || assignment.indexed {
            return;
        }
        if let Expr::Ident(identifier) = assignment.target
            && let Some((ty, owner)) =
                state
                    .signatures
                    .resolved_writable_field(receiver, identifier.name(), &state.model)
            && is_bytes(&state.model.substitute(&ty, &owner))
        {
            self.report(assignment.target.span());
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        // Grouped operands can be visited before their enclosing composition.
        self.nested_concatenations.sort_unstable();
        for (id, span) in std::mem::take(&mut self.concatenations) {
            if self.nested_concatenations.binary_search(&id).is_err() {
                self.report(span);
            }
        }
        self.diagnostics
    }
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(
            "Prefer `BytesBuilder` to compose bytes instead of expensive list operations.",
            span,
        ));
    }
}
