use super::LspState;

use falcon_diagnostics::{FixKind, LineIndex};
use falcon_project::{ProjectAnalysis, effective_fix_kind, fix_file};
use lsp_types::{
    CodeAction, CodeActionKind, CodeActionParams, DocumentChanges, OneOf,
    OptionalVersionedTextDocumentIdentifier, Range, TextDocumentEdit, TextEdit, Uri, WorkspaceEdit,
};

pub(crate) const SAFE_FIX_ALL: &str = "source.fixAll.falcon";
// Not a descendant of source.fixAll: generic save requests must remain safe.
pub(crate) const UNSAFE_FIX_ALL: &str = "source.falcon.fixAllUnsafe";

fn requested(params: &CodeActionParams, kind: &str) -> bool {
    params.context.only.as_ref().is_none_or(|only| {
        only.iter().any(|parent| {
            kind == parent.as_str()
                || kind
                    .strip_prefix(parent.as_str())
                    .is_some_and(|suffix| suffix.starts_with('.'))
        })
    })
}

fn workspace_edit(uri: &Uri, version: Option<i32>, edits: Vec<TextEdit>) -> WorkspaceEdit {
    WorkspaceEdit {
        document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
            text_document: OptionalVersionedTextDocumentIdentifier {
                uri: uri.clone(),
                version,
            },
            edits: edits.into_iter().map(OneOf::Left).collect(),
        }])),
        ..Default::default()
    }
}

fn minimal_edit(source: &str, mut replacement: String, lines: &LineIndex) -> TextEdit {
    let mut prefix = source
        .chars()
        .zip(replacement.chars())
        .take_while(|(left, right)| left == right)
        .map(|(character, _)| character.len_utf8())
        .sum::<usize>();
    let splits_crlf = |text: &str, offset: usize| {
        offset > 0 && text.as_bytes().get(offset - 1..offset + 1) == Some(b"\r\n")
    };
    if splits_crlf(source, prefix) || splits_crlf(&replacement, prefix) {
        prefix -= 1;
    }
    let mut suffix = source[prefix..]
        .chars()
        .rev()
        .zip(replacement[prefix..].chars().rev())
        .take_while(|(left, right)| left == right)
        .map(|(character, _)| character.len_utf8())
        .sum::<usize>();
    if splits_crlf(source, source.len() - suffix)
        || splits_crlf(&replacement, replacement.len() - suffix)
    {
        suffix -= 1;
    }
    let end = source.len() - suffix;
    replacement.truncate(replacement.len() - suffix);
    replacement.replace_range(..prefix, "");
    TextEdit {
        range: Range::new(lines.lsp_position(prefix), lines.lsp_position(end)),
        new_text: replacement,
    }
}

