//! Flags an assignment immediately followed by `return` of the same variable.
//!
//! When a block ends with `x = e; return x;` the two statements can be joined
//! into `return x = e;` (or simply `return e;` for a local). The rule looks at
//! the last two statements of every block: the second-to-last must be an
//! assignment (any operator), increment, decrement, or other prefix/postfix
//! expression on the same variable or property chain that the final `return`
//! yields. A run of assignments to that same target (`x = a; x += b; return
//! x;`) is left alone, as are switch-case statement lists, which are not blocks.
//!
//! ```dart
//! int f(int a) {
//!   a += 1; // flagged
//!   return a;
//! }
//! ```
//!
//! Adopted from the official `join_return_with_assignment` lint.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::semantic_scope::{self, SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct JoinReturnWithAssignment;

impl Rule for JoinReturnWithAssignment {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diagnostics: Vec::new(),
        };
        semantic_scope::visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a, 's> {
    ctx: &'a AnalyzeContext<'s>,
    diagnostics: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_, '_> {
    fn visit_block_end(&mut self, node: &Block, state: &SemanticState<'_>) {
        let [.., second_last, last] = node.stmts.as_slice() else {
            return;
        };
        let Stmt::Return(ReturnStmt {
            value: Some(returned),
            ..
        }) = last
        else {
            return;
        };
        let Some((assigned, write)) = assignment_target(second_last) else {
            return;
        };
        let third = node
            .stmts
            .len()
            .checked_sub(3)
            .and_then(|index| assignment_target(&node.stmts[index]));
        if !third.is_some_and(|(third, third_write)| {
            same_element(assigned, write, third, third_write, state)
        }) && same_element(returned, false, assigned, write, state)
            && let Stmt::Expr(statement) = second_last
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "join-return-with-assignment",
                "Assignment could be inlined in 'return' statement.",
                *statement.expr.span(),
            ));
        }
    }
}

/// The written operand of an expression statement that is an assignment, a
/// prefix expression, or a postfix expression (including `!`).
fn assignment_target(stmt: &Stmt) -> Option<(&Expr, bool)> {
    let Stmt::Expr(statement) = stmt else {
        return None;
    };
    match statement.expr.unparenthesized() {
        Expr::Assign { target, .. } => Some((target, true)),
        Expr::Unary { operand, op, .. } => Some((
            operand,
            matches!(op, UnaryOp::PlusPlus | UnaryOp::MinusMinus),
        )),
        Expr::PostfixIncDec { operand, .. } => Some((operand, true)),
        Expr::NullAssert { operand, .. } => Some((operand, false)),
        _ => None,
    }
}

/// Mirrors the SDK's canonical identifier comparison. Accessors canonicalize
/// to their declaring variable, except instantiated members retain their own
/// occurrence identity rather than canonicalizing to the generic declaration.
pub(super) fn same_element(
    left: &Expr,
    left_write: bool,
    right: &Expr,
    right_write: bool,
    state: &SemanticState<'_>,
) -> bool {
    struct SameElement<'a, 's> {
        right: &'a Expr,
        left_write: bool,
        right_write: bool,
        state: &'a SemanticState<'s>,
        equal: bool,
    }
    impl Visitor for SameElement<'_, '_> {
        fn visit_expr(&mut self, node: &Expr) {
            if !self.equal {
                return;
            }
            match (node.unparenthesized(), self.right.unparenthesized()) {
                (Expr::Ident(left), Expr::Ident(right)) => {
                    self.equal = identifier_element(left, self.left_write, self.state)
                        == identifier_element(right, self.right_write, self.state);
                }
                (
                    Expr::Field {
                        object: left_object,
                        field: left,
                        is_null_safe: left_safe,
                        ..
                    },
                    Expr::Field {
                        object: right_object,
                        field: right,
                        is_null_safe: right_safe,
                        ..
                    },
                ) => {
                    let left_prefixed =
                        matches!(left_object.as_ref(), Expr::Ident(_)) && !left_safe;
                    let right_prefixed =
                        matches!(right_object.as_ref(), Expr::Ident(_)) && !right_safe;
                    self.equal = left_prefixed == right_prefixed
                        && match (
                            member_element(
                                left_object,
                                left,
                                *left_safe,
                                self.left_write,
                                self.state,
                            ),
                            member_element(
                                right_object,
                                right,
                                *right_safe,
                                self.right_write,
                                self.state,
                            ),
                        ) {
                            (Some(left), Some(right)) => left == right,
                            (None, None) => {
                                matches!(
                                    self.state.infer_access_receiver(
                                        left_object,
                                        &left.name,
                                        *left_safe
                                    ),
                                    falcon_analyze::ResolvedType::Dynamic
                                ) && matches!(
                                    self.state.infer_access_receiver(
                                        right_object,
                                        &right.name,
                                        *right_safe
                                    ),
                                    falcon_analyze::ResolvedType::Dynamic
                                )
                            }
                            _ => false,
                        };
                    self.right = right_object;
                    self.left_write = false;
                    self.right_write = false;
                    walk_expr(self, node.unparenthesized());
                }
                _ => self.equal = false,
            }
        }
    }
    let mut comparison = SameElement {
        right,
        left_write,
        right_write,
        state,
        equal: true,
    };
    comparison.visit_expr(left);
    comparison.equal
}

