//! LSP server state: document cache, cached ASTs, config-driven rule set.
//!
//! Caching model (see `.omc/docs/LSP_CACHING_DESIGN.md`): a document's AST is
//! invalidated only by *text* changes; the rule set is invalidated only by
//! *config* changes. A config reload therefore re-runs the new rules over the
//! cached ASTs without re-parsing — by construction there is no
//! stale-AST-with-new-config state.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use falcon_analyze::{
    AnalyzeContext, CrossFileRuleRegistry, FileSuppressions, ProjectFile, Rule,
    syntax_error_diagnostics,
};
use falcon_config::{FalconConfig, load_config, load_or_default};
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    apply_severities, meta::suppression_lookup, resolve_cross_file_rules, resolve_rules,
};
use falcon_syntax::Program;
use glob::Pattern;
use tracing::{debug, warn};
use walkdir::WalkDir;

/// One open document: full text, cached AST, and instrumentation counters.
pub struct DocumentState {
    pub text: String,
    pub version: Option<i32>,
    program: Program,
    /// Whether the last parse produced errors — carried so the cross-file pass
    /// can populate [`ProjectFile::has_parse_errors`] without re-parsing.
    has_parse_errors: bool,
    /// `syntax-error` diagnostics from the last parse, kept so `analyze` can
    /// publish them alongside lints without re-parsing (parity with the CLI).
    syntax_diagnostics: Vec<Diagnostic>,
    /// Most recent published output (byte spans) — read by hover. May include
    /// cross-file diagnostics after a cross-file pass; never an input to analysis.
    pub last_diagnostics: Vec<Diagnostic>,
    /// Whether the last *cross-file pass* published a set carrying cross-file
    /// diagnostics. Lets the pass republish only docs whose cross-file set changed
    /// (adding new cross-file diags, or clearing ones shown before).
    ///
    /// Write it only from the cross-file pass. It deliberately does not track what
    /// the editor currently shows: a didChange republishes per-file diagnostics and
    /// leaves this `true`, which costs at most one redundant publish of identical
    /// content. Clearing it elsewhere would make a stale `false` reachable — the
    /// pass would skip a doc whose cross-file squiggles are still on screen and
    /// never clear them.
    had_cross_file_diags: bool,
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
    /// Cross-file rules; empty unless enabled by config. When empty the
    /// cross-file pass is skipped entirely (the whole-workspace walk is expensive).
    cross_file_rules: CrossFileRuleRegistry,
    /// Root under which the cross-file pass walks `.dart` files: the config's
    /// directory, falling back to the current directory.
    workspace_root: PathBuf,
}

