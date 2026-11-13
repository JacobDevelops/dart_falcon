//! Flags `/** ... */` block documentation comments in favor of the `///` form.
//!
//! Block documentation attached to libraries, type declarations, members, and
//! functions should use the consistent `///` form. Documentation on parameters
//! and local variables is not covered, matching the analyzer lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct SlashForDocComments;

impl Rule for SlashForDocComments {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diags = Vec::new();
        let first_token = ctx
            .tree()
            .top_level_tokens(ctx.source())
            .find(|token| !token.is_trivia())
            .map(|token| token.span().start);
        for block in ctx.tree().doc_blocks(ctx.source()) {
            let Some(token) = ctx.tree().token_at(ctx.source(), block.next_token) else {
                continue;
            };
            let attached = token.parent().ancestors().any(|node| {
                node.span().start == block.next_token
                    && matches!(
                        node.kind(),
                        NodeKind::LibraryDirective
                            | NodeKind::ClassDecl
                            | NodeKind::ClassTypeAliasDecl
                            | NodeKind::MixinDecl
                            | NodeKind::MixinClassDecl
                            | NodeKind::EnumDecl
                            | NodeKind::EnumVariant
                            | NodeKind::ExtensionDecl
                            | NodeKind::ExtensionTypeDecl
                            | NodeKind::FieldDecl
                            | NodeKind::ConstructorDecl
                            | NodeKind::MethodDecl
                            | NodeKind::GetterDecl
                            | NodeKind::SetterDecl
                            | NodeKind::OperatorDecl
                            | NodeKind::FunctionDecl
                            | NodeKind::LocalFuncDecl
                            | NodeKind::TopLevelVarDecl
                            | NodeKind::TypeAliasDecl
                    )
            });
            if first_token != Some(block.next_token) && !attached {
                continue;
            }
            if let Some(comment) = block
                .tokens()
                .find(|comment| comment.text().starts_with("/**"))
            {
                diags.push(ctx.diagnostic(
                    "Use the end-of-line form (`///`) for doc comments.",
                    comment.span(),
                ));
            }
        }
        RulePreparation::Diagnostics(diags)
    }
}
