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
use falcon_syntax::ast::*;
use falcon_syntax::{Program, TokenKind};

#[derive(Default)]
pub struct AvoidReturnTypesOnSetters;

impl Rule for AvoidReturnTypesOnSetters {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_, '_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

    fn visit_expr(&mut self, _node: &Expr) -> StructuralTraversal {
        StructuralTraversal::SkipChildren
    }

    fn visit_top_level_decl(&mut self, decl: &TopLevelDecl) -> StructuralTraversal {
        match decl {
            TopLevelDecl::Function(_)
            | TopLevelDecl::Class(_)
            | TopLevelDecl::Mixin(_)
            | TopLevelDecl::MixinClass(_)
            | TopLevelDecl::Enum(_)
            | TopLevelDecl::Extension(_)
            | TopLevelDecl::ExtensionType(_) => StructuralTraversal::Continue,
            _ => StructuralTraversal::SkipChildren,
        }
    }

    fn visit_class_member(&mut self, member: &ClassMember) -> StructuralTraversal {
        if matches!(member, ClassMember::Setter(_)) {
            StructuralTraversal::Continue
        } else {
            StructuralTraversal::SkipChildren
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) -> StructuralTraversal {
        if matches!(node.kind, FunctionKind::Setter(_)) && node.return_type.is_some() {
            self.diags.push(self.ctx.diagnostic(
                "avoid-return-types-on-setters",
                "Avoid return types on setters.",
                node.name.span,
            ));
        }
        StructuralTraversal::SkipChildren
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) -> StructuralTraversal {
        if setter_has_return_type(node, self.ctx.syntax) {
            self.diags.push(self.ctx.diagnostic(
                "avoid-return-types-on-setters",
                "Avoid return types on setters.",
                node.name.span,
            ));
        }
        StructuralTraversal::SkipChildren
    }
}
}

fn setter_has_return_type(setter: &SetterDecl, syntax: &falcon_syntax::SyntaxStore) -> bool {
    let ann_end = setter
        .annotations
        .iter()
        .map(|a| a.span.end)
        .max()
        .unwrap_or(setter.span.start)
        .max(setter.span.start);
    syntax
        .tokens_in(Span::new(ann_end, setter.name.span.start))
        .any(|token| {
            !token.is_trivia()
                && !matches!(
                    token.kind,
                    TokenKind::Static | TokenKind::External | TokenKind::Set
                )
        })
}
