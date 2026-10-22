//! Reports direct self-calls in functions and methods, including guarded recursion.
//!
//! Local declaration identities exclude shadowed call targets. Calls on another
//! receiver, getters, extension dispatch, and calls inside anonymous closures
//! are deliberately excluded rather than guessed to be recursive.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidRecursiveCalls;

impl Rule for AvoidRecursiveCalls {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            declarations: ctx.local_declarations(program),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    declarations: &'a HashMap<usize, NodeId>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, expression: Expr<'a>) -> StructuralTraversal {
        if let Expr::Call(call) = expression
            && let Some((name, local, method)) = enclosing_callable(call)
        {
            let recursive = match call.callee().unparenthesized() {
                Expr::Ident(target) if target.name() == name.name() => {
                    let binding = self.declarations.get(&target.span().start);
                    if local {
                        binding == Some(&name.syntax().id())
                    } else {
                        binding.is_none()
                    }
                }
                Expr::Field(target) => method && target.field().name() == name.name()
                    && matches!(target.object().unparenthesized(), Expr::This(_)),
                _ => false,
            };
            if recursive {
                self.diags.push(self.ctx.diagnostic(
                    "Avoid calling the same function recursively; consider an iterative implementation.",
                    call.span(),
                ));
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn enclosing_callable(call: CallExpr<'_>) -> Option<(Identifier<'_>, bool, bool)> {
    let mut ancestor = call.syntax().parent();
    while let Some(node) = ancestor {
        if let Some(function) = FunctionDecl::cast(node) {
            return matches!(function.kind(), FunctionKind::Function(_)).then_some((
                function.name(),
                false,
                false,
            ));
        }
        if let Some(function) = LocalFuncDecl::cast(node) {
            return Some((function.name(), true, false));
        }
        if let Some(method) = MethodDecl::cast(node) {
            if method.syntax().parent().is_some_and(|parent| {
                matches!(
                    parent.kind(),
                    NodeKind::ExtensionDecl | NodeKind::ExtensionTypeDecl
                )
            }) {
                return None;
            }
            return Some((method.name(), false, true));
        }
        if matches!(
            node.kind(),
            NodeKind::FuncExpr
                | NodeKind::GetterDecl
                | NodeKind::SetterDecl
                | NodeKind::ConstructorDecl
        ) {
            return None;
        }
        ancestor = node.parent();
    }
    None
}
