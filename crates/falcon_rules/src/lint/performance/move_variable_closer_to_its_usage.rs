//! Move final/const scalar literals into their only branch, or after an if.
//!
//! Literal initialization has no observable side effects. Declaration-resolved
//! references exclude shadowed names and captures. `ignore_const` defaults to
//! false; `ignore_after_if` defaults to true. Effectful/late/mutable variables
//! are deliberately excluded.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct MoveVariableCloserToItsUsage;

impl Rule for MoveVariableCloserToItsUsage {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let options = crate::lint::options::for_rule(ctx, MoveVariableCloserToItsUsage::NAME);
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            references: local_declarations(program),
            candidates: Vec::new(),
            uses: HashMap::new(),
            ignore_const: options
                .and_then(|value| value.get("ignore_const"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false),
            ignore_after_if: options
                .and_then(|value| value.get("ignore_after_if"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(true),
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
    references: HashMap<usize, NodeId>,
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
                (span.start <= usage.start && usage.end <= span.end).then_some(span)
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
            && let Some(id) = self.references.remove(&identifier.span().start)
        {
            self.uses.entry(id).or_default().push(identifier);
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        // The lexical index also contains constant-pattern names that the
        // semantic expression callbacks do not visit. Never move a declaration
        // unless every indexed reference was accounted for.
        let unvisited: HashSet<_> = self.references.values().copied().collect();
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
            && statement.span().start >= candidate.declaration.span().end
            && statement.span().end <= first.start
    })
}
