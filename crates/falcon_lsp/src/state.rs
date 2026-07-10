//! LSP server state: document cache, cached ASTs, config-driven rule set.
//!
//! Caching model (see `.omc/docs/LSP_CACHING_DESIGN.md`): a document's AST is
//! invalidated only by *text* changes; the rule set is invalidated only by
//! *config* changes. A config reload therefore re-runs the new rules over the
//! cached ASTs without re-parsing — by construction there is no
//! stale-AST-with-new-config state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use falcon_analyze::{AnalyzeContext, FileSuppressions, Rule};
use falcon_config::{FalconConfig, load_config, load_or_default};
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;
use falcon_rules::{apply_severities, meta::suppression_lookup, resolve_rules};
use falcon_syntax::Program;
use tracing::{debug, warn};

/// One open document: full text, cached AST, and instrumentation counters.
pub struct DocumentState {
    pub text: String,
    pub version: Option<i32>,
    program: Program,
    /// Most recent analysis output (byte spans) — read by hover. This is a
    /// copy of the last published result, never an input to analysis.
    pub last_diagnostics: Vec<Diagnostic>,
    /// Number of times this document has been parsed (incremental tests).
    pub parse_count: u64,
    /// Number of times this document has been analyzed (incremental tests).
    pub analyze_count: u64,
}

/// Server-side cache: open documents, active config, enabled rule set.
pub struct LspState {
    documents: HashMap<String, DocumentState>,
    config: FalconConfig,
    config_path: Option<PathBuf>,
    rules: Vec<Box<dyn Rule>>,
}

impl LspState {
    /// Create state with config from `config_path`, or discovery from the
    /// current directory when `None` (same order as the CLI: cwd → git root →
    /// `$HOME/.falcon.json` → defaults).
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let config = load_from(config_path.as_deref());
        let resolved = resolve_rules(&config);
        Self {
            documents: HashMap::new(),
            config,
            config_path,
            rules: resolved.rules,
        }
    }

    pub fn config(&self) -> &FalconConfig {
        &self.config
    }

    pub fn document(&self, uri: &str) -> Option<&DocumentState> {
        self.documents.get(uri)
    }

    pub fn open_uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self.documents.keys().cloned().collect();
        uris.sort();
        uris
    }

    /// `textDocument/didOpen`: cache and parse the document, then analyze it.
    pub fn open(&mut self, uri: &str, text: String, version: Option<i32>) -> Vec<Diagnostic> {
        let (program, _parse_errors) = parse(&text);
        self.documents.insert(
            uri.to_string(),
            DocumentState {
                text,
                version,
                program,
                last_diagnostics: Vec::new(),
                parse_count: 1,
                analyze_count: 0,
            },
        );
        self.analyze(uri)
    }

    /// `textDocument/didChange` (full sync): replace text and re-parse the
    /// changed document only. Analysis is the caller's responsibility — the
    /// server loop defers it behind the debounce window.
    ///
    /// Returns false if the document is not open.
    pub fn change(&mut self, uri: &str, text: String, version: Option<i32>) -> bool {
        let Some(doc) = self.documents.get_mut(uri) else {
            warn!(uri, "didChange for unopened document — ignored");
            return false;
        };
        let (program, _parse_errors) = parse(&text);
        doc.text = text;
        doc.version = version;
        doc.program = program;
        doc.parse_count += 1;
        true
    }

    /// `textDocument/didSave`: refresh text if the client included it
    /// (re-parsing only when it actually differs), then analyze.
    pub fn save(&mut self, uri: &str, text: Option<String>) -> Vec<Diagnostic> {
        if let Some(text) = text {
            let differs = self.documents.get(uri).is_some_and(|doc| doc.text != text);
            if differs {
                let version = self.documents.get(uri).and_then(|d| d.version);
                self.change(uri, text, version);
            }
        }
        self.analyze(uri)
    }

    /// `textDocument/didClose`: drop the cache entry.
    pub fn close(&mut self, uri: &str) {
        self.documents.remove(uri);
    }

    /// Run the enabled rules over the cached AST of `uri`. Diagnostics are
    /// sorted by span for deterministic publishing.
    pub fn analyze(&mut self, uri: &str) -> Vec<Diagnostic> {
        let Some(doc) = self.documents.get_mut(uri) else {
            return Vec::new();
        };
        let file_path = uri_to_path(uri);
        let ctx = AnalyzeContext {
            file_path: &file_path,
            source: &doc.text,
            config: &self.config,
        };
        let mut diagnostics: Vec<Diagnostic> = self
            .rules
            .iter()
            .flat_map(|rule| rule.analyze(&doc.program, &ctx))
            .collect();
        // Honor inline `// falcon-ignore` suppressions (the LSP drives rules
        // directly rather than through RuleRegistry::run_all, so it filters and
        // reports malformed comments here too).
        let suppressions =
            FileSuppressions::parse(&doc.text, &file_path.to_string_lossy(), suppression_lookup);
        if !suppressions.is_empty() {
            diagnostics.retain(|diag| {
                let line = suppressions.line_for_offset(diag.span.start);
                !suppressions.is_suppressed(diag.rule, line)
            });
        }
        diagnostics.extend(suppressions.into_diagnostics());
        apply_severities(&mut diagnostics, &self.config);
        diagnostics.sort_by(|a, b| a.span.start.cmp(&b.span.start).then(a.rule.cmp(b.rule)));
        doc.analyze_count += 1;
        doc.last_diagnostics = diagnostics.clone();
        debug!(uri, count = diagnostics.len(), "analyzed document");
        diagnostics
    }

    /// Reload config and rule set, then re-analyze every open document
    /// against its cached AST (no re-parse). Returns per-document results
    /// for the caller to publish.
    pub fn reload_config(&mut self) -> Vec<(String, Vec<Diagnostic>)> {
        self.config = load_from(self.config_path.as_deref());
        let resolved = resolve_rules(&self.config);
        self.rules = resolved.rules;
        debug!(rule_count = self.rules.len(), "config reloaded");
        self.open_uris()
            .into_iter()
            .map(|uri| {
                let diagnostics = self.analyze(&uri);
                (uri, diagnostics)
            })
            .collect()
    }
}

fn load_from(path: Option<&Path>) -> FalconConfig {
    match path {
        Some(p) => load_config(p).unwrap_or_else(|e| {
            warn!(
                "failed to load config from {}: {} — using defaults",
                p.display(),
                e
            );
            FalconConfig::default()
        }),
        None => match std::env::current_dir() {
            Ok(cwd) => load_or_default(&cwd),
            Err(_) => FalconConfig::default(),
        },
    }
}

/// Best-effort conversion of a `file://` URI to a filesystem path for
/// diagnostic attribution. Percent-encoded paths are passed through verbatim
/// (Phase 1; jfit paths are plain ASCII).
pub fn uri_to_path(uri: &str) -> PathBuf {
    PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri))
}
