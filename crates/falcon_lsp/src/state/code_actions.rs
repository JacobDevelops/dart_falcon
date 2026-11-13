use super::{DocumentState, LspState};

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
pub(crate) const FIX_ALL_COMMAND: &str = "falcon.fixAll";

#[derive(Debug)]
pub(crate) enum ActionError {
    InvalidData(String),
    ContentModified,
    AnalysisUnavailable(String),
}

impl ActionError {
    pub(crate) fn response(self, id: lsp_server::RequestId) -> lsp_server::Response {
        let (code, message) = match self {
            Self::InvalidData(message) => (lsp_server::ErrorCode::InvalidParams, message),
            Self::ContentModified => (
                lsp_server::ErrorCode::ContentModified,
                "Document changed since Falcon offered the action".to_owned(),
            ),
            Self::AnalysisUnavailable(message) => (lsp_server::ErrorCode::RequestFailed, message),
        };
        lsp_server::Response::new_err(id, code as i32, message)
    }
}

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

struct QuickFixContext<'a> {
    document: &'a DocumentState,
    config: &'a falcon_config::FalconConfig,
    lines: &'a LineIndex<'a>,
    document_changes: bool,
}

impl QuickFixContext<'_> {
    fn append_actions(
        &self,
        params: &CodeActionParams,
        cancelled: &dyn Fn() -> bool,
        actions: &mut Vec<CodeAction>,
    ) -> Result<(), ActionError> {
        let document = self.document;
        let config = self.config;
        let lines = self.lines;
        let source = document.text();
        let uri = &params.text_document.uri;
        for diagnostic in document.current_diagnostics().unwrap_or_default() {
            if cancelled() {
                return Err(ActionError::AnalysisUnavailable(
                    "Falcon code actions cancelled".to_owned(),
                ));
            }
            let Some(fix) = &diagnostic.fix else { continue };
            let Some(kind) = effective_fix_kind(config, &document.path, diagnostic.rule, fix.kind)
            else {
                continue;
            };
            let lsp_diagnostic = diagnostic.format_lsp_in(lines);
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
                    self.document_changes,
                )),
                ..Default::default()
            });
        }
        Ok(())
    }
}

impl LspState {
    pub(crate) fn configure_code_actions(&mut self, params: &serde_json::Value) {
        self.capabilities.code_action_resolve = params
            .pointer("/capabilities/textDocument/codeAction/dataSupport")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
            && params
                .pointer("/capabilities/textDocument/codeAction/resolveSupport/properties")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|properties| properties.iter().any(|property| property == "edit"));
        self.capabilities.workspace_document_changes = params
            .pointer("/capabilities/workspace/workspaceEdit/documentChanges")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
        self.capabilities.workspace_apply_edit = params
            .pointer("/capabilities/workspace/applyEdit")
            .and_then(serde_json::Value::as_bool)
            == Some(true);
    }

    pub(crate) fn code_actions(
        &mut self,
        params: &CodeActionParams,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<Vec<CodeAction>, ActionError> {
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
        if cancelled() || document.parsed.is_rejected() || document.version.is_none() {
            return Ok(Vec::new());
        }
        let Some(diagnostics) = document.current_diagnostics() else {
            return Ok(Vec::new());
        };
        let root_index = self.root_index(&document.path);
        let config = if root_index == 0 {
            &self.config
        } else {
            &self.additional_root_configs[root_index - 1].config
        };
        let source = document.text();
        let lines = LineIndex::new(source);
        let capacity = if quickfix { diagnostics.len() } else { 0 }
            + usize::from(safe)
            + usize::from(unsafe_fixes);
        let mut actions = Vec::with_capacity(capacity);
        if quickfix && !document.has_parse_errors() {
            QuickFixContext {
                document,
                config,
                lines: &lines,
                document_changes: self.capabilities.workspace_document_changes,
            }
            .append_actions(params, cancelled, &mut actions)?;
        }
        // Admission checks and fix suppression happen in the normal diagnostic
        // pipeline; do not run the standalone engine on an excluded document.
        if !document.has_parse_errors()
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
            if !document.has_parse_errors()
                && !diagnostics.iter().any(|diagnostic| {
                    diagnostic.fix.as_ref().is_some_and(|fix| {
                        effective_fix_kind(config, &document.path, diagnostic.rule, fix.kind)
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
            if document.has_parse_errors() {
                action.disabled = Some(lsp_types::CodeActionDisabled {
                    reason: "Falcon cannot fix a document with syntax errors".to_owned(),
                });
            } else if !self.capabilities.code_action_resolve {
                if !self.capabilities.workspace_apply_edit {
                    continue;
                }
                action.command = Some(lsp_types::Command {
                    title: title.to_owned(),
                    command: FIX_ALL_COMMAND.to_owned(),
                    arguments: Some(vec![action.data.take().expect("fix-all data")]),
                });
            }
            actions.push(action);
        }
        Ok(actions)
    }

    pub(crate) fn resolve_code_action(
        &mut self,
        mut action: CodeAction,
        cancelled: &dyn Fn() -> bool,
    ) -> Result<CodeAction, ActionError> {
        let data = action
            .data
            .as_ref()
            .ok_or_else(|| ActionError::InvalidData("Missing Falcon fix-all data".to_owned()))?;
        let uri: Uri = serde_json::from_value(data["uri"].clone())
            .map_err(|error| ActionError::InvalidData(error.to_string()))?;
        let version: i32 = serde_json::from_value(data["version"].clone())
            .map_err(|error| ActionError::InvalidData(error.to_string()))?;
        let version = Some(version);
        let include_unsafe = data["includeUnsafe"]
            .as_bool()
            .ok_or_else(|| ActionError::InvalidData("Missing fix-all policy".to_owned()))?;
        let document = self
            .documents
            .get(uri.as_str())
            .ok_or(ActionError::ContentModified)?;
        if document.version != version || document.current_diagnostics().is_none() {
            return Err(ActionError::ContentModified);
        }
        if cancelled() {
            return Err(ActionError::AnalysisUnavailable(
                "Falcon fix all cancelled".to_owned(),
            ));
        }
        if document.has_parse_errors() {
            return Err(ActionError::AnalysisUnavailable(
                "Falcon cannot fix a document with syntax errors".to_owned(),
            ));
        }
        if self.resolve_semantics() && self.semantic_snapshot.is_none() {
            self.ensure_semantic_snapshot();
        }
        self.refresh_required_source_roots(&[]);
        let document = self.documents.get(uri.as_str()).expect("open document");
        if document.version != version || document.current_diagnostics().is_none() {
            return Err(ActionError::ContentModified);
        }
        if let Some(error) = &self.semantic_snapshot_error {
            return Err(ActionError::AnalysisUnavailable(format!(
                "Falcon cannot fix without a valid semantic snapshot: {error}"
            )));
        }
        if self.semantic_source_incomplete {
            return Err(ActionError::AnalysisUnavailable(
                "Falcon cannot fix while semantic sources are incomplete".to_owned(),
            ));
        }
        let root_index = self.root_index(&document.path);
        let config = if root_index == 0 {
            &self.config
        } else {
            &self.additional_root_configs[root_index - 1].config
        };
        let analysis = ProjectAnalysis::new(config);
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
            config,
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
            return Err(ActionError::AnalysisUnavailable(format!(
                "Falcon fix all stopped without returning edits: {stop}"
            )));
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
                self.capabilities.workspace_document_changes,
            ));
        }
        Ok(action)
    }
}

#[cfg(test)]
mod tests;
