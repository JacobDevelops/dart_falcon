use super::LspState;

use falcon_analyze::ExternalIndexParts;
use falcon_diagnostics::{FixKind, LineIndex};
use falcon_project::{FixContext, ProjectAnalysis, effective_fix_kind, fix_file_with_context};
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

fn workspace_edit(
    uri: &Uri,
    version: Option<i32>,
    edits: Vec<TextEdit>,
    document_changes: bool,
) -> WorkspaceEdit {
    if !document_changes {
        return WorkspaceEdit {
            changes: Some([(uri.clone(), edits)].into_iter().collect()),
            ..Default::default()
        };
    }
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
    pub(crate) fn configure_code_actions(&mut self, params: &serde_json::Value) {
        self.code_action_resolve = params
            .pointer("/capabilities/textDocument/codeAction/dataSupport")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && params
                .pointer("/capabilities/textDocument/codeAction/resolveSupport/properties")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|properties| properties.iter().any(|property| property == "edit"));
        self.workspace_document_changes = params
            .pointer("/capabilities/workspace/workspaceEdit/documentChanges")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
    }

    pub(crate) fn code_actions(
        &mut self,
        params: &CodeActionParams,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<CodeAction>, String> {
        let quickfix = requested(params, "quickfix");
        let safe = requested(params, SAFE_FIX_ALL);
        let unsafe_fixes = requested(params, UNSAFE_FIX_ALL);
        if !quickfix && !safe && !unsafe_fixes {
            return Ok(Vec::new());
        }
        let uri = &params.text_document.uri;
        let Some(document) = self.documents.get(uri.as_str()) else {
            return Ok(Vec::new());
        };
        if cancelled()
            || document.source_rejected
            || document.version.is_none()
            || document.diagnostics_parse_count != document.parse_count
        {
            return Ok(Vec::new());
        }
        let document = &self.documents[uri.as_str()];
        let source = document.text();
        let diagnostics = &document.last_diagnostics;
        let lines = LineIndex::new(source);
        let capacity = if quickfix { diagnostics.len() } else { 0 }
            + usize::from(safe)
            + usize::from(unsafe_fixes);
        let mut actions = Vec::with_capacity(capacity);
        if quickfix && !document.has_parse_errors {
            for diagnostic in diagnostics {
                if cancelled() {
                    return Err("Falcon code actions cancelled".to_owned());
                }
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
                    range.start <= params.range.start && params.range.start <= range.end
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
                    edit: Some(workspace_edit(
                        uri,
                        document.version,
                        edits,
                        self.workspace_document_changes,
                    )),
                    ..Default::default()
                });
            }
        }
        // Admission checks and fix suppression happen in the normal diagnostic
        // pipeline; do not run the standalone engine on an excluded document.
        if !document.has_parse_errors
            && !diagnostics
                .iter()
                .any(|diagnostic| diagnostic.fix.is_some())
        {
            return Ok(actions);
        }
        for (kind, include_unsafe, title) in [
            (SAFE_FIX_ALL, false, "Fix all safe Falcon issues"),
            (UNSAFE_FIX_ALL, true, "Fix all Falcon issues (unsafe)"),
        ] {
            if (include_unsafe && !unsafe_fixes) || (!include_unsafe && !safe) {
                continue;
            }
            if !document.has_parse_errors
                && !diagnostics.iter().any(|diagnostic| {
                    diagnostic.fix.as_ref().is_some_and(|fix| {
                        effective_fix_kind(&self.config, &document.path, diagnostic.rule, fix.kind)
                            .is_some_and(|kind| include_unsafe || kind == FixKind::Safe)
                    })
                })
            {
                continue;
            }
            let mut action = CodeAction {
                title: title.to_owned(),
                kind: Some(CodeActionKind::from(kind)),
                data: Some(serde_json::json!({
                    "uri": uri,
                    "version": document.version,
                    "includeUnsafe": include_unsafe,
                })),
                ..Default::default()
            };
            if document.has_parse_errors {
                action.disabled = Some(lsp_types::CodeActionDisabled {
                    reason: "Falcon cannot fix a document with syntax errors".to_owned(),
                });
            }
            actions.push(action);
        }
        if !self.code_action_resolve {
            for action in &mut actions {
                if action.data.is_some() && action.disabled.is_none() {
                    let unresolved = std::mem::take(action);
                    match self.resolve_code_action(unresolved.clone(), cancelled) {
                        Ok(resolved) => *action = resolved,
                        Err(reason) if quickfix => {
                            *action = unresolved;
                            action.disabled = Some(lsp_types::CodeActionDisabled { reason });
                        }
                        Err(reason) => return Err(reason),
                    }
                }
            }
            actions.retain(|action| action.edit.is_some() || action.disabled.is_some());
        }
        Ok(actions)
    }

    pub(crate) fn resolve_code_action(
        &mut self,
        mut action: CodeAction,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CodeAction, String> {
        let data = action.data.as_ref().ok_or("Missing Falcon fix-all data")?;
        let uri: Uri =
            serde_json::from_value(data["uri"].clone()).map_err(|error| error.to_string())?;
        let version: Option<i32> =
            serde_json::from_value(data["version"].clone()).map_err(|error| error.to_string())?;
        let include_unsafe = data["includeUnsafe"]
            .as_bool()
            .ok_or("Missing fix-all policy")?;
        let document = self
            .documents
            .get(uri.as_str())
            .ok_or("Document is no longer open")?;
        if document.version != version || document.diagnostics_parse_count != document.parse_count {
            return Err("Document changed since Falcon offered the action".to_owned());
        }
        if cancelled() {
            return Err("Falcon fix all cancelled".to_owned());
        }
        if document.has_parse_errors {
            return Err("Falcon cannot fix a document with syntax errors".to_owned());
        }
        if self.resolve_semantics && self.semantic_snapshot.is_none() {
            self.ensure_semantic_snapshot();
        }
        let document = self.documents.get(uri.as_str()).expect("open document");
        if document.version != version || document.diagnostics_parse_count != document.parse_count {
            return Err("Document changed since Falcon offered the action".to_owned());
        }
        if let Some(error) = &self.semantic_snapshot_error {
            return Err(format!(
                "Falcon cannot fix without a valid semantic snapshot: {error}"
            ));
        }
        let analysis = ProjectAnalysis::new(&self.config);
        let external = self
            .external_indexes
            .as_ref()
            .map(|cache| ExternalIndexParts {
                identities: &cache.identities,
                types: &cache.types,
                signatures: &cache.signatures,
            });
        let outcome = fix_file_with_context(
            &analysis,
            &self.config,
            &mut self.environment,
            &document.path,
            document.text(),
            include_unsafe,
            FixContext {
                external,
                project_files: self
                    .semantic_snapshot
                    .as_ref()
                    .map_or(&[], |snapshot| snapshot.files.as_slice()),
                external_libraries: Some(&self.external_libraries),
                cancelled,
            },
        );
        if let Some(stop) = outcome.stop {
            return Err(format!(
                "Falcon fix all stopped without returning edits: {stop}"
            ));
        }
        if outcome.source != document.text() {
            action.edit = Some(workspace_edit(
                &uri,
                document.version,
                vec![minimal_edit(
                    document.text(),
                    outcome.source,
                    &LineIndex::new(document.text()),
                )],
                self.workspace_document_changes,
            ));
        }
        Ok(action)
    }
}

#[cfg(test)]
mod tests;
