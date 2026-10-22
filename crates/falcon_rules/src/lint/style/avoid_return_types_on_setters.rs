//! Flags setters declared with an explicit return type.
//!
//! A setter never returns a value, so annotating one with a return type — even
//! `void` — is redundant and can mislead readers into thinking the result is
//! usable. Drop the return type and write `set foo(int v)`. Both top-level
//! setters and setters declared in classes, mixins, enums, and extensions are
//! checked.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::TokenKind;
use falcon_syntax::cst::{self, FunctionKind, Span};

#[derive(Default)]
pub struct AvoidReturnTypesOnSetters;

impl Rule for AvoidReturnTypesOnSetters {
    fn prepare<'a>(
        &self,
        _program: cst::Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> { crate::lint::support::preparation::prepare!(Structural, Collector, ctx, diags) }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, _node: cst::Expr<'_>) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_top_level_decl(&mut self, decl: cst::TopLevelDecl<'_>) -> StructuralTraversal {
        match decl {
            cst::TopLevelDecl::Function(_)
            | cst::TopLevelDecl::Class(_)
            | cst::TopLevelDecl::Mixin(_)
            | cst::TopLevelDecl::MixinClass(_)
            | cst::TopLevelDecl::Enum(_)
            | cst::TopLevelDecl::Extension(_)
            | cst::TopLevelDecl::ExtensionType(_) => StructuralTraversal::Continue,
            _ => StructuralTraversal::SkipChildren,
        }
    }

    fn visit_class_member(&mut self, member: cst::ClassMember<'_>) -> StructuralTraversal {
        if matches!(member, cst::ClassMember::Setter(_)) {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_function_decl(&mut self, node: cst::FunctionDecl<'_>) -> StructuralTraversal {
        if matches!(node.kind(), FunctionKind::Setter(_)) && node.return_type().is_some() {
            self.diags.push(self.ctx.diagnostic(
                "Avoid return types on setters.",
                node.name().span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_setter_decl(&mut self, node: cst::SetterDecl<'_>) -> StructuralTraversal {
        if setter_has_return_type(node, self.ctx.tree(), self.ctx.source()) {
            self.diags.push(self.ctx.diagnostic(
                "Avoid return types on setters.",
                node.name().span(),
            ));
        }
        StructuralTraversal::SkipChildren
    }
}
}

fn setter_has_return_type(
    setter: cst::SetterDecl<'_>,
    tree: &cst::SyntaxTree,
    source: &str,
) -> bool {
    let ann_end = setter
        .annotations()
        .map(|a| a.span().end)
        .max()
        .unwrap_or(setter.span().start)
        .max(setter.span().start);
    tree.tokens_in(source, Span::new(ann_end, setter.name().span().start))
        .any(|token| {
            !token.is_trivia()
                && !matches!(
                    token.kind(),
                    TokenKind::Static | TokenKind::External | TokenKind::Set
                )
        })
}
