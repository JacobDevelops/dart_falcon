//! Move final/const scalar literals into their only branch, or after an if.
//!
//! Literal initialization has no observable side effects. Declaration-resolved
//! references exclude shadowed names and captures. `ignore-const` defaults to
//! false; `ignore-after-if` defaults to true. Effectful/late/mutable variables
//! are deliberately excluded.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Skip const variables even when their literal initializer has no side effects.
    ignore_const: bool,
    /// Skip moving declarations that follow an `if` statement.
    ignore_after_if: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self { ignore_const: false, ignore_after_if: true }
    }
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct MoveVariableCloserToItsUsage;

impl Rule for MoveVariableCloserToItsUsage {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let options = crate::lint::options::get::<Options>(ctx, Self::NAME);
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            references: ctx.local_declarations(program),
            visited_references: HashSet::new(),
            candidates: Vec::new(),
            uses: HashMap::new(),
            ignore_const: options.ignore_const,
            ignore_after_if: options.ignore_after_if,
        }))
    }
}

struct Candidate<'a> {
    id: NodeId,
    span: Span,
    block: NodeId,
    declaration: LocalVarDecl<'a>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: &'a HashMap<usize, NodeId>,
    visited_references: HashSet<usize>,
    candidates: Vec<Candidate<'a>>,
    uses: HashMap<NodeId, Vec<Identifier<'a>>>,
    ignore_const: bool,
    ignore_after_if: bool,
}

/// Find a branch directly owned by the declaration's block, not a closure's if.
fn branch(mut node: SyntaxNode<'_>, declaration_block: NodeId) -> Option<Span> {
    let usage = node.span();
    while node.id() != declaration_block {
        let parent = node.parent()?;
        if matches!(parent.kind(), NodeKind::FuncExpr | NodeKind::LocalFuncDecl) {
            return None;
        }
        node = parent;
    }
    let block = Block::cast(node)?;
    block.stmts().find_map(|statement| {
        let Stmt::If(statement) = statement else {
            return None;
        };
        std::iter::once(statement.then_branch())
            .chain(statement.else_branch())
            .find_map(|branch| {
                let Stmt::Block(block) = branch else {
                    return None;
                };
                let span = block.span();
                span.contains(usage).then_some(span)
            })
    })
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, _state: &SemanticState<'a>) {
        let Stmt::LocalVar(declaration) = node else {
            return;
        };
        if !matches!(declaration.keyword(), VarKeyword::Final | VarKeyword::Const)
            || (self.ignore_const && declaration.keyword() == VarKeyword::Const)
            || declaration.is_late()
            || declaration.declarators().len() != 1
        {
            return;
        }
        let Some(block) = declaration
            .syntax()
            .parent()
            .filter(|node| node.kind() == NodeKind::Block)
        else {
            return;
        };
        let Some(variable) = declaration.declarators().next() else {
            return;
        };
        if variable.initializer().is_some_and(|value| {
            matches!(
                value.unparenthesized(),
                Expr::IntLit(_) | Expr::DoubleLit(_) | Expr::BoolLit(_) | Expr::NullLit(_)
            )
        }) {
            self.candidates.push(Candidate {
                id: variable.name().syntax().id(),
                span: variable.span(),
                block: block.id(),
                declaration,
            });
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, _state: &SemanticState<'a>) {
        if let Expr::Ident(identifier) = node
            && let Some(&id) = self.references.get(&identifier.span().start)
            && self.visited_references.insert(identifier.span().start)
        {
            self.uses.entry(id).or_default().push(identifier);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        // The lexical index also contains constant-pattern names that the
        // semantic expression callbacks do not visit. Never move a declaration
        // unless every indexed reference was accounted for.
        let unvisited: HashSet<_> = self.references
            .iter()
            .filter(|(offset, _)| !self.visited_references.contains(*offset))
            .map(|(_, id)| *id)
            .collect();
        self.candidates.iter().filter_map(|candidate| {
            if unvisited.contains(&candidate.id) {
                return None;
            }
            let uses = self.uses.get(&candidate.id)?;
            let in_branch = branch(uses.first()?.syntax(), candidate.block).is_some_and(|first| {
                uses.iter().all(|identifier| branch(identifier.syntax(), candidate.block) == Some(first))
            });
            let after_if = !self.ignore_after_if && can_move_after_if(candidate, uses);
            (in_branch || after_if).then(|| self.ctx.diagnostic(if in_branch {
                "This variable is used in only one branch. Move its declaration into that branch, closer to its uses."
            } else {
                "This variable is only used after an intervening if. Move its declaration after the if, closer to its uses."
            }, candidate.span))
        }).collect()
    }
}

fn direct_statement(mut node: SyntaxNode<'_>, block: NodeId) -> Option<Span> {
    loop {
        let parent = node.parent()?;
        if parent.id() == block {
            return Stmt::cast(node).map(|statement| statement.span());
        }
        if matches!(parent.kind(), NodeKind::FuncExpr | NodeKind::LocalFuncDecl) {
            return None;
        }
        node = parent;
    }
}

fn can_move_after_if(candidate: &Candidate<'_>, uses: &[Identifier<'_>]) -> bool {
    let Some(first) = uses
        .first()
        .and_then(|identifier| direct_statement(identifier.syntax(), candidate.block))
    else {
        return false;
    };
    if !uses.iter().all(|identifier| {
        direct_statement(identifier.syntax(), candidate.block)
            .is_some_and(|statement| statement.start >= first.start)
    }) {
        return false;
    }
    let Some(block) = candidate
        .declaration
        .syntax()
        .parent()
        .and_then(Block::cast)
    else {
        return false;
    };
    block.stmts().any(|statement| {
        matches!(statement, Stmt::If(_))
            && Span::new(candidate.declaration.span().end, first.start).contains(statement.span())
    })
}
