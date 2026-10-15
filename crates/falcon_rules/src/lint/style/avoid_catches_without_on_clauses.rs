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

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, for_each_expr, walk_expr, walk_stmt};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct AvoidCatchesWithoutOnClauses;

impl Rule for AvoidCatchesWithoutOnClauses {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        for catch in &statement.catches {
            if catch.exception_type.is_some() {
                continue;
            }
            let Some(exception) = &catch.exception_var else {
                continue;
            };
            let mut uses = ValidUse {
                exception: &exception.name,
                state,
                can_rethrow: true,
                has_valid_use: false,
            };
            for statement in &catch.body.stmts {
                uses.visit_stmt(statement);
            }
            if uses.has_valid_use {
                continue;
            }
            let span = self
                .ctx
                .source
                .get(catch.span.start..catch.span.end)
                .and_then(|text| text.find("catch"))
                .map_or(catch.span, |offset| {
                    let start = catch.span.start + offset;
                    Span::new(start, start + "catch".len())
                });
            self.diagnostics.push(self.ctx.diagnostic(
                "avoid-catches-without-on-clauses",
                "Catch clause should use 'on' to specify the type of exception being caught.",
                span,
            ));
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

impl ValidUse<'_, '_> {
    fn mentions_exception(&self, expr: &Expr) -> bool {
        let mut found = false;
        for_each_expr(expr, &mut |node| {
            found |= matches!(node.unparenthesized(), Expr::Ident(identifier) if identifier.name == self.exception);
        });
        found
    }

    fn check_arguments(&mut self, args: &ArgList) {
        if args
            .positional
            .iter()
            .chain(args.named.iter().map(|argument| &argument.value))
            .any(|argument| self.mentions_exception(argument))
        {
            self.has_valid_use = true;
        }
    }

    /// Whether a call hands its arguments on as an error.
    fn forwards_error(&self, callee: &Expr, type_args: &[DartType], args: &ArgList) -> bool {
        if let Expr::Field { object, field, .. } = callee.unparenthesized() {
            // `Future<T>.error(...)` names the class through an instantiation.
            let class = match object.unparenthesized() {
                Expr::GenericInstantiation { target, .. } => target.as_ref(),
                other => other,
            };
            if let Expr::Ident(target) = class.unparenthesized() {
                let is_class = |name: &str| target.name == name && !self.is_project_name(name);
                if (field.name == "error" && is_class("Future"))
                    || (field.name == "reportError" && target.name == "FlutterError")
                    || (field.name == "throwWithStackTrace" && is_class("Error"))
                {
                    return true;
                }
            }
            if field.name == "completeError" {
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

    fn is_project_name(&self, name: &str) -> bool {
        self.state.environment.is_bound(name)
            || matches!(
                self.state.model.resolve_name(name),
                Some(DeclarationIdentity::Project { .. })
            )
    }
}

impl Visitor for ValidUse<'_, '_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(statement) = node {
            for nested in &statement.body.stmts {
                self.visit_stmt(nested);
            }
            for catch in &statement.catches {
                self.can_rethrow = false;
                for nested in &catch.body.stmts {
                    self.visit_stmt(nested);
                }
            }
            if let Some(finally) = &statement.finally {
                for nested in &finally.stmts {
                    self.visit_stmt(nested);
                }
            }
            return;
        }
        if matches!(node, Stmt::Rethrow(_)) {
            self.has_valid_use = self.can_rethrow;
        } else if let Stmt::Throw(statement) = node
            && self.mentions_exception(&statement.value)
        {
            self.has_valid_use = true;
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Throw { expr, .. } if self.mentions_exception(expr) => {
                self.has_valid_use = true;
            }
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } if self.forwards_error(callee, type_args, args) => self.check_arguments(args),
            Expr::New {
                dart_type: DartType::Named(named),
                constructor_name: Some(constructor),
                args,
                ..
            } if constructor.name == "error"
                && matches!(named.segments.last(), Some(name) if name.name == "Future")
                && !self.is_project_name("Future") =>
            {
                self.check_arguments(args);
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
