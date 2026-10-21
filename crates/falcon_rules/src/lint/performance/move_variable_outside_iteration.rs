//! Reports loop-local final variables whose computation is independent of the loop.
//!
//! Moving a repeated computation before the loop avoids doing the same work on
//! every iteration. This conservative rule checks direct loop-body declarations:
//! non-throwing SDK numeric addition, subtraction and multiplication, and the
//! SDK `List.reversed` view of a final local list literal. Dependencies must be
//! final locals declared before the loop, never reassigned, and used with their
//! declaration-time type rather than a loop-local promotion. Numeric expressions
//! must depend on a local, not consist entirely of literals. List sources and
//! reversed views may only be iterated or have read-only properties inspected;
//! aliases, escaping identities, mutations and closure captures are excluded.
//! Calls, constructors, fresh collection literals, user-defined operators and
//! getters, late declarations and conditional declarations are not candidates.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct MoveVariableOutsideIteration;

impl Rule for MoveVariableOutsideIteration {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            references: local_declarations(program),
            mutated: ctx.mutated_declarations(program),
            bindings: HashMap::new(),
            declaration_types: HashMap::new(),
            unsafe_uses: HashSet::new(),
            candidates: Vec::new(),
        }))
    }
}

#[derive(Clone, Copy)]
struct Binding<'a> {
    declaration: LocalVarDecl<'a>,
    variable: VarDeclarator<'a>,
}

struct Candidate {
    span: Span,
    view: Option<(NodeId, NodeId)>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: HashMap<usize, NodeId>,
    mutated: &'a HashSet<NodeId>,
    bindings: HashMap<NodeId, Binding<'a>>,
    declaration_types: HashMap<NodeId, ResolvedType>,
    unsafe_uses: HashSet<NodeId>,
    candidates: Vec<Candidate>,
}

fn callable(mut node: SyntaxNode<'_>) -> Option<NodeId> {
    loop {
        if matches!(
            node.kind(),
            NodeKind::FuncExpr
                | NodeKind::LocalFuncDecl
                | NodeKind::FunctionDecl
                | NodeKind::MethodDecl
                | NodeKind::ConstructorDecl
                | NodeKind::GetterDecl
                | NodeKind::SetterDecl
                | NodeKind::OperatorDecl
        ) {
            return Some(node.id());
        }
        node = node.parent()?;
    }
}

fn direct_loop(declaration: LocalVarDecl<'_>) -> Option<Span> {
    let block = declaration.syntax().parent()?;
    if block.kind() != NodeKind::Block {
        return None;
    }
    let parent = block.parent()?;
    matches!(
        parent.kind(),
        NodeKind::ForStmt | NodeKind::WhileStmt | NodeKind::DoWhileStmt
    )
    .then_some(parent.span())
}
fn numeric<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    matches!(state.infer(expression), ResolvedType::Interface { identity, nullable: false, .. }
        if identity.is_sdk("dart:core", "int") || identity.is_sdk("dart:core", "double"))
}

/// Only uses that cannot expose or mutate a collection's identity are allowed.
fn read_only_use<'a>(identifier: Identifier<'a>, state: &SemanticState<'a>) -> bool {
    let mut node = identifier.syntax();
    while let Some(parent) = node.parent() {
        if parent.kind() == NodeKind::ParenExpr {
            node = parent;
            continue;
        }
        if let Some(Expr::Field(field)) = Expr::cast(parent) {
            return field.object().syntax().id() == node.id()
                && (matches!(field.field().name(), "length" | "isEmpty" | "isNotEmpty")
                    || (field.field().name() == "reversed"
                        && matches!(state.infer(field.object()), ResolvedType::Interface { identity, nullable: false, .. }
                            if identity.is_sdk("dart:core", "List"))))
                && !parent.parent().is_some_and(|outer| {
                    matches!(
                        outer.kind(),
                        NodeKind::AssignExpr
                            | NodeKind::CallExpr
                            | NodeKind::PostfixIncDecExpr
                            | NodeKind::UnaryExpr
                    )
                });
        }
        return matches!(
            parent.kind(),
            NodeKind::ForInInit | NodeKind::PatternForInInit
        );
    }
    false
}