impl LspState {
    /// Create state with config from `config_path`, or discovery from the
    /// current directory when `None` (same order as the CLI: cwd → git root →
    /// `$HOME/.falcon.json` → defaults).
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let config = load_from(config_path.as_deref());
        let resolved = resolve_rules(&config);
        let cross_file_rules = build_cross_file_registry(&config);
        let workspace_root = workspace_root_for(config_path.as_deref());
        Self {
            documents: HashMap::new(),
            config,
            config_path,
            rules: resolved.rules,
            cross_file_rules,
            workspace_root,
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
        let (program, parse_errors) = parse(&text);
        let syntax_diagnostics = syntax_error_diagnostics(&uri_to_path(uri), &parse_errors);
        self.documents.insert(
            uri.to_string(),
            DocumentState {
                text,
                version,
                program,
                has_parse_errors: !parse_errors.is_empty(),
                syntax_diagnostics,
                last_diagnostics: Vec::new(),
                had_cross_file_diags: false,
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
        let (program, parse_errors) = parse(&text);
        doc.syntax_diagnostics = syntax_error_diagnostics(&uri_to_path(uri), &parse_errors);
        doc.text = text;
        doc.version = version;
        doc.program = program;
        doc.has_parse_errors = !parse_errors.is_empty();
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
        // Degraded: the LSP analyzes a single open buffer and has no whole-project
        // view, so it supplies no project index. Resolver-dependent rules fall
        // back to their conservative (no-type-facts) behavior here. A future
        // enhancement could pass `AnalyzeContext::with_project(&ProjectIndex::from_program(..))`
        // for single-file resolution.
        let ctx = AnalyzeContext::new(&file_path, &doc.text, &self.config);
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
        // Syntax errors are surfaced like the CLI: reported regardless of inline
        // suppression (added after the retain above), but still severity-mapped.
        diagnostics.extend(doc.syntax_diagnostics.iter().cloned());
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
        self.cross_file_rules = build_cross_file_registry(&self.config);
        debug!(
            rule_count = self.rules.len(),
            cross_file_rule_count = self.cross_file_rules.rules().len(),
            "config reloaded"
        );
        self.open_uris()
            .into_iter()
            .map(|uri| {
                let diagnostics = self.analyze(&uri);
                (uri, diagnostics)
            })
            .collect()
    }

    /// Whether any cross-file rule is enabled. When false the caller skips the
    /// cross-file pass, avoiding the whole-workspace walk.
    pub fn cross_file_rules_enabled(&self) -> bool {
        !self.cross_file_rules.is_empty()
    }

    /// Run the cross-file rules over the whole workspace and republish the
    /// merged (per-file + cross-file) diagnostics for every open document whose
    /// cross-file set changed. Returns the `(uri, merged)` pairs actually
    /// published so the caller can send them and clear their dirty flags.
    ///
    /// Only open docs are republished: an editor shows diagnostics for open
    /// buffers, and republishing unchanged docs would be redundant traffic.
    pub fn cross_file_pass(&mut self) -> Vec<(String, Vec<Diagnostic>)> {
        if self.cross_file_rules.is_empty() {
            return Vec::new();
        }
        // Compute the cross-file map first so its immutable borrow ends before the
        // per-document `analyze`/mutation loop below (avoids &self vs &mut self).
        let cross_file_map = self.cross_file_diagnostics();
        let mut published = Vec::new();
        for uri in self.open_uris() {
            let path = uri_to_path(&uri).to_string_lossy().into_owned();
            let cross_file_diags = cross_file_map.get(&path);
            let has_now = cross_file_diags.is_some_and(|d| !d.is_empty());
            let had_before = self
                .documents
                .get(&uri)
                .is_some_and(|d| d.had_cross_file_diags);
            // Nothing to add and nothing to clear: leave the last publish intact.
            if !has_now && !had_before {
                continue;
            }
            let mut merged = self.analyze(&uri);
            if let Some(diags) = cross_file_diags {
                merged.extend(diags.iter().cloned());
                merged.sort_by(|a, b| a.span.start.cmp(&b.span.start).then(a.rule.cmp(b.rule)));
            }
            if let Some(doc) = self.documents.get_mut(&uri) {
                doc.last_diagnostics = merged.clone();
                doc.had_cross_file_diags = has_now;
            }
            published.push((uri, merged));
        }
        published
    }

    /// Build the cross-file diagnostics for the whole workspace, grouped by file
    /// path. Open buffers contribute their in-memory text and cached AST; every
    /// other `.dart` file under the workspace root is read and parsed from disk.
    fn cross_file_diagnostics(&self) -> HashMap<String, Vec<Diagnostic>> {
        if self.cross_file_rules.is_empty() {
            return HashMap::new();
        }
        let files = self.collect_cross_file_files();
        let mut diags = self.cross_file_rules.run_all(&files, &self.config);
        suppress_cross_file_diags(&mut diags, &files);
        apply_severities(&mut diags, &self.config);
        let mut grouped: HashMap<String, Vec<Diagnostic>> = HashMap::new();
        for diag in diags {
            grouped
                .entry(diag.file_path.clone())
                .or_default()
                .push(diag);
        }
        grouped
    }

    /// Assemble the [`ProjectFile`] set: every non-excluded `.dart` file under
    /// the workspace root, preferring an open buffer's text + cached AST over the
    /// on-disk copy so unsaved edits are reflected in cross-file analysis.
    fn collect_cross_file_files(&self) -> Vec<ProjectFile> {
        let exclude = compile_patterns(&self.config.files.exclude_patterns());
        let includes = compile_patterns(&self.config.files.include_patterns());
        let open_by_path: HashMap<PathBuf, &DocumentState> = self
            .documents
            .iter()
            .map(|(uri, doc)| (uri_to_path(uri), doc))
            .collect();

        let mut files = Vec::new();
        for entry in WalkDir::new(&self.workspace_root).follow_links(true) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("error walking workspace: {}", e);
                    continue;
                }
            };
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("dart") {
                continue;
            }
            let path_str = path.to_string_lossy();
            if exclude.iter().any(|p| p.matches(&path_str)) {
                continue;
            }
            if !includes.is_empty() && !includes.iter().any(|p| p.matches(&path_str)) {
                continue;
            }
            if let Some(doc) = open_by_path.get(path) {
                files.push(ProjectFile {
                    path: path.to_path_buf(),
                    source: doc.text.clone(),
                    program: doc.program.clone(),
                    has_parse_errors: doc.has_parse_errors,
                });
            } else {
                match std::fs::read_to_string(path) {
                    Ok(source) => {
                        let (program, errors) = parse(&source);
                        files.push(ProjectFile {
                            path: path.to_path_buf(),
                            source,
                            program,
                            has_parse_errors: !errors.is_empty(),
                        });
                    }
                    Err(e) => warn!("failed to read {}: {}", path.display(), e),
                }
            }
        }
        files
    }
}

