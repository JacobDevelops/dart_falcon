//! Warn when SDK Iterable.reduce can receive an empty collection.
//!
//! Constant nonempty collections and fresh literals with an unconditional
//! element are safe, including reduce as the first cascade operation.
//! Immediate isEmpty/isNotEmpty branches require a local or final-field receiver
//! and no intervening expression that can mutate it. Custom reduce members and
//! unresolved receivers are excluded.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, ConstantValue, MemberKind, QualifiedName, ResolvedType, Rule, RulePreparation,
    ConstantContext, evaluate_constant,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnsafeReduce;

impl Rule for AvoidUnsafeReduce {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}

fn sdk_reduce(receiver: &ResolvedType, state: &SemanticState<'_>) -> bool {
    if let Some(owner) = state
        .signatures
        .resolved_member_owner(receiver, "reduce", &state.model)
    {
        return ["Iterable", "List", "Set"]
            .iter()
            .any(|name| owner.is_sdk("dart:core", name));
    }
    // Without SDK sources, direct core identities still establish the member.
    ["Iterable", "List", "Set"]
        .iter()
        .any(|name| receiver.interface("dart:core", name))
}

fn nonempty(receiver: Expr<'_>, state: &SemanticState<'_>) -> bool {
    let receiver = receiver.unparenthesized();
    match receiver {
        Expr::List(list)
            if list
                .elements()
                .any(|element| matches!(element, CollectionElement::Expr(_))) =>
        {
            return true;
        }
        Expr::Set(set)
            if set
                .elements()
                .any(|element| matches!(element, CollectionElement::Expr(_))) =>
        {
            return true;
        }
        _ => {}
    }
    match evaluate_constant(receiver, ConstantContext { owner: state.constant_owner.as_ref(), locals: Some(&state.visible_local_constants()), ..ConstantContext::new(&state.model, &state.signatures) }) {
        Some(ConstantValue::List(_, values) | ConstantValue::Set(_, values)) => !values.is_empty(),
        _ => false,
    }
}

// A truth-sensitive proof, not a traversal: only negation preserves the
// recognized condition while reversing which branch guarantees nonemptiness.
fn nonempty_condition(condition: Expr<'_>, receiver: &str, truth: bool) -> bool {
    match condition.unparenthesized() {
        Expr::Unary(unary) => {
            unary.op() == UnaryOp::Bang && nonempty_condition(unary.operand(), receiver, !truth)
        }
        Expr::Field(field) => {
            !field.is_null_safe()
                && matches!(field.object().unparenthesized(), Expr::Ident(name) if name.name() == receiver)
                && matches!(
                    (field.field().name(), truth),
                    ("isNotEmpty", true) | ("isEmpty", false)
                )
        }
        Expr::IntLit(_)
        | Expr::DoubleLit(_)
        | Expr::StringLit(_)
        | Expr::BoolLit(_)
        | Expr::NullLit(_)
        | Expr::Ident(_)
        | Expr::Paren(_)
        | Expr::This(_)
        | Expr::Super(_)
        | Expr::PostfixIncDec(_)
        | Expr::Binary(_)
        | Expr::Assign(_)
        | Expr::Conditional(_)
        | Expr::Is(_)
        | Expr::As(_)
        | Expr::Index(_)
        | Expr::Call(_)
        | Expr::Cascade(_)
        | Expr::List(_)
        | Expr::Map(_)
        | Expr::Set(_)
        | Expr::Record(_)
        | Expr::FuncExpr(_)
        | Expr::New(_)
        | Expr::DotShorthand(_)
        | Expr::Await(_)
        | Expr::Throw(_)
        | Expr::Switch(_)
        | Expr::NullAssert(_)
        | Expr::SymbolLit(_)
        | Expr::GenericInstantiation(_)
        | Expr::Error(_) => false,
    }
}

fn safe_callback(argument: Expr<'_>, state: &SemanticState<'_>) -> bool {
    match argument.unparenthesized() {
        Expr::FuncExpr(_) => true,
        Expr::Ident(name) => {
            if state.environment.is_bound(name.name()) {
                return true;
            }
            if let Some(facts) = state
                .enclosing_type
                .as_ref()
                .and_then(|receiver| {
                    state
                        .signatures
                        .resolved_member_facts(receiver, name.name(), &state.model)
                })
                .filter(|facts| !facts.is_empty())
            {
                return facts.iter().all(|fact| fact.kind == MemberKind::Method);
            }
            !state
                .model
                .resolve_value(name.name())
                .is_some_and(|identity| state.signatures.is_value_getter(&identity))
        }
        Expr::Field(field) => {
            if let Expr::Ident(prefix) = field.object().unparenthesized() {
                if !state.environment.is_bound(prefix.name()) {
                    let name = QualifiedName::prefixed(prefix.name(), field.field().name());
                    if let Some(identity) = state.model.resolve_value(name) {
                        return !state.signatures.is_value_getter(&identity);
                    }
                }
            } else if !matches!(field.object().unparenthesized(), Expr::This(_)) {
                return false;
            }
            state
                .signatures
                .resolved_member_facts(
                    &state.infer_receiver(field.object()),
                    field.field().name(),
                    &state.model,
                )
                .is_none_or(|facts| facts.iter().all(|fact| fact.kind == MemberKind::Method))
        }
        _ => false,
    }
}