#[derive(PartialEq, Eq)]
enum Element<'a> {
    Local(Span),
    Prefix(&'a str),
    Declaration(DeclarationIdentity),
    Member(DeclarationIdentity, &'a str, Option<Span>),
}

fn identifier_element<'a>(
    identifier: &'a Identifier,
    write: bool,
    state: &SemanticState<'_>,
) -> Option<Element<'a>> {
    if let Some(declaration) = state.environment.binding_declaration(&identifier.name) {
        return Some(Element::Local(declaration));
    }
    if let Some(owner) = &state.constant_owner
        && has_static_member(owner, &identifier.name, state)
    {
        return Some(Element::Member(owner.clone(), &identifier.name, None));
    }
    if let Some(receiver) = &state.enclosing_type
        && let Some(owner) = member_owner(receiver, &identifier.name, write, state)
    {
        return Some(canonical_member(owner, receiver, identifier, write, state));
    }
    // Within one library, a resolved import prefix is uniquely named, including
    // multiple imports deliberately sharing that prefix.
    if state.model.resolve_doc_name(&identifier.name) == falcon_analyze::DocName::Prefix {
        return Some(Element::Prefix(&identifier.name));
    }
    state
        .model
        .resolve_value(identifier.name.as_str())
        .or_else(|| state.model.resolve_name(identifier.name.as_str()))
        .or_else(|| state.model.resolve_extension(identifier.name.as_str()))
        .map(Element::Declaration)
}

fn member_element<'a>(
    object: &Expr,
    field: &'a Identifier,
    null_safe: bool,
    write: bool,
    state: &SemanticState<'_>,
) -> Option<Element<'a>> {
    if let Expr::Ident(prefix) = object.unparenthesized()
        && !state.environment.is_bound(&prefix.name)
    {
        let qualified = falcon_analyze::QualifiedName {
            prefix: Some(&prefix.name),
            name: &field.name,
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
        && has_static_member(&owner, &field.name, state)
    {
        return Some(Element::Member(owner, &field.name, None));
    }
    let receiver = state.infer_access_receiver(object, &field.name, null_safe);
    member_owner(&receiver, &field.name, write, state)
        .map(|owner| canonical_member(owner, &receiver, field, write, state))
}

fn canonical_member<'a>(
    owner: DeclarationIdentity,
    receiver: &falcon_analyze::ResolvedType,
    field: &'a Identifier,
    write: bool,
    state: &SemanticState<'_>,
) -> Element<'a> {
    let occurrence = state
        .signatures
        .member_access_is_substituted(receiver, &field.name, write, &state.model)
        .then_some(field.span);
    Element::Member(owner, &field.name, occurrence)
}

fn has_static_member(owner: &DeclarationIdentity, name: &str, state: &SemanticState<'_>) -> bool {
    state
        .signatures
        .declared_member_facts(owner, name)
        .iter()
        .any(|fact| fact.is_static)
}

fn static_receiver_owner(object: &Expr, state: &SemanticState<'_>) -> Option<DeclarationIdentity> {
    let name = match object.unparenthesized() {
        Expr::Ident(name) if !state.environment.is_bound(&name.name) => {
            falcon_analyze::QualifiedName::from(name.name.as_str())
        }
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } => {
            let Expr::Ident(prefix) = object.as_ref() else {
                return None;
            };
            if state.environment.is_bound(&prefix.name) {
                return None;
            }
            falcon_analyze::QualifiedName {
                prefix: Some(&prefix.name),
                name: &field.name,
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