/// Build a cross-file-rule registry from `config` (empty unless a cross-file
/// rule is enabled), mirroring the CLI's `build_cross_file_registry`.
fn build_cross_file_registry(config: &FalconConfig) -> CrossFileRuleRegistry {
    let mut registry = CrossFileRuleRegistry::new();
    for rule in resolve_cross_file_rules(config).rules {
        registry.register(rule);
    }
    registry
}

/// The directory the cross-file pass walks: the config file's parent, else the
/// current directory (`.` if even that is unavailable).
fn workspace_root_for(config_path: Option<&Path>) -> PathBuf {
    config_path
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

/// Compile glob patterns, logging and skipping invalid ones.
fn compile_patterns(patterns: &[String]) -> Vec<Pattern> {
    patterns
        .iter()
        .filter_map(|p| match Pattern::new(p) {
            Ok(pat) => Some(pat),
            Err(_) => {
                warn!("invalid glob pattern: {}", p);
                None
            }
        })
        .collect()
}

/// Honor inline `// falcon-ignore` suppressions for cross-file-rule diagnostics,
/// mirroring `falcon_cli::analyze_pipeline::suppress_cross_file_diags`. Only
/// filters; malformed-suppression diagnostics are reported by the per-file pass.
fn suppress_cross_file_diags(diags: &mut Vec<Diagnostic>, files: &[ProjectFile]) {
    if diags.is_empty() {
        return;
    }
    let sources: HashMap<String, &str> = files
        .iter()
        .map(|f| (f.path.to_string_lossy().into_owned(), f.source.as_str()))
        .collect();
    let mut cache: HashMap<String, FileSuppressions> = HashMap::new();
    diags.retain(|diag| {
        let Some(src) = sources.get(&diag.file_path) else {
            return true;
        };
        let sup = cache
            .entry(diag.file_path.clone())
            .or_insert_with(|| FileSuppressions::parse(src, &diag.file_path, suppression_lookup));
        if sup.is_empty() {
            return true;
        }
        let line = sup.line_for_offset(diag.span.start);
        !sup.is_suppressed(diag.rule, line)
    });
}

fn load_from(path: Option<&Path>) -> FalconConfig {
    let mut config = match path {
        Some(p) => load_config(p).unwrap_or_else(|e| {
            warn!(
                "failed to load config from {}: {} — using defaults",
                p.display(),
                e
            );
            FalconConfig::default()
        }),
        None => match std::env::current_dir() {
            Ok(cwd) => load_or_default(&cwd).unwrap_or_else(|e| {
                warn!("failed to load discovered config: {e} — using defaults");
                FalconConfig::default()
            }),
            Err(_) => FalconConfig::default(),
        },
    };
    // Rewrite any legacy rule ids in the config to their canonical ids so old
    // falcon.json files keep resolving.
    falcon_rules::meta::canonicalize_config(&mut config);
    config
}

/// Best-effort conversion of a `file://` URI to a filesystem path for
/// diagnostic attribution. Percent-encoded paths are passed through verbatim
/// (Phase 1; jfit paths are plain ASCII).
pub fn uri_to_path(uri: &str) -> PathBuf {
    PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri))
}
