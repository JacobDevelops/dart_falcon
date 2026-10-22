//! Flags calls and callback arguments resolving to `dart:core`'s `print`.
//! Lexical bindings and members declared in the enclosing body shadow it.
//! Calls guarded by Flutter's `kDebugMode` are exempt within the same function body.
//! Diagnostics underline the `print` identifier, not the invocation or its arguments.

use crate::lint::declarations::members_of;
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidPrint;

impl Rule for AvoidPrint {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !ctx.source().contains("print") {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let Some(identities) = ctx.identities() else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = falcon_analyze::SemanticModel::new(
            ctx.file_path(),
            ctx.parsed(),
            identities,
            ctx.types(),
        );
        if !model
            .resolve_sdk_member("print")
            .is_some_and(|identity| identity.is_sdk("dart:core", "print"))
        {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Collector {
            diagnostics: Vec::new(),
            ctx,
            declarations: program.declarations(),
            debug_calls: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    declarations: AstChildren<'a, TopLevelDecl<'a>>,
    debug_calls: Vec<Span>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::If(statement) = node else {
            return;
        };
        let IfCondition::Expr(Expr::Ident(identifier)) = statement.condition() else {
            return;
        };
        if identifier.name() != "kDebugMode"
            || state.environment.is_bound(identifier.name())
            || matches!(&state.enclosing_type, Some(ResolvedType::Interface { identity, .. })
                if !state.signatures.declared_member_facts(identity, identifier.name()).is_empty())
        {
            return;
        }
        if state
            .model
            .resolve_value(identifier.name())
            .is_some_and(|identity| {
                self.ctx.identities().and_then(|identities| {
                    identities
                        .declaration_library(&identity)
                        .and_then(|library| identities.library_uri(library))
                }) == Some("package:flutter/src/foundation/constants.dart")
            })
        {
            DebugCalls(&mut self.debug_calls).visit_stmt(statement.then_branch());
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Call(call) = node else {
            return;
        };
        let callee = call.callee();
        let args = call.args();
        if matches!(callee, Expr::Ident(_) | Expr::Field(_))
            && args.positional().any(
                |argument| matches!(argument, Expr::Ident(identifier) if identifier.name() == "print"),
            )
            && !state.is_function_expression_invocation(callee)
            && !state.is_constructor_invocation(callee)
        {
            self.check_arguments(args, state);
        }
        let Expr::Ident(identifier) = callee.unparenthesized() else {
            return;
        };
        if !self.debug_calls.contains(&call.span()) {
            self.check_identifier(identifier, identifier.span(), state);
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        if let CascadeOp::Call(call) = operation
            && !state.is_callable_value_member(receiver, call.name().name())
        {
            self.check_arguments(call.args(), state);
        }
    }
}

struct DebugCalls<'a>(&'a mut Vec<Span>);

impl<'a> Visitor<'a> for DebugCalls<'_> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        if let Expr::Call(call) = node
            && matches!(call.callee().unparenthesized(), Expr::Ident(identifier) if identifier.name() == "print")
        {
            self.0.push(call.span());
        }
        walk_expr(self, node);
    }

    fn visit_function_body(&mut self, _node: FunctionBody<'a>) {}
}

impl Collector<'_> {
    fn check_arguments(&mut self, args: ArgList<'_>, state: &SemanticState<'_>) {
        for argument in args.positional() {
            if let Expr::Ident(identifier) = argument {
                self.check_identifier(identifier, identifier.span(), state);
            }
        }
    }

    fn check_identifier(
        &mut self,
        identifier: Identifier<'_>,
        span: Span,
        state: &SemanticState<'_>,
    ) {
        if identifier.name() != "print"
            || state.environment.is_bound("print")
            || (state.enclosing_type.is_some()
                && self
                    .declarations
                    .clone()
                    .find(|declaration| {
                        let enclosing_span = declaration.span();
                        enclosing_span.contains(span)
                    })
                    .and_then(members_of)
                    .is_some_and(|mut members| {
                        members.any(|member| match member {
                            ClassMember::Method(method) => method.name().name() == "print",
                            ClassMember::Getter(getter) => getter.name().name() == "print",
                            ClassMember::Setter(setter) => setter.name().name() == "print",
                            _ => false,
                        })
                    }))
        {
            return;
        }
        self.diagnostics.push(self.ctx.diagnostic("Avoid using `print` in production code; use a logging framework instead.", span));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn calls_report_the_print_identifier_only() {
        let source = "void f() { print('hello'); print('second'); }";
        let diagnostics = inference::analyze(AvoidPrint::NAME, &AvoidPrint, Path::new("print_spans.dart"),
        source,
        &FalconConfig::default(),);
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics.iter().all(|diagnostic| diagnostic.message
            == "Avoid using `print` in production code; use a logging framework instead."));
        for (diagnostic, (start, _)) in diagnostics.iter().zip(source.match_indices("print")) {
            assert_eq!(diagnostic.span.start, start);
            assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "print");
        }
    }
}
