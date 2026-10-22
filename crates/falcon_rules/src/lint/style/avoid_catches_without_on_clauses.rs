//! Avoid `catch` clauses without an `on` type.
//!
//! A bare `catch (e)` swallows everything, including programming errors that
//! should surface. Name the exceptions you can handle:
//!
//! ```dart
//! try { work(); } catch (e) { log(e); } // BAD
//! try { work(); } on IOException catch (e) { log(e); } // GOOD
//! ```
//!
//! A bare catch is allowed when its body hands the caught object on: a
//! `rethrow` (before any nested catch clause), a `throw` mentioning it, or
//! passing it to `Future.error`, `Completer.completeError`,
//! `FlutterError.reportError`, `Error.throwWithStackTrace`, or another call
//! whose resolved return type is `Never`. A `completeError` call on a receiver
//! whose type cannot be resolved also counts, so unknown receivers stay silent.
//!
//! Adopted from the official `avoid_catches_without_on_clauses` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
    expression_segments,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::TokenKind;
use falcon_syntax::cst::visit::{Visitor, for_each_expr, walk_expr, walk_stmt};
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use crate::lint::flutter::value_name_is_shadowed;

#[derive(Default)]
pub struct AvoidCatchesWithoutOnClauses;

impl Rule for AvoidCatchesWithoutOnClauses {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        crate::lint::support::preparation::prepare!(Semantic, Collector, ctx, diagnostics)
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        for catch in statement.catches() {
            if catch.exception_type().is_some() {
                continue;
            }
            let Some(exception) = catch.exception_var() else {
                continue;
            };
            let mut uses = ValidUse {
                exception: exception.name(),
                state,
                can_rethrow: true,
                has_valid_use: false,
            };
            for statement in catch.body().stmts() {
                uses.visit_stmt(statement);
            }
            if uses.has_valid_use {
                continue;
            }
            let span = self
                .ctx
                .tree()
                .keywords_in(self.ctx.source(), catch.span(), TokenKind::Catch)
                .next()
                .map_or(catch.span(), |token| {
                    Span::new(token.offset(), token.offset() + token.len())
                });
            self.diagnostics.push(self.ctx.diagnostic("Catch clause should use `on` to specify the type of exception being caught.", span));
        }
    }
}

/// Mirrors the official visitor: walks the catch body in source order, so a
/// `rethrow` only counts until the first nested catch clause is entered.
struct ValidUse<'s, 'a> {
    exception: &'s str,
    state: &'s SemanticState<'a>,
    can_rethrow: bool,
    has_valid_use: bool,
}

impl<'s, 'a> ValidUse<'s, 'a> {
    fn mentions_exception(&self, expr: Expr<'a>) -> bool {
        let mut found = false;
        for_each_expr(expr, &mut |node| {
            found |= matches!(node.unparenthesized(), Expr::Ident(identifier) if identifier.name() == self.exception);
        });
        found
    }

    fn check_arguments(&mut self, args: ArgList<'a>) {
        if args
            .positional()
            .chain(args.named().map(|argument| argument.value()))
            .any(|argument| self.mentions_exception(argument))
        {
            self.has_valid_use = true;
        }
    }

    /// Whether a call hands its arguments on as an error.
    fn forwards_error(
        &self,
        callee: Expr<'a>,
        type_args: AstChildren<'a, DartType<'a>>,
        args: ArgList<'a>,
    ) -> bool {
        if let Expr::Field(field) = callee.unparenthesized() {
            let object = field.object();
            // `Future<T>.error(...)` names the class through an instantiation.
            let class = match object.unparenthesized() {
                Expr::GenericInstantiation(instantiation) => instantiation.target(),
                other => other,
            };
            if let Some(segments) = expression_segments(class, false)
                && !value_name_is_shadowed(segments[0], self.state)
            {
                let identity = self
                    .state
                    .model
                    .resolve_dotted(&segments)
                    .map(|identity| self.state.model.unaliased(identity));
                // Missing imports or parts leave the class unresolved; fall back to its spelling.
                let is = |class: &str, matches: &dyn Fn(&DeclarationIdentity) -> bool| {
                    identity.as_ref().map_or(segments == [class], matches)
                };
                let forwards = match field.field().name() {
                    "error" => is("Future", &|identity| identity.is_sdk("dart:async", "Future")),
                    "reportError" => is("FlutterError", &|identity| {
                        identity.is_package("flutter", "FlutterError")
                    }),
                    "throwWithStackTrace" => {
                        is("Error", &|identity| identity.is_sdk("dart:core", "Error"))
                    }
                    _ => false,
                };
                if forwards {
                    return true;
                }
            }
            if field.field().name() == "completeError" {
                let receiver = self.state.infer(object);
                if self.state.signatures.interface_check_sdk(
                    &receiver,
                    "dart:async",
                    "Completer",
                    &self.state.model,
                ) != Truth::No
                {
                    return true;
                }
            }
        }
        self.state
            .call_return_type(callee, type_args, args, None)
            .is_some_and(|return_type| matches!(return_type, ResolvedType::Never))
    }

}

impl<'a> Visitor<'a> for ValidUse<'_, 'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>) {
        if let Stmt::TryCatch(statement) = node {
            for nested in statement.body().stmts() {
                self.visit_stmt(nested);
            }
            for catch in statement.catches() {
                self.can_rethrow = false;
                for nested in catch.body().stmts() {
                    self.visit_stmt(nested);
                }
            }
            if let Some(finally) = statement.finally() {
                for nested in finally.stmts() {
                    self.visit_stmt(nested);
                }
            }
            return;
        }
        if matches!(node, Stmt::Rethrow(_)) {
            self.has_valid_use = self.can_rethrow;
        } else if let Stmt::Throw(statement) = node
            && self.mentions_exception(statement.value())
        {
            self.has_valid_use = true;
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: Expr<'a>) {
        let future_error = match node {
            Expr::New(new) => {
                (new.constructor_name().is_some_and(|name| name.name() == "error")
                    && self.state.model.resolve_type_in(new.dart_type(), &self.state.type_parameters)
                        .interface("dart:async", "Future"))
                    .then_some(new.args())
            }
            _ => None,
        };
        if let Some(args) = future_error {
            self.check_arguments(args);
        }
        match node {
            Expr::Throw(throw) if self.mentions_exception(throw.expr()) => {
                self.has_valid_use = true;
            }
            Expr::Call(call)
                if self.forwards_error(call.callee(), call.type_args(), call.args()) =>
            {
                self.check_arguments(call.args());
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
