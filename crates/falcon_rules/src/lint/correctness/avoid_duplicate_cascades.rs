//! Flags repeated field or index assignment targets within one cascade.
//! Calls are not assumed redundant. Different right-hand sides still overwrite
//! the same target; targets involving calls or mutation are conservatively skipped.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst;

#[derive(Default)]
pub struct AvoidDuplicateCascades;

impl Rule for AvoidDuplicateCascades {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: cst::Expr<'a>) -> StructuralTraversal {
        if let cst::Expr::Cascade(cascade) = node {
            let mut barrier = 0;
            for (index, section) in cascade.sections().enumerate() {
                if section.syntax().descendants().filter_map(cst::Expr::cast).any(|expr| matches!(expr, cst::Expr::Call(_) | cst::Expr::New(_) | cst::Expr::Await(_)))
                    || section.ops().any(|op| matches!(op, cst::CascadeOp::Call(_) | cst::CascadeOp::Invoke(_)))
                {
                    barrier = index + 1;
                    continue;
                }
                if let Some(target) = assignment_target(section)
                    && cascade.sections().take(index).skip(barrier).filter_map(assignment_target).any(|earlier| {
                        let tokens = |span| self.ctx.tree().tokens_in(self.ctx.source, span)
                            .filter(|token| !token.is_trivia())
                            .skip_while(|token| matches!(token.kind(), falcon_syntax::TokenKind::DotDot | falcon_syntax::TokenKind::DotDotQmark))
                            .map(|token| (token.kind(), token.text()));
                        tokens(earlier).eq(tokens(target))
                    })
                {
                    self.diags.push(self.ctx.diagnostic(
                        "avoid-duplicate-cascades",
                        "This cascade assigns to the same target more than once — remove the overwritten assignment or correct its target",
                        section.span(),
                    ));
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn assignment_target(section: cst::CascadeSection<'_>) -> Option<cst::Span> {
    let target = match section.ops().last()? {
        cst::CascadeOp::Assign(node) if node.op() == cst::AssignOp::Eq => node.target(),
        cst::CascadeOp::IndexAssign(node) if node.op() == cst::AssignOp::Eq => node.target(),
        _ => return None,
    };
    if section.ops().any(|op| matches!(op, cst::CascadeOp::Call(_) | cst::CascadeOp::Invoke(_)))
        || target.syntax().descendants().filter_map(cst::Expr::cast).any(|expr| {
            matches!(expr, cst::Expr::Call(_) | cst::Expr::New(_) | cst::Expr::Assign(_) | cst::Expr::PostfixIncDec(_) | cst::Expr::Await(_))
                || matches!(expr, cst::Expr::Unary(unary) if matches!(unary.op(), cst::UnaryOp::PlusPlus | cst::UnaryOp::MinusMinus))
        })
    {
        return None;
    }
    Some(cst::Span::new(section.span().start, target.span().end))
}
