//! Flags calls that resolve to `dart:core`'s top-level `print` function.
//! Lexical bindings and members declared in the enclosing body shadow it.

use crate::lint::declarations::members_of;
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::{AnalyzeContext, Rule};
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
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Expr::Call { callee, span, .. } = node else {
            return;
        };
        let Expr::Ident(identifier) = callee.unparenthesized() else {
            return;
        };
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
            *span,
        ));
    }
}
