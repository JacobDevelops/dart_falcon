//! Flags an assignment immediately followed by `return` of the same variable.
//!
//! When a block ends with `x = e; return x;` the two statements can be joined
//! into `return x = e;` (or simply `return e;` for a local). The rule looks at
//! the last two statements of every block: the second-to-last must be an
//! assignment (any operator), increment, decrement, or other prefix/postfix
//! expression on the same variable or property chain that the final `return`
//! yields. A run of assignments to that same target (`x = a; x += b; return
//! x;`) is left alone, as are switch-case statement lists, which are not blocks.
//! Diagnostics cover the expression statement, including its semicolon.
//!
//! ```dart
//! int f(int a) {
//!   a += 1; // flagged
//!   return a;
//! }
//! ```
//!
//! Adopted from the official `join_return_with_assignment` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct JoinReturnWithAssignment;

impl Rule for JoinReturnWithAssignment {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a, 's> {
    ctx: &'a AnalyzeContext<'s>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a, 'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_block_end(&mut self, node: Block<'a>, state: &SemanticState<'a>) {
        let mut stmts = node.stmts().rev();
        let Some(Stmt::Return(ret)) = stmts.next() else {
            return;
        };
        let Some(returned) = ret.value() else {
            return;
        };
        let Some(second_last) = stmts.next() else {
            return;
        };
        let Some((assigned, write)) = assignment_target(second_last) else {
            return;
        };
        let third = stmts.next().and_then(assignment_target);
        if !third.is_some_and(|(third, third_write)| {
            same_element(assigned, write, third, third_write, state)
        }) && same_element(returned, false, assigned, write, state)
            && let Stmt::Expr(statement) = second_last
        {
            self.diagnostics.push(self.ctx.diagnostic("Assignment could be inlined in 'return' statement.", statement.span()));
        }
    }
}

/// The written operand of an expression statement that is an assignment, a
/// prefix expression, or a postfix expression (including `!`).
fn assignment_target(stmt: Stmt<'_>) -> Option<(Expr<'_>, bool)> {
    let Stmt::Expr(statement) = stmt else {
        return None;
    };
    match statement.expr().unparenthesized() {
        Expr::Assign(assign) => Some((assign.target(), true)),
        Expr::Unary(unary) => Some((
            unary.operand(),
            matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus),
        )),
        Expr::PostfixIncDec(expr) => Some((expr.operand(), true)),
        Expr::NullAssert(expr) => Some((expr.operand(), false)),
        _ => None,
    }
}

/// Mirrors the SDK's canonical identifier comparison. Accessors canonicalize
/// to their declaring variable, except instantiated members retain their own
/// occurrence identity rather than canonicalizing to the generic declaration.
pub(super) fn same_element<'a>(
    mut left: Expr<'a>,
    mut left_write: bool,
    mut right: Expr<'a>,
    mut right_write: bool,
    state: &SemanticState<'a>,
) -> bool {
    loop {
        match (left.unparenthesized(), right.unparenthesized()) {
            (Expr::Ident(left), Expr::Ident(right)) => {
                return identifier_element(left, left_write, state)
                    == identifier_element(right, right_write, state);
            }
            (Expr::Field(left_field_expr), Expr::Field(right_field_expr)) => {
                let left_object = left_field_expr.object();
                let right_object = right_field_expr.object();
                let left_field = left_field_expr.field();
                let right_field = right_field_expr.field();
                let left_safe = left_field_expr.is_null_safe();
                let right_safe = right_field_expr.is_null_safe();
                let left_prefixed = matches!(left_object, Expr::Ident(_)) && !left_safe;
                let right_prefixed = matches!(right_object, Expr::Ident(_)) && !right_safe;
                if left_prefixed != right_prefixed {
                    return false;
                }
                let equal = match (
                    member_element(left_object, left_field, left_safe, left_write, state),
                    member_element(right_object, right_field, right_safe, right_write, state),
                ) {
                    (Some(left), Some(right)) => left == right,
                    (None, None) => {
                        matches!(
                            state.infer_access_receiver(left_object, left_field.name(), left_safe),
                            falcon_analyze::ResolvedType::Dynamic
                        ) && matches!(
                            state.infer_access_receiver(
                                right_object,
                                right_field.name(),
                                right_safe
                            ),
                            falcon_analyze::ResolvedType::Dynamic
                        )
                    }
                    _ => false,
                };
                if !equal {
                    return false;
                }
                left = left_object;
                right = right_object;
                left_write = false;
                right_write = false;
            }
            _ => return false,
        }
    }
}