fn immediately_guarded(call: CallExpr<'_>, receiver: Expr<'_>, state: &SemanticState<'_>) -> bool {
    let Expr::Ident(receiver) = receiver.unparenthesized() else {
        return false;
    };
    let name = receiver.name();
    let stable = if state.environment.binding_declaration(name).is_some() {
        !state.environment.is_local_function(name)
    } else {
        state.enclosing_type.as_ref().is_some_and(|owner| {
            state
                .signatures
                .resolved_member_facts(owner, name, &state.model)
                .is_some_and(|facts| {
                    !facts.is_empty()
                        && facts.iter().all(|fact| {
                            !fact.is_abstract
                                && matches!(
                                    fact.kind,
                                    MemberKind::Field {
                                        writable: false,
                                        ..
                                    }
                                )
                        })
                })
        })
    };
    if !stable {
        return false;
    }
    // Evaluating an arbitrary callback-producing expression could empty the list.
    if !call
        .args()
        .positional()
        .all(|argument| safe_callback(argument, state))
    {
        return false;
    }
    let mut node = call.syntax();
    while let Some(parent) = node.parent() {
        if let Some(Expr::Binary(binary)) = Expr::cast(parent)
            && binary.right().span() == node.span()
        {
            match binary.op() {
                BinaryOp::And => return nonempty_condition(binary.left(), receiver.name(), true),
                BinaryOp::Or => return nonempty_condition(binary.left(), receiver.name(), false),
                _ => {}
            }
        }
        let evaluated_first = match Expr::cast(parent) {
            Some(Expr::Paren(_)) => true,
            Some(Expr::Binary(binary)) => binary.left().span() == node.span(),
            Some(Expr::Assign(assignment)) => {
                assignment.op() == AssignOp::Eq
                    && matches!(assignment.target().unparenthesized(), Expr::Ident(_))
                    && assignment.value().span() == node.span()
            }
            Some(Expr::Field(field)) => field.object().span() == node.span(),
            Some(Expr::Call(outer)) => outer.callee().span() == node.span(),
            _ => false,
        };
        if evaluated_first || VarDeclarator::cast(parent).is_some() {
            node = parent;
            continue;
        }
        if let Some(Expr::Conditional(branch)) = Expr::cast(parent) {
            let truth = if branch.then_expr().span() == node.span() {
                true
            } else if branch.else_expr().span() == node.span() {
                false
            } else {
                return false;
            };
            return nonempty_condition(branch.condition(), receiver.name(), truth);
        }
        match Stmt::cast(parent) {
            Some(Stmt::Return(_) | Stmt::Expr(_)) => node = parent,
            Some(Stmt::LocalVar(declaration))
                if !declaration.is_late()
                    && declaration
                        .declarators()
                        .next()
                        .is_some_and(|first| first.span() == node.span()) =>
            {
                node = parent;
            }
            Some(Stmt::Block(block)) => {
                let previous = block
                    .stmts()
                    .take_while(|statement| statement.span() != node.span())
                    .last();
                let Some(previous) = previous else {
                    node = parent;
                    continue;
                };
                let Stmt::If(guard) = previous else {
                    return false;
                };
                let IfCondition::Expr(condition) = guard.condition() else {
                    return false;
                };
                let branch = match guard.then_branch() {
                    Stmt::Block(block) => block.stmts().last(),
                    statement => Some(statement),
                };
                return guard.else_branch().is_none()
                    && matches!(
                        branch,
                        Some(Stmt::Return(_) | Stmt::Throw(_) | Stmt::Continue(_) | Stmt::Break(_))
                    )
                    && nonempty_condition(condition, receiver.name(), false);
            }
            Some(Stmt::If(branch)) => {
                let IfCondition::Expr(condition) = branch.condition() else {
                    return false;
                };
                let truth = if branch.then_branch().span() == node.span() {
                    true
                } else if branch
                    .else_branch()
                    .is_some_and(|branch| branch.span() == node.span())
                {
                    false
                } else {
                    return false;
                };
                return nonempty_condition(condition, receiver.name(), truth);
            }
            _ => return false,
        }
    }
    false
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return;
        };
        if field.field().name() == "reduce"
            && sdk_reduce(&state.infer_receiver(field.object()), state)
            && !nonempty(field.object(), state)
            && !immediately_guarded(call, field.object(), state)
        {
            self.diagnostics.push(self.ctx.diagnostic("Reducing a possibly empty iterable can throw; use fold or check for emptiness.", field.field().span()));
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        if let CascadeOp::Call(call) = operation
            && call.name().name() == "reduce"
            && sdk_reduce(receiver, state)
            && !call
                .syntax()
                .parent()
                .and_then(CascadeSection::cast)
                .and_then(|section| section.syntax().parent())
                .and_then(CascadeExpr::cast)
                .is_some_and(|cascade| {
                    nonempty(cascade.object(), state)
                        && cascade
                            .sections()
                            .next()
                            .and_then(|section| section.ops().next())
                            .is_some_and(|first| first.span() == call.span())
                })
        {
            self.diagnostics.push(self.ctx.diagnostic("Reducing a possibly empty iterable can throw; use fold or check for emptiness.", call.name().span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
