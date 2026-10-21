//! Flags loop conditions whose local scalar inputs are never modified by the loop.
//!
//! A forgotten update can make a `while`, `do` or condition-bearing `for` loop
//! repeat forever. Conditions must consist of resolved local scalar bindings,
//! literals, operators, and immutable String length/emptiness properties. Any
//! write to a referenced declaration in the body, condition or for-update avoids
//! a report; initialization and writes after the loop do not count. Shadowed
//! names are distinct bindings. Literal-only conditions belong to other rules.
//!
//! Unknown types, mutable-object properties, calls in conditions, captured
//! writes, and loops containing a possible break, labeled continue, return,
//! throw, yield, await, or resolved Never-returning call are deliberately ignored.
//! Exit checks include nested constructs; this is not an infinite-loop proof.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::ResolvedType;
use falcon_analyze::resolve::lexical_usage::{LocalWrite, local_declarations};
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::for_each_expr;
use falcon_syntax::cst::*;


#[derive(Default)]
pub struct AvoidUnmodifiedLoopCondition;

impl Rule for AvoidUnmodifiedLoopCondition {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            declarations: local_declarations(program),
            writes: ctx.local_writes(program),
            captured: HashSet::new(),
            loops: Vec::new(),
        }))
    }
}

struct Loop<'a> {
    span: Span,
    repeated: Span,
    condition: Expr<'a>,
    inputs: Vec<NodeId>,
    stable: bool,
    exits: bool,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    declarations: HashMap<usize, NodeId>,
    writes: &'a [LocalWrite],
    captured: HashSet<NodeId>,
    loops: Vec<Loop<'a>>,
}

impl<'a> Collector<'a> {
    fn capture(&mut self, span: Span) {
        for write in self.writes {
            if contains(span, write.span) {
                self.captured.insert(write.declaration);
            }
        }
    }

    fn condition_is_stable(
        &self,
        expression: Expr<'a>,
        repeated: Span,
        inputs: &mut Vec<NodeId>,
        state: &SemanticState<'a>,
    ) -> bool {
        let mut stable = true;
        for_each_expr(expression, &mut |expression| {
            if !stable {
                return;
            }
            stable = match expression {
                Expr::Ident(name) => {
                    let Some(declaration) = self.declarations.get(&name.span().start) else {
                        stable = false;
                        return;
                    };
                    let ty = state.infer(expression);
                    inputs.push(*declaration);
                    ["bool", "int", "double", "num", "String"]
                        .iter()
                        .any(|name| ty.interface("dart:core", name))
                        && !self.writes.iter().any(|write| {
                            write.declaration == *declaration && contains(repeated, write.span)
                        })
                }
                Expr::BoolLit(_)
                | Expr::IntLit(_)
                | Expr::DoubleLit(_)
                | Expr::NullLit(_)
                | Expr::Paren(_)
                | Expr::Binary(_) => true,
                Expr::StringLit(string) => string.interpolations().next().is_none(),
                Expr::Unary(node) => !matches!(node.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus),
                Expr::Field(field) => {
                    matches!(field.field().name(), "length" | "isEmpty" | "isNotEmpty")
                        && state.infer(field.object()).interface("dart:core", "String")
                }
                _ => false,
            };
        });
        stable
    }
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.loops
            .iter()
            .filter(|candidate| {
                candidate.stable
                    && !candidate.exits
                    && !candidate.inputs.is_empty()
                    && !candidate.inputs.iter().any(|input| self.captured.contains(input))
            })
            .map(|candidate| {
                self.ctx.diagnostic("None of the local variables in this condition are modified by the loop. Check for a missing update.", candidate.condition.span())
            })
            .collect()
    }

    fn visit_stmt(&mut self, statement: Stmt<'a>, _state: &SemanticState<'a>) {
        if let Stmt::LocalFunc(function) = statement {
            self.capture(function.span());
        }
        let condition = match statement {
            Stmt::While(node) => Some((node.condition(), statement.span())),
            Stmt::DoWhile(node) => Some((node.condition(), statement.span())),
            Stmt::For(node) => node.header().condition().map(|condition| {
                (
                    condition,
                    Span::new(condition.span().start, statement.span().end),
                )
            }),
            _ => None,
        };
        if let Some((condition, repeated)) = condition {
            self.loops.push(Loop {
                span: statement.span(),
                repeated,
                condition,
                inputs: Vec::new(),
                stable: false,
                exits: false,
            });
        }
        if matches!(
            statement,
            Stmt::Break(_) | Stmt::Return(_) | Stmt::Throw(_) | Stmt::Rethrow(_) | Stmt::Yield(_)
        ) || matches!(statement, Stmt::Continue(node) if node.label().is_some())
        {
            for candidate in &mut self.loops {
                if contains(candidate.span, statement.span()) {
                    candidate.exits = true;
                }
            }
        }
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        if matches!(expression, Expr::FuncExpr(_)) {
            self.capture(expression.span());
        }
        if let Some(index) = self
            .loops
            .iter()
            .position(|candidate| candidate.condition.syntax().id() == expression.syntax().id())
        {
            let mut inputs = Vec::new();
            let stable = self.condition_is_stable(
                expression,
                self.loops[index].repeated,
                &mut inputs,
                state,
            );
            self.loops[index].inputs = inputs;
            self.loops[index].stable = stable;
        }
        let within_loop = self
            .loops
            .iter()
            .any(|candidate| !candidate.exits && contains(candidate.span, expression.span()));
        if within_loop
            && (matches!(expression, Expr::Await(_) | Expr::Throw(_))
                || matches!(expression, Expr::Call(_))
                    && matches!(state.infer(expression), ResolvedType::Never))
        {
            for candidate in &mut self.loops {
                if contains(candidate.span, expression.span()) {
                    candidate.exits = true;
                }
            }
        }
    }
}