impl<'a> Collector<'a> {
    fn invariant_binding(
        &self,
        identifier: Identifier<'a>,
        loop_span: Span,
        state: &SemanticState<'a>,
    ) -> Option<Binding<'a>> {
        let id = self.references.get(&identifier.span().start)?;
        let binding = *self.bindings.get(id)?;
        (binding.declaration.keyword() == VarKeyword::Final
            && !binding.declaration.is_late()
            && binding.declaration.span().end <= loop_span.start
            && !self.mutated.contains(id)
            && self.declaration_types.get(id) == Some(&state.infer(Expr::Ident(identifier)))
            && callable(identifier.syntax()) == callable(binding.declaration.syntax()))
        .then_some(binding)
    }

    fn invariant_number(
        &self,
        expression: Expr<'a>,
        loop_span: Span,
        state: &SemanticState<'a>,
    ) -> Option<bool> {
        struct NumberCheck<'c, 'a> {
            collector: &'c Collector<'a>,
            state: &'c SemanticState<'a>,
            loop_span: Span,
            valid: bool,
            has_dependency: bool,
        }
        impl<'a> Visitor<'a> for NumberCheck<'_, 'a> {
            fn visit_expr(&mut self, node: Expr<'a>) {
                if !self.valid {
                    return;
                }
                self.valid = numeric(node, self.state)
                    && match node.unparenthesized() {
                        Expr::IntLit(_) | Expr::DoubleLit(_) => true,
                        Expr::Ident(identifier) => {
                            self.has_dependency = true;
                            self.collector
                                .invariant_binding(identifier, self.loop_span, self.state)
                                .is_some()
                        }
                        Expr::Binary(binary) => {
                            matches!(binary.op(), BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul)
                        }
                        _ => false,
                    };
                if self.valid {
                    walk_expr(self, node);
                }
            }
        }
        let mut check = NumberCheck {
            collector: self,
            state,
            loop_span,
            valid: true,
            has_dependency: false,
        };
        check.visit_expr(expression);
        check.valid.then_some(check.has_dependency)
    }

    fn check(&mut self, declaration: LocalVarDecl<'a>, state: &SemanticState<'a>) {
        for variable in declaration.declarators() {
            let ty = declaration.var_type().map_or_else(
                || {
                    variable
                        .initializer()
                        .map_or(ResolvedType::Unknown, |value| state.infer(value))
                },
                |ty| state.model.resolve_type(ty),
            );
            self.declaration_types
                .insert(variable.name().syntax().id(), ty);
            self.bindings.insert(
                variable.name().syntax().id(),
                Binding {
                    declaration,
                    variable,
                },
            );
        }
        if declaration.keyword() != VarKeyword::Final || declaration.is_late() {
            return;
        }
        let Some(loop_span) = direct_loop(declaration) else {
            return;
        };
        for variable in declaration.declarators() {
            let id = variable.name().syntax().id();
            if self.mutated.contains(&id) {
                continue;
            }
            let Some(initializer) = variable.initializer().map(Expr::unparenthesized) else {
                continue;
            };
            match initializer {
                Expr::Binary(_)
                    if self.invariant_number(initializer, loop_span, state) == Some(true) =>
                {
                    self.candidates.push(Candidate {
                        span: variable.span(),
                        view: None,
                    });
                }
                Expr::Field(field) if field.field().name() == "reversed" => {
                    let Expr::Ident(source) = field.object().unparenthesized() else {
                        continue;
                    };
                    let Some(binding) = self.invariant_binding(source, loop_span, state) else {
                        continue;
                    };
                    if !matches!(
                        binding.variable.initializer().map(Expr::unparenthesized),
                        Some(Expr::List(_))
                    ) || !matches!(state.infer(field.object()), ResolvedType::Interface { identity, nullable: false, .. }
                            if identity.is_sdk("dart:core", "List"))
                    {
                        continue;
                    }
                    self.candidates.push(Candidate {
                        span: variable.span(),
                        view: Some((id, binding.variable.name().syntax().id())),
                    });
                }
                _ => {}
            }
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        if let Stmt::LocalVar(declaration) = node {
            self.check(declaration, state);
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Ident(identifier) = node
            && let Some(&id) = self.references.get(&identifier.span().start)
            && (!read_only_use(identifier, state)
                || self.bindings.get(&id).is_some_and(|binding| {
                    callable(identifier.syntax()) != callable(binding.declaration.syntax())
                }))
        {
            self.unsafe_uses.insert(id);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.candidates
            .iter()
            .filter(|candidate| {
                candidate.view.is_none_or(|(view, source)| {
                    !self.unsafe_uses.contains(&view) && !self.unsafe_uses.contains(&source)
                })
            })
            .map(|candidate| {
                self.ctx.diagnostic("This computation is loop-invariant. Move the final variable before the loop.", candidate.span)
            })
            .collect()
    }
}
