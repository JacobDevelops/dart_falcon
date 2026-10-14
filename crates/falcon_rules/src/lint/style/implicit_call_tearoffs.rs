//! Explicitly tear off callable objects' call methods.

use falcon_analyze::{AnalyzeContext, MemberKind, ResolvedType, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::Expr;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct ImplicitCallTearoffs;

impl Rule for ImplicitCallTearoffs {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if matches!(node, Expr::Field { field, .. } if field.name == "call") {
            return;
        }
        let Some(expected) = state.expected() else {
            return;
        };
        if !matches!(expected, ResolvedType::Function { .. })
            && !expected.interface("dart:core", "Function")
        {
            return;
        }
        let receiver = state.infer(node);
        if matches!(
            receiver,
            ResolvedType::Function { .. } | ResolvedType::Unknown | ResolvedType::Dynamic
        ) || !state
            .signatures
            .resolved_member_facts(&receiver, "call", &state.model)
            .is_some_and(|facts| {
                facts
                    .iter()
                    .any(|fact| fact.kind == MemberKind::Method && !fact.is_static)
            })
        {
            return;
        }
        let span = node.span();
        self.diags.push(self.ctx.diagnostic(
            "implicit-call-tearoffs",
            "Explicitly tear off the 'call' method.",
            *span,
        ));
    }
}
