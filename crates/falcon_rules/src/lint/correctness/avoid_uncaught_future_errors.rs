//! Flags local futures first awaited after another await inside a try/catch.
//!
//! A future can complete with an unhandled error while execution is suspended,
//! before the later await attaches the surrounding catch's error handler. Await
//! it immediately or attach an error handler before the first async gap.
//!
//! This conservative analysis follows straight-line statements directly in a
//! try body with at least one catch. It tracks resolved Future-returning calls
//! and constructors assigned to locals, and reports the later awaited local,
//! not discarded futures. Unknown base types and FutureOr are excluded. Early
//! catchError, onError, and then(onError:) calls are exempt, including handlers
//! attached in a separate statement or initializer. Parentheses are transparent.
//! Branches, loops, nested blocks, local declarations of functions, and other
//! expression statements end tracking: no control-flow or alias assumptions are
//! made across them. Late and multi-variable declarations are excluded. Nested
//! functions and shadowed locals cannot consume an outer candidate. Only root
//! await expressions in expression, return, and local-variable statements count
//! as gaps; awaits nested in other expressions are outside the matching limits.
//! A reference inside a composite await operand conservatively consumes that
//! candidate, covering combinators and forwarding methods without alias guesses.
//! A handler protects only the call it terminates, not subsequent chained calls.
//! When intermediate types are unavailable, only SDK Future method chains
//! (then/catchError/onError/timeout/whenComplete) on a proven Future base qualify.

use std::iter::Peekable;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUncaughtFutureErrors;

impl Rule for AvoidUncaughtFutureErrors {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            regions: Vec::new(),
            diagnostics: Vec::new(),
            consuming: None,
            direct_await: None,
        }))
    }
}

struct Candidate {
    declaration: Span,
    crossed_gap: bool,
}

struct Region<'a> {
    body: Span,
    statements: Peekable<AstChildren<'a, Stmt<'a>>>,
    candidates: Vec<Candidate>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    regions: Vec<Region<'a>>,
    diagnostics: Vec<Diagnostic>,
    consuming: Option<Span>,
    direct_await: Option<Span>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        // Advance only on a direct child: callbacks in nested callables and
        // compound statements never alter this region's sequence.
        if let Some(region) = self.regions.iter_mut().rev().find_map(|region| {
            region
                .statements
                .peek()
                .is_some_and(|next| next.span() == node.span())
                .then_some(region)
        }) {
            region.statements.next();
            self.direct_await = None;
            match node {
                Stmt::LocalVar(declaration) => {
                    if declaration.is_late() || declaration.declarators().len() != 1 {
                        region.candidates.clear();
                        return;
                    }
                    for variable in declaration.declarators() {
                        let Some(initializer) = variable.initializer() else {
                            continue;
                        };
                        if let Expr::Await(awaited) = initializer.unparenthesized() {
                            self.direct_await = Some(awaited.span());
                        }
                        if matches!(initializer.unparenthesized(), Expr::Ident(_)) {
                            // An alias may acquire a handler under another name.
                            region.candidates.clear();
                        }
                        observe_expression(
                            initializer,
                            &mut region.candidates,
                            state,
                            self.ctx,
                            &mut self.diagnostics,
                        );
                        if matches!(initializer.unparenthesized(), Expr::Call(_) | Expr::New(_))
                            && !has_error_handler(initializer, state)
                            && future_expression(initializer, state)
                        {
                            region.candidates.push(Candidate {
                                declaration: variable.name().span(),
                                crossed_gap: false,
                            });
                        }
                    }
                }
                Stmt::Expr(statement) => {
                    let expression = statement.expr();
                    if let Expr::Await(awaited) = expression.unparenthesized() {
                        self.direct_await = Some(awaited.span());
                    }
                    if !matches!(expression.unparenthesized(), Expr::Await(_))
                        && !has_error_handler(expression, state)
                    {
                        region.candidates.clear();
                    } else {
                        observe_expression(
                            expression,
                            &mut region.candidates,
                            state,
                            self.ctx,
                            &mut self.diagnostics,
                        );
                    }
                }
                Stmt::Return(statement) => {
                    if let Some(expression) = statement.value() {
                        if let Expr::Await(awaited) = expression.unparenthesized() {
                            self.direct_await = Some(awaited.span());
                        }
                        observe_expression(
                            expression,
                            &mut region.candidates,
                            state,
                            self.ctx,
                            &mut self.diagnostics,
                        );
                    }
                    region.candidates.clear();
                }
                _ => region.candidates.clear(),
            }
        }
        if let Stmt::TryCatch(statement) = node
            && statement.catches().next().is_some()
        {
            self.regions.push(Region {
                body: statement.body().span(),
                statements: statement.body().stmts().peekable(),
                candidates: Vec::new(),
            });
        }
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if self.consuming.is_some_and(|operand| {
            node.span().start < operand.start || node.span().end > operand.end
        }) {
            self.consuming = None;
        }
        if let Expr::Await(awaited) = node
            && self.direct_await == Some(awaited.span())
            && !matches!(awaited.expr().unparenthesized(), Expr::Ident(_))
            && self.consuming.is_none()
        {
            self.consuming = Some(awaited.expr().span());
        }
        if self.consuming.is_some()
            && let Expr::Ident(name) = node
            && let Some(declaration) = state.environment.binding_declaration(name.name())
        {
            for region in &mut self.regions {
                region
                    .candidates
                    .retain(|candidate| candidate.declaration != declaration);
            }
        }
    }

    fn visit_block_end(&mut self, node: Block<'a>, _state: &SemanticState<'a>) {
        if self
            .regions
            .last()
            .is_some_and(|region| region.body == node.span())
        {
            self.regions.pop();
        }
    }
}

