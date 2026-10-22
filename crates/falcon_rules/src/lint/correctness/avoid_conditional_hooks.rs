//! Flags hook calls in conditional branches, short-circuit operands, and loops.
//!
//! Hooks depend on a stable invocation order across builds. Move the hook call
//! outside the condition and put conditional work inside its callback instead.
//! Only hook-widget builds, custom useX functions, and HookBuilder builders
//! are checked; invalid hook contexts belong to the other hook rules.
//! Package hooks and custom hooks proven to compose them are recognized by
//! semantic identity. If/ternary conditions, switch subjects, left operands,
//! and for-loop initializers/iterables are unconditional. Nested functions
//! start separate execution contexts. This rule checks enclosing syntax, not
//! conditional early exits, exception paths, lazy variables, or null-aware calls.

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::flutter_hooks::HookCalls;

use crate::lint::support::hook_contexts::HookContexts;

#[derive(Default)]
pub struct AvoidConditionalHooks;

impl Rule for AvoidConditionalHooks {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            hooks: HookCalls::new(program, ctx),
            contexts: HookContexts::new(program, ctx),
            candidates: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    hooks: HookCalls<'a>,
    contexts: HookContexts<'a>,
    candidates: Vec<Expr<'a>>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        self.hooks.observe(expression, state);
        self.contexts.observe(expression, state);
        if matches!(expression, Expr::Call(_)) && self.contexts.valid_context(expression, state) {
            self.candidates.push(expression);
        }
    }

    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        self.hooks.finish();
        self.candidates
            .iter()
            .filter(|expression| self.hooks.contains(**expression) && conditional(**expression))
            .map(|expression| {
                self.ctx.diagnostic("Avoid calling hooks conditionally. Move this call outside the condition or loop.", expression.span())
            })
            .collect()
    }
}

fn conditional(expression: Expr<'_>) -> bool {
    let span = expression.span();
    for ancestor in expression.syntax().ancestors().skip(1) {
        if matches!(
            ancestor.kind(),
            NodeKind::FuncExpr
                | NodeKind::LocalFuncDecl
                | NodeKind::FunctionDecl
                | NodeKind::MethodDecl
                | NodeKind::GetterDecl
                | NodeKind::SetterDecl
                | NodeKind::OperatorDecl
                | NodeKind::ConstructorDecl
        ) {
            break;
        }
        let in_branch = match ancestor.kind() {
            NodeKind::IfStmt => {
                let branch = IfStmt::cast(ancestor).unwrap();
                branch.then_branch().span().contains(span)
                    || branch
                        .else_branch()
                        .is_some_and(|branch| branch.span().contains(span))
            }
            NodeKind::ConditionalExpr => {
                let branch = ConditionalExpr::cast(ancestor).unwrap();
                branch.then_expr().span().contains(span)
                    || branch.else_expr().span().contains(span)
            }
            NodeKind::BinaryExpr => {
                let binary = BinaryExpr::cast(ancestor).unwrap();
                matches!(
                    binary.op(),
                    BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce
                ) && binary.right().span().contains(span)
            }
            NodeKind::AssignExpr => {
                let assignment = AssignExpr::cast(ancestor).unwrap();
                assignment.op() == AssignOp::NullCoalesceEq
                    && assignment.value().span().contains(span)
            }
            NodeKind::ForStmt => {
                let loop_ = ForStmt::cast(ancestor).unwrap();
                loop_region(loop_.header(), loop_.body().span(), span)
            }
            NodeKind::WhileStmt | NodeKind::DoWhileStmt | NodeKind::SwitchExprArm => true,
            NodeKind::SwitchCase => {
                let case = SwitchCase::cast(ancestor).unwrap();
                !case
                    .cases()
                    .any(|label| matches!(label, SwitchCaseKind::Default(_)))
                    || ancestor
                        .parent()
                        .and_then(SwitchStmt::cast)
                        .is_some_and(|switch| switch.cases().nth(1).is_some())
            }
            NodeKind::IfElement => {
                let branch = IfElement::cast(ancestor).unwrap();
                branch.then_elem().span().contains(span)
                    || branch
                        .else_elem()
                        .is_some_and(|element| element.span().contains(span))
            }
            NodeKind::MapIfElement => {
                let branch = MapIfElement::cast(ancestor).unwrap();
                branch.then_entry().span().contains(span)
                    || branch
                        .else_entry()
                        .is_some_and(|entry| entry.span().contains(span))
            }
            NodeKind::ForElement => {
                let loop_ = ForElement::cast(ancestor).unwrap();
                loop_region(loop_.header(), loop_.element().span(), span)
            }
            NodeKind::MapForElement => {
                let loop_ = MapForElement::cast(ancestor).unwrap();
                loop_region(loop_.header(), loop_.entry().span(), span)
            }
            _ => false,
        };
        if in_branch {
            return true;
        }
    }
    false
}

fn loop_region(header: ForHeader<'_>, body: Span, span: Span) -> bool {
    body.contains(span)
        || header
            .condition()
            .is_some_and(|condition| condition.span().contains(span))
        || header
            .updates()
            .any(|update| update.span().contains(span))
}