#[derive(PartialEq, Eq)]
enum Element<'a> {
    Local(Span),
    Prefix(&'a str),
    Declaration(DeclarationIdentity),
    Member(DeclarationIdentity, &'a str, Option<Span>),
}

fn identifier_element<'a>(
    identifier: Identifier<'a>,
    write: bool,
    state: &SemanticState<'_>,
) -> Option<Element<'a>> {
    if let Some(declaration) = state.environment.binding_declaration(identifier.name()) {
        return Some(Element::Local(declaration));
    }
    if let Some(owner) = &state.constant_owner
        && has_static_member(owner, identifier.name(), state)
    {
        return Some(Element::Member(owner.clone(), identifier.name(), None));
    }
    if let Some(receiver) = &state.enclosing_type
        && let Some(owner) = member_owner(receiver, identifier.name(), write, state)
    {
        return Some(canonical_member(owner, receiver, identifier, write, state));
    }
    // Within one library, a resolved import prefix is uniquely named, including
    // multiple imports deliberately sharing that prefix.
    if state.model.resolve_doc_name(identifier.name()) == falcon_analyze::DocName::Prefix {
        return Some(Element::Prefix(identifier.name()));
    }
    state
        .model
        .resolve_value(identifier.name())
        .or_else(|| state.model.resolve_name(identifier.name()))
        .or_else(|| state.model.resolve_extension(identifier.name()))
        .map(Element::Declaration)
}

fn member_element<'a>(
    object: Expr<'a>,
    field: Identifier<'a>,
    null_safe: bool,
    write: bool,
    state: &SemanticState<'a>,
) -> Option<Element<'a>> {
    if let Expr::Ident(prefix) = object.unparenthesized()
        && !state.environment.is_bound(prefix.name())
    {
        let qualified = falcon_analyze::QualifiedName {
            prefix: Some(prefix.name()),
            name: field.name(),
        };
        if let Some(identity) = state
            .model
            .resolve_value(qualified)
            .or_else(|| state.model.resolve_name(qualified))
            .or_else(|| state.model.resolve_extension(qualified))
        {
            return Some(Element::Declaration(identity));
        }
    }
    if let Some(owner) = static_receiver_owner(object, state)
        && has_static_member(&owner, field.name(), state)
    {
        return Some(Element::Member(owner, field.name(), None));
    }
    let receiver = state.infer_access_receiver(object, field.name(), null_safe);
    member_owner(&receiver, field.name(), write, state)
        .map(|owner| canonical_member(owner, &receiver, field, write, state))
}

fn canonical_member<'a>(
    owner: DeclarationIdentity,
    receiver: &falcon_analyze::ResolvedType,
    field: Identifier<'a>,
    write: bool,
    state: &SemanticState<'_>,
) -> Element<'a> {
    let occurrence = state
        .signatures
        .member_access_is_substituted(receiver, field.name(), write, &state.model)
        .then_some(field.span());
    Element::Member(owner, field.name(), occurrence)
}

fn has_static_member(owner: &DeclarationIdentity, name: &str, state: &SemanticState<'_>) -> bool {
    state
        .signatures
        .declared_member_facts(owner, name)
        .iter()
        .any(|fact| fact.is_static)
}

fn static_receiver_owner(
    object: Expr<'_>,
    state: &SemanticState<'_>,
) -> Option<DeclarationIdentity> {
    let name = match object.unparenthesized() {
        Expr::Ident(name) if !state.environment.is_bound(name.name()) => {
            falcon_analyze::QualifiedName::from(name.name())
        }
        Expr::Field(field) if !field.is_null_safe() => {
            let Expr::Ident(prefix) = field.object() else {
                return None;
            };
            if state.environment.is_bound(prefix.name()) {
                return None;
            }
            falcon_analyze::QualifiedName {
                prefix: Some(prefix.name()),
                name: field.field().name(),
            }
        }
        _ => return None,
    };
    state
        .model
        .resolve_name(name)
        .or_else(|| state.model.resolve_extension(name))
        .map(|owner| state.model.unaliased(owner))
}

fn member_owner(
    receiver: &falcon_analyze::ResolvedType,
    name: &str,
    write: bool,
    state: &SemanticState<'_>,
) -> Option<DeclarationIdentity> {
    if write {
        state
            .signatures
            .resolved_writable_member_owner(receiver, name, &state.model)
    } else {
        state
            .signatures
            .resolved_readable_member_owner(receiver, name, &state.model)
    }
    .or_else(|| {
        state
            .signatures
            .resolved_extension_access_owner(receiver, name, write, &state.model)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn assignment_range_includes_the_statement_semicolon() {
        let source = "int f(int x) { x += 1; return x; }";
        let diagnostics = inference::analyze(JoinReturnWithAssignment::NAME, &JoinReturnWithAssignment, Path::new("join_span.dart"),
        source,
        &FalconConfig::default(),);
        assert_eq!(diagnostics.len(), 1);
        let span = diagnostics[0].span;
        assert_eq!(span.start, source.find("x +=").unwrap());
        assert_eq!(&source[span.start..span.end], "x += 1;");
    }
}
