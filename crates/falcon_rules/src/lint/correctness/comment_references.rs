//! Flags doc-comment references `[name]` that resolve to nothing in scope.
//!
//! dartdoc links a bracketed name to the declaration it denotes, so a
//! misspelled or stale `[name]` renders as plain text and the link silently
//! breaks. References are parsed and resolved as the analyzer's doc-comment
//! parser and `CommentReferenceResolver` do (see `crate::lint::doc_comments`):
//! `[name]`, `[Type.member]`, `[prefix.name]` and `[prefix.Type.member]`
//! (including operators) are reported when the last name resolves to
//! nothing; `[this]`, `[null]`, `[true]` and `[false]` are always reported.
//! References defined as Markdown link references (`/// [name]: https://...`),
//! Markdown link text and code spans and blocks are not references. Names
//! falcon cannot prove absent (an unindexed import or supertype) are not
//! reported.
//!
//! Adopted from the official `comment_references` lint.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::cst::*;

use crate::lint::doc_comments::{
    DocLookup, DocResolver, DocScopes, keyword_references, references,
};
use falcon_analyze::semantic_scope::SemanticState;

const ID: &str = "comment-references";

#[derive(Default)]
pub struct CommentReferences;

impl Rule for CommentReferences {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let sdk_loaded = ctx
            .identities()
            .is_some_and(|identities| identities.has_sdk_sources());
        let scopes = DocScopes::new(program, &state.model, ctx.source());
        let resolver = DocResolver::new(
            program,
            &state.model,
            &state.signatures,
            &scopes,
            sdk_loaded,
        );
        let mut diagnostics = Vec::new();
        let mut report = |offset: usize, text: &str| {
            diagnostics.push(ctx.diagnostic(
                ID,
                format!("The referenced name '{text}' isn't visible in scope."),
                DiagSpan {
                    start: offset,
                    end: offset + text.len(),
                },
            ));
        };
        for block in ctx.tree().doc_blocks(ctx.source()) {
            let Some(scope) = scopes.scope_for(&block) else {
                continue;
            };
            for (offset, keyword) in keyword_references(&block) {
                report(offset, keyword);
            }
            let (found, definitions) = references(&block);
            for reference in found {
                let Some(parsed) = reference.parse() else {
                    continue;
                };
                if !definitions.contains(reference.text.trim())
                    && matches!(
                        resolver.resolve(&parsed, scope).last(),
                        Some(DocLookup::Absent)
                    )
                {
                    report(reference.offset, reference.text);
                }
            }
        }
        RulePreparation::Diagnostics(diagnostics)
    }
}