impl LspState {
    pub(crate) fn code_actions(
        &mut self,
        params: &CodeActionParams,
    ) -> Result<Vec<CodeAction>, String> {
        let quickfix = requested(params, "quickfix");
        // Unfiltered lightbulb requests must not run iterative whole-file fixes.
        let safe = params.context.only.is_some() && requested(params, SAFE_FIX_ALL);
        let unsafe_fixes = params.context.only.is_some() && requested(params, UNSAFE_FIX_ALL);
        if !quickfix && !safe && !unsafe_fixes {
            return Ok(Vec::new());
        }
        let uri = &params.text_document.uri;
        let Some(document) = self.documents.get(uri.as_str()) else {
            return Ok(Vec::new());
        };
        if document.source_rejected || document.version.is_none() {
            return Ok(Vec::new());
        }
        // didChange may still be debounced, so last_diagnostics can be stale.
        // Keep the last published (possibly cross-file merged) hover diagnostics.
        let published = std::mem::take(
            &mut self
                .documents
                .get_mut(uri.as_str())
                .expect("open document")
                .last_diagnostics,
        );
        let diagnostics = self.analyze(uri.as_str());
        self.documents
            .get_mut(uri.as_str())
            .expect("open document")
            .last_diagnostics = published;
        let document = &self.documents[uri.as_str()];
        let source = document.text();
        let lines = LineIndex::new(source);
        let capacity = if quickfix { diagnostics.len() } else { 0 }
            + usize::from(safe)
            + usize::from(unsafe_fixes);
        let mut actions = Vec::with_capacity(capacity);
        if quickfix {
            for diagnostic in &diagnostics {
                let Some(fix) = &diagnostic.fix else { continue };
                let Some(kind) =
                    effective_fix_kind(&self.config, &document.path, diagnostic.rule, fix.kind)
                else {
                    continue;
                };
                let lsp_diagnostic = diagnostic.format_lsp_in(&lines);
                let range = lsp_diagnostic.range;
                let intersects = if range.start == range.end {
                    if params.range.start == params.range.end {
                        range.start == params.range.start
                    } else {
                        params.range.start <= range.start && range.start < params.range.end
                    }
                } else if params.range.start == params.range.end {
                    range.start <= params.range.start && params.range.start < range.end
                } else {
                    range.start < params.range.end && params.range.start < range.end
                };
                if !intersects
                    || (!params.context.diagnostics.is_empty()
                        && !params.context.diagnostics.iter().any(|context| {
                            context.code == lsp_diagnostic.code
                                && context.range == range
                                && context.message == lsp_diagnostic.message
                        }))
                {
                    continue;
                }
                let title = if kind == FixKind::Unsafe {
                    format!("{} (unsafe)", fix.title)
                } else {
                    fix.title.clone()
                };
                if let Err(error) = fix.validate(source) {
                    actions.push(CodeAction {
                        title,
                        kind: Some(CodeActionKind::QUICKFIX),
                        diagnostics: Some(vec![lsp_diagnostic]),
                        is_preferred: Some(false),
                        disabled: Some(lsp_types::CodeActionDisabled {
                            reason: format!("Invalid fix from {}: {error}", diagnostic.rule),
                        }),
                        ..Default::default()
                    });
                    continue;
                }
                let edits = fix
                    .edits()
                    .iter()
                    .map(|edit| TextEdit {
                        range: Range::new(
                            lines.lsp_position(edit.span.start),
                            lines.lsp_position(edit.span.end),
                        ),
                        new_text: edit.replacement.clone(),
                    })
                    .collect();
                actions.push(CodeAction {
                    title,
                    kind: Some(CodeActionKind::QUICKFIX),
                    diagnostics: Some(vec![lsp_diagnostic]),
                    is_preferred: Some(kind == FixKind::Safe),
                    edit: Some(workspace_edit(uri, document.version, edits)),
                    ..Default::default()
                });
            }
        }
        // Admission checks and fix suppression happen in the normal diagnostic
        // pipeline; do not run the standalone engine on an excluded document.
        if !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.fix.is_some())
        {
            return Ok(actions);
        }
        if safe || unsafe_fixes {
            let analysis = ProjectAnalysis::new(&self.config);
            for (kind, include_unsafe, title) in [
                (SAFE_FIX_ALL, false, "Fix all safe Falcon issues"),
                (UNSAFE_FIX_ALL, true, "Fix all Falcon issues (unsafe)"),
            ] {
                if (include_unsafe && !unsafe_fixes) || (!include_unsafe && !safe) {
                    continue;
                }
                let outcome = fix_file(
                    &analysis,
                    &self.config,
                    &mut self.environment,
                    &document.path,
                    source,
                    include_unsafe,
                );
                if let Some(stop) = outcome.stop {
                    let reason = format!("Falcon fix all stopped without returning edits: {stop}");
                    if !quickfix {
                        return Err(reason);
                    }
                    actions.push(CodeAction {
                        title: title.to_owned(),
                        kind: Some(CodeActionKind::from(kind)),
                        disabled: Some(lsp_types::CodeActionDisabled { reason }),
                        ..Default::default()
                    });
                    continue;
                }
                if outcome.source == source {
                    continue;
                }
                actions.push(CodeAction {
                    title: title.to_string(),
                    kind: Some(CodeActionKind::from(kind)),
                    edit: Some(workspace_edit(
                        uri,
                        document.version,
                        vec![minimal_edit(source, outcome.source, &lines)],
                    )),
                    ..Default::default()
                });
            }
        }
        Ok(actions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::Position;

    #[test]
    fn requesting_actions_preserves_published_diagnostic_cache() {
        let directory = tempfile::tempdir().unwrap();
        let config = directory.path().join("falcon.json");
        std::fs::write(
            &config,
            r#"{"linter":{"rules":{"recommended":false,"style":{"unnecessary-new":"warn"}}}}"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        let uri = format!("file://{}", directory.path().join("main.dart").display());
        let source = "void main() { final x = new Object(); }";
        let mut published = state.open(&uri, source.to_owned(), Some(1));
        assert_eq!(published.len(), 1);
        published[0].rule = "avoid-unused-files";
        published[0].message = "Published cross-file diagnostic".to_owned();
        state.documents.get_mut(&uri).unwrap().last_diagnostics = published;
        let params = serde_json::from_value(serde_json::json!({
            "textDocument": { "uri": uri },
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 100 } },
            "context": { "diagnostics": [], "only": ["quickfix"] }
        })).unwrap();
        let actions = state.code_actions(&params).unwrap();
        assert_eq!(actions.len(), 1);
        let cached = &state.documents[&uri].last_diagnostics;
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].rule, "avoid-unused-files");
        assert_eq!(cached[0].message, "Published cross-file diagnostic");
    }
    #[test]
    fn minimal_edits_preserve_common_text_and_utf8_boundaries() {
        for (source, replacement, start, end, text) in [
            (
                "a😀z",
                "a😁z",
                Position::new(0, 1),
                Position::new(0, 3),
                "😁",
            ),
            (
                "a😀z",
                "a😀!z",
                Position::new(0, 3),
                Position::new(0, 3),
                "!",
            ),
            ("a😀z", "az", Position::new(0, 1), Position::new(0, 3), ""),
            (
                "one\nsame\nlast",
                "one\nnew\nlast",
                Position::new(1, 0),
                Position::new(1, 4),
                "new",
            ),
            (
                "a\r\nb",
                "a\nb",
                Position::new(0, 1),
                Position::new(1, 0),
                "\n",
            ),
            (
                "a\nb",
                "a\r\nb",
                Position::new(0, 1),
                Position::new(1, 0),
                "\r\n",
            ),
        ] {
            let edit = minimal_edit(source, replacement.to_owned(), &LineIndex::new(source));
            assert_eq!(edit.range, Range::new(start, end));
            assert_eq!(edit.new_text, text);
            let mut applied = source.to_owned();
            let start = falcon_diagnostics::lsp_position_to_byte(source, edit.range.start);
            let end = falcon_diagnostics::lsp_position_to_byte(source, edit.range.end);
            applied.replace_range(start..end, &edit.new_text);
            assert_eq!(applied, replacement);
        }
    }
}