fn observe_expression<'a>(
    expression: Expr<'a>,
    candidates: &mut Vec<Candidate>,
    state: &SemanticState<'a>,
    ctx: &AnalyzeContext<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let expression = expression.unparenthesized();
    if let Expr::Await(awaited) = expression {
        if let Expr::Ident(name) = awaited.expr().unparenthesized()
            && let Some(declaration) = state.environment.binding_declaration(name.name())
            && let Some(index) = candidates
                .iter()
                .position(|candidate| candidate.declaration == declaration)
        {
            let candidate = candidates.remove(index);
            if candidate.crossed_gap {
                diagnostics.push(ctx.diagnostic(
                    "avoid-uncaught-future-errors",
                    "This future is awaited after an async gap; await it immediately or attach an error handler first.",
                    name.span(),
                ));
            }
        }
        for candidate in candidates {
            candidate.crossed_gap = true;
        }
    } else if let Some(receiver) = error_handler_receiver(expression, state) {
        let Expr::Ident(name) = receiver.unparenthesized() else {
            candidates.clear();
            return;
        };
        let Some(declaration) = state.environment.binding_declaration(name.name()) else {
            candidates.clear();
            return;
        };
        if candidates
            .iter()
            .any(|candidate| candidate.declaration == declaration)
        {
            candidates
                .retain(|candidate| candidate.declaration != declaration || candidate.crossed_gap);
        } else {
            candidates.clear();
        }
    }
}

fn has_error_handler<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    error_handler_receiver(expression, state).is_some()
}

fn error_handler_receiver<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> Option<Expr<'a>> {
    let Expr::Call(call) = expression.unparenthesized() else {
        return None;
    };
    let Expr::Field(field) = call.callee().unparenthesized() else {
        return None;
    };
    let has_handler = match field.field().name() {
        "catchError" | "onError" => call
            .args()
            .positional()
            .next()
            .is_some_and(|argument| !matches!(argument.unparenthesized(), Expr::NullLit(_))),
        "then" => call.args().named().any(|argument| {
            argument.name().name() == "onError"
                && !matches!(argument.value().unparenthesized(), Expr::NullLit(_))
        }),
        _ => false,
    };
    (has_handler && future_expression(field.object(), state)).then_some(field.object())
}

fn future_expression<'a>(expression: Expr<'a>, state: &SemanticState<'a>) -> bool {
    let ty = state.infer(expression);
    if state.signatures.implements_sdk(&ty, "dart:async", "Future") == Truth::Yes {
        return true;
    }
    if !matches!(ty, falcon_analyze::ResolvedType::Unknown) {
        return false;
    }
    let mut current = expression.unparenthesized();
    while let Expr::Call(call) = current {
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return false;
        };
        if !matches!(
            field.field().name(),
            "then" | "catchError" | "onError" | "timeout" | "whenComplete"
        ) {
            return false;
        }
        current = field.object().unparenthesized();
        let receiver = state.infer(current);
        if receiver.interface("dart:async", "Future") {
            return true;
        }
        if !matches!(receiver, falcon_analyze::ResolvedType::Unknown) {
            return false;
        }
    }
    false
}
