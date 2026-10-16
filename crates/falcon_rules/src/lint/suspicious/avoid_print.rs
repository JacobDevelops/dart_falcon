//! Flags calls and callback arguments resolving to `dart:core`'s `print`.
//! Lexical bindings and members declared in the enclosing body shadow it.
//! Calls guarded by Flutter's `kDebugMode` are exempt within the same function body.

use crate::lint::declarations::members_of;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct AvoidPrint;

impl Rule for AvoidPrint {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if !ctx.source.contains("print") {
            return Vec::new();
        }
        let mut presence = PrintPresence(false);
        presence.visit_program(program);
        if !presence.0 {
            return Vec::new();
        }
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        if !state
            .model
            .resolve_sdk_member("print")
            .is_some_and(|identity| identity.is_sdk("dart:core", "print"))
        {
            return Vec::new();
        }
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            declarations: &program.declarations,
            debug_calls: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct PrintPresence(bool);

impl Visitor for PrintPresence {
    fn visit_identifier(&mut self, node: &Identifier) {
        self.0 |= node.name == "print";
    }

    fn visit_expr(&mut self, node: &Expr) {
        if !self.0 {
            walk_expr(self, node);
        }
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    declarations: &'a [TopLevelDecl],
    debug_calls: Vec<Span>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        let Stmt::If(statement) = node else {
            return;
        };
        let IfCondition::Expr(Expr::Ident(identifier)) = &statement.condition else {
            return;
        };
        if identifier.name != "kDebugMode"
            || state.environment.is_bound(&identifier.name)
            || matches!(&state.enclosing_type, Some(ResolvedType::Interface { identity, .. })
                if !state.signatures.declared_member_facts(identity, &identifier.name).is_empty())
        {
            return;
        }
        if state
            .model
            .resolve_value(&identifier.name)
            .is_some_and(|identity| {
                self.ctx.identities.and_then(|identities| {
                    identities
                        .declaration_library(&identity)
                        .and_then(|library| identities.library_uri(library))
                }) == Some("package:flutter/src/foundation/constants.dart")
            })
        {
            DebugCalls(&mut self.debug_calls).visit_stmt(&statement.then_branch);
        }
    }

    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Call {
            callee, args, span, ..
        } = node
        else {
            return;
        };
        if matches!(callee.as_ref(), Expr::Ident(_) | Expr::Field { .. })
            && args.positional.iter().any(
                |argument| matches!(argument, Expr::Ident(identifier) if identifier.name == "print"),
            )
            && !state.is_function_expression_invocation(callee)
            && !state.is_constructor_invocation(callee)
        {
            self.check_arguments(args, state);
        }
        let Expr::Ident(identifier) = callee.unparenthesized() else {
            return;
        };
        if !self.debug_calls.contains(span) {
            self.check_identifier(identifier, *span, state);
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: &CascadeOp,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'_>,
    ) {
        if let CascadeOp::Call(identifier, _, args) = operation
            && !state.is_callable_value_member(receiver, &identifier.name)
        {
            self.check_arguments(args, state);
        }
    }
}

struct DebugCalls<'a>(&'a mut Vec<Span>);

impl Visitor for DebugCalls<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call { callee, span, .. } = node
            && matches!(callee.unparenthesized(), Expr::Ident(identifier) if identifier.name == "print")
        {
            self.0.push(*span);
        }
        walk_expr(self, node);
    }

    fn visit_function_body(&mut self, _node: &FunctionBody) {}
}

impl Collector<'_> {
    fn check_arguments(&mut self, args: &ArgList, state: &SemanticState<'_>) {
        for argument in &args.positional {
            if let Expr::Ident(identifier) = argument {
                self.check_identifier(identifier, identifier.span, state);
            }
        }
    }

    fn check_identifier(&mut self, identifier: &Identifier, span: Span, state: &SemanticState<'_>) {
        if identifier.name != "print"
            || state.environment.is_bound("print")
            || (state.enclosing_type.is_some()
                && self
                    .declarations
                    .iter()
                    .find(|declaration| {
                        let enclosing_span = declaration.span();
                        enclosing_span.start <= span.start && span.end <= enclosing_span.end
                    })
                    .and_then(members_of)
                    .is_some_and(|members| {
                        members.iter().any(|member| match member {
                            ClassMember::Method(method) => method.name.name == "print",
                            ClassMember::Getter(getter) => getter.name.name == "print",
                            ClassMember::Setter(setter) => setter.name.name == "print",
                            _ => false,
                        })
                    }))
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic(
            "avoid-print",
            "Avoid using print in production code; use a logging framework instead.",
            span,
        ));
    }
}
