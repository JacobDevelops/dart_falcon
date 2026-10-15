//! LSP server state: document cache, cached ASTs, config-driven rule set.
//!
//! Text changes invalidate a document's AST; config changes rerun rules over
//! cached ASTs, reparsing only when the effective language version changes.
//! Unresolved filesystem identities are retried before analysis when files materialize.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use lsp_types::Uri;

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, CrossFileRuleRegistry, ExternalLibraries, ExternalSources,
    IdentityIndex, IdentitySource, LibraryGrouping, LibrarySource, PackageIdentity, ProjectFile,
    RuleRegistry, SemanticIndexes, SignatureIndex, TypeIndex, canonical_or_lexical,
    group_libraries, import_path, library_unit, syntax_error_diagnostics,
};
use falcon_config::{FalconConfig, load_config, load_or_default};
use falcon_dart_parser::{parse, parse_for_package};
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    apply_severities, build_cross_file_registry, build_registry, package_identities,
    requires_resolution, run_cross_file,
};
use falcon_syntax::{LanguageVersion, Program};
use tracing::{debug, warn};
use walkdir::WalkDir;

/// One open document: full text, cached AST, and instrumentation counters.
pub struct DocumentState {
    pub text: String,
    path: PathBuf,
    /// Only unresolved paths need filesystem checks between document events.
    unresolved_path: bool,
    pub version: Option<i32>,
    program: Program,
    language_version: LanguageVersion,
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

/// One immutable semantic view of the workspace. It owns the parsed files and
/// every index derived from them, so all documents affected by one edit share the
/// same facts instead of walking, parsing, and indexing the workspace repeatedly.
struct SemanticSnapshot {
    files: Vec<ProjectFile>,
    indexes: SemanticIndexes,
    packages: Vec<PackageIdentity>,
    grouping: LibraryGrouping,
    by_path: HashMap<PathBuf, usize>,
    reverse_dependencies: Vec<Vec<usize>>,
}

/// Reachability metadata survives even when project back-edges prevent layering.
struct ExternalTopology {
    library_count: usize,
    paths: Vec<PathBuf>,
    project_dependency_targets: HashSet<PathBuf>,
}

/// External-only facts are shared by successive project overlays.
struct ExternalIndexCache {
    identities: Arc<IdentityIndex>,
    types: Arc<TypeIndex>,
    signatures: Arc<SignatureIndex>,
}

enum ProjectView {
    Semantic,
    CrossFile,
}

/// Server-side cache: open documents, active config, enabled rule set.
pub struct LspState {
    documents: HashMap<String, DocumentState>,
    config: FalconConfig,
    config_path: Option<PathBuf>,
    rules: RuleRegistry,
    /// Cross-file rules; empty unless enabled by config. When empty the
    /// cross-file pass is skipped entirely (the whole-workspace walk is expensive).
    cross_file_rules: CrossFileRuleRegistry,
    /// Root under which the cross-file pass walks `.dart` files: the config's
    /// directory, falling back to the current directory.
    workspace_root: PathBuf,
    /// Whether any enabled per-file rule consumes resolver-backed semantic facts.
    resolve_semantics: bool,
    /// Lazily rebuilt after document, config, or watched-workspace changes.
    semantic_snapshot: Option<SemanticSnapshot>,
    /// Dependents from the graph before an edit, retained until that edit is analyzed.
    pending_semantic_affected: HashMap<String, HashSet<String>>,
    /// Instrumentation for incremental tests: number of semantic snapshot builds.
    semantic_snapshot_build_count: u64,
    /// Instrumentation for incremental tests: number of dependency topology builds.
    semantic_topology_build_count: u64,
    /// Package configurations, language versions and SDKs of analyzed files.
    environment: AnalysisEnvironment,
    /// Resolution inputs captured on discovery or watched configuration events,
    /// never polled on ordinary project edits.
    environment_stamps: Option<falcon_analyze::environment::EnvironmentCacheKey>,
    /// Versioned hosted/SDK facts are immutable; mutable roots use client events.
    external_libraries: ExternalLibraries,
    /// Instrumentation for incremental tests: number of external-library resets.
    external_reset_count: u64,
    external_indexes: Option<ExternalIndexCache>,
    external_topology: Option<ExternalTopology>,
    watched_dependency_generation: u64,
    external_index_build_count: u64,
}

impl LspState {
    /// Create state with config from `config_path`, or discovery from the
    /// current directory when `None` (same order as the CLI: cwd → git root →
    /// `$HOME/.falcon.json` → defaults).
    pub fn new(config_path: Option<PathBuf>) -> Self {
        Self::new_with_sdk_locator(config_path, falcon_analyze::environment::DartSdk::locate)
    }

    /// Create state with explicit SDK discovery, allowing hermetic fixtures.
    pub fn new_with_sdk_locator(
        config_path: Option<PathBuf>,
        locator: fn(
            Option<&falcon_analyze::environment::PackageConfig>,
        ) -> Option<falcon_analyze::environment::DartSdk>,
    ) -> Self {
        let config = load_from(config_path.as_deref());
        let rules = build_registry(&config);
        let cross_file_rules = build_cross_file_registry(&config);
        let workspace_root = workspace_root_for(config_path.as_deref());
        let resolve_semantics = requires_resolution(&rules);
        Self {
            documents: HashMap::new(),
            config,
            config_path,
            rules,
            cross_file_rules,
            workspace_root,
            resolve_semantics,
            semantic_snapshot: None,
            pending_semantic_affected: HashMap::new(),
            semantic_snapshot_build_count: 0,
            semantic_topology_build_count: 0,
            environment: AnalysisEnvironment::with_sdk_locator(locator),
            environment_stamps: None,
            external_libraries: ExternalLibraries::default(),
            external_reset_count: 0,
            external_indexes: None,
            external_topology: None,
            watched_dependency_generation: 0,
            external_index_build_count: 0,
        }
    }

    /// Mutable package roots for native LSP file watching. Versioned hosted
    /// packages and SDK sources are intentionally absent.
    pub fn watched_dependency_roots(&self) -> Vec<PathBuf> {
        self.environment
            .watched_dependency_roots()
            .map(Path::to_path_buf)
            .collect()
    }

    /// Changes only when discovery or invalidation can alter dependency watches.
    pub fn watched_dependency_generation(&self) -> u64 {
        self.watched_dependency_generation
    }

    /// Process one native watcher batch. Project-only saves retain the shared
    /// external indexes; mutable dependencies or changed resolution inputs drop
    /// the external graph once, then all open documents share the rebuilt view.
    pub fn watched_files_changed(&mut self, paths: &[PathBuf]) -> Vec<(String, Vec<Diagnostic>)> {
        if paths.is_empty() {
            return Vec::new();
        }
        let rules_changed = paths.iter().any(|path| {
            matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("falcon.json" | ".falcon.json")
            )
        });
        // Discovery can capture new disk bytes while retaining a known context.
        // A delivered input event must therefore invalidate even an equal key.
        let inputs_changed = self
            .environment_stamps
            .as_ref()
            .is_some_and(|key| paths.iter().any(|path| key.affected_by(path)));
        let dependency_changed = paths.iter().any(|path| {
            self.environment
                .watched_dependency_roots()
                .any(|root| path.starts_with(root))
                && !self
                    .documents
                    .values()
                    .any(|document| document.path == *path)
                && !self
                    .semantic_snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.by_path.contains_key(path))
                && !self.watched_project_source(path)
        });
        let project_changed = paths.iter().any(|path| {
            path.starts_with(&self.workspace_root)
                && !self
                    .documents
                    .values()
                    .any(|document| document.path == *path)
        });
        if !inputs_changed && !dependency_changed && !project_changed && !rules_changed {
            return Vec::new();
        }
        if inputs_changed || dependency_changed {
            self.reset_external_environment();
        }
        self.pending_semantic_affected.clear();
        self.invalidate_semantic_snapshot();
        if rules_changed {
            return self.reload_config();
        }
        self.open_uris()
            .into_iter()
            .map(|uri| {
                let diagnostics = self.analyze(&uri);
                (uri, diagnostics)
            })
            .collect()
    }

    fn reset_external_environment(&mut self) {
        self.environment.reset();
        self.environment_stamps = None;
        self.external_libraries = ExternalLibraries::default();
        self.external_indexes = None;
        self.external_topology = None;
        self.external_reset_count += 1;
        self.watched_dependency_generation += 1;
    }

    /// Discover new directories without polling dependency/configuration files.
    /// Configuration changes enter through `watched_files_changed`.
    fn refresh_environment<'a>(&mut self, paths: impl Iterator<Item = &'a Path> + Clone) {
        if !self.environment.covers(paths.clone())
            || !self
                .environment
                .covers(self.documents.values().map(|doc| doc.path.as_path()))
        {
            self.environment
                .extend(self.documents.values().map(|doc| doc.path.as_path()));
            self.environment.extend(paths);
            self.environment_stamps = Some(self.environment.config_stamps());
            self.watched_dependency_generation += 1;
        }
    }

    /// Keep cached grammar in sync with package ownership and marker overrides.
    fn refresh_document_versions(&mut self) {
        let mut changed = false;
        for doc in self.documents.values_mut() {
            let language_version = self.environment.language_version(&doc.path, &doc.text);
            if language_version == doc.language_version {
                continue;
            }
            let (program, errors) = parse_for_package(&doc.text, Some(language_version));
            doc.program = program;
            doc.has_parse_errors = !errors.is_empty();
            doc.syntax_diagnostics = syntax_error_diagnostics(&doc.path, &errors);
            doc.language_version = language_version;
            doc.parse_count += 1;
            changed = true;
        }
        if changed {
            self.invalidate_semantic_snapshot();
        }
    }

    pub fn document(&self, uri: &str) -> Option<&DocumentState> {
        self.documents.get(uri)
    }

    pub fn open_uris(&self) -> Vec<String> {
        let mut uris: Vec<String> = self.documents.keys().cloned().collect();
        uris.sort();
        uris
    }

    /// Number of workspace semantic snapshots built since state creation.
    pub fn semantic_snapshot_build_count(&self) -> u64 {
        self.semantic_snapshot_build_count
    }

    /// Number of times loaded external libraries were dropped because a
    /// package configuration or dependency changed, or a missing part appeared.
    pub fn external_reset_count(&self) -> u64 {
        self.external_reset_count
    }

    /// External base builds, including conservative combined graph rebuilds.
    pub fn external_index_build_count(&self) -> u64 {
        self.external_index_build_count
    }

    /// Number of reverse-dependency topologies built since state creation.
    pub fn semantic_topology_build_count(&self) -> u64 {
        self.semantic_topology_build_count
    }

    /// `textDocument/didOpen`: cache and parse the document, then analyze it.
    pub fn open(&mut self, uri: &str, text: String, version: Option<i32>) -> Vec<Diagnostic> {
        let (path, unresolved_path) = document_path(uri);
        self.pending_semantic_affected.remove(uri);
        if self.resolve_semantics && !self.documents.is_empty() {
            self.ensure_semantic_snapshot();
            if let Some(snapshot) = self.semantic_snapshot.as_ref() {
                let affected = affected_open_uris_in_snapshot(snapshot, &path, &self.documents);
                self.pending_semantic_affected
                    .insert(uri.to_string(), affected.into_iter().collect());
            }
        }
        self.refresh_environment(std::iter::once(path.as_path()));
        let package_version = self.environment.package_language_version(&path);
        let (program, parse_errors) = parse_for_package(&text, package_version);
        let language_version = self.environment.language_version(&path, &text);
        let syntax_diagnostics = syntax_error_diagnostics(&path, &parse_errors);
        self.documents.insert(
            uri.to_string(),
            DocumentState {
                path,
                unresolved_path,
                text,
                version,
                program,
                language_version,
                has_parse_errors: !parse_errors.is_empty(),
                syntax_diagnostics,
                last_diagnostics: Vec::new(),
                had_cross_file_diags: false,
                parse_count: 1,
                analyze_count: 0,
            },
        );
        self.invalidate_semantic_snapshot();
        self.analyze(uri)
    }

    /// `textDocument/didChange` (full sync): replace text and re-parse the
    /// changed document only. Unresolved paths refresh during analysis, while
    /// resolved identities survive deletion. The server loop defers analysis
    /// behind the debounce window.
    ///
    /// Returns false if the document is not open.
    pub fn change(&mut self, uri: &str, text: String, version: Option<i32>) -> bool {
        if !self.documents.contains_key(uri) {
            warn!(uri, "didChange for unopened document — ignored");
            return false;
        }
        if self.resolve_semantics {
            // Keep the old dependency graph without parsing text about to be
            // replaced under a newly changed package version or path identity.
            if !self.pending_semantic_affected.contains_key(uri)
                && !self.documents[uri].unresolved_path
            {
                self.ensure_semantic_snapshot();
            }
            if let Some(snapshot) = self.semantic_snapshot.as_ref() {
                let changed_path = &self.documents[uri].path;
                let affected =
                    affected_open_uris_in_snapshot(snapshot, changed_path, &self.documents);
                self.pending_semantic_affected
                    .entry(uri.to_string())
                    .or_default()
                    .extend(affected);
            }
        }

        self.refresh_unresolved_paths();
        let path = self.documents[uri].path.clone();
        self.refresh_environment(std::iter::once(path.as_path()));
        let package_version = self.environment.package_language_version(&path);
        let doc = self.documents.get_mut(uri).expect("document checked above");
        let (program, parse_errors) = parse_for_package(&text, package_version);
        doc.syntax_diagnostics = syntax_error_diagnostics(&doc.path, &parse_errors);
        doc.text = text;
        doc.version = version;
        doc.program = program;
        doc.language_version = self.environment.language_version(&doc.path, &doc.text);
        doc.has_parse_errors = !parse_errors.is_empty();
        doc.parse_count += 1;
        self.invalidate_semantic_snapshot();
        true
    }

    /// `textDocument/didSave`: refresh filesystem identity when resolvable and
    /// any supplied text (re-parsing only when it actually differs), then analyze.
    pub fn save(&mut self, uri: &str, text: Option<String>) -> Vec<Diagnostic> {
        if let Some(doc) = self.documents.get(uri) {
            let (path, unresolved_path) = document_path(uri);
            if !unresolved_path && path != doc.path {
                if self.resolve_semantics {
                    let affected = self.affected_open_uris(uri);
                    self.pending_semantic_affected
                        .entry(uri.to_string())
                        .or_default()
                        .extend(affected);
                }
                let doc = self.documents.get_mut(uri).expect("document checked above");
                let file_path = path.to_string_lossy();
                for diagnostic in &mut doc.syntax_diagnostics {
                    diagnostic.file_path.clear();
                    diagnostic.file_path.push_str(&file_path);
                }
                doc.path = path;
                self.invalidate_semantic_snapshot();
            }
            self.documents
                .get_mut(uri)
                .expect("document checked above")
                .unresolved_path &= unresolved_path;
        }
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
        self.pending_semantic_affected.remove(uri);
        if self.documents.remove(uri).is_some() {
            self.invalidate_semantic_snapshot();
        }
    }

    /// Run the enabled rules over the cached AST of `uri`. Diagnostics are
    /// sorted by span for deterministic publishing.
    pub fn analyze(&mut self, uri: &str) -> Vec<Diagnostic> {
        if !self.documents.contains_key(uri) {
            return Vec::new();
        }
        if self.resolve_semantics {
            self.ensure_semantic_snapshot();
        } else {
            self.refresh_unresolved_paths();
        }
        let file_path = self.documents[uri].path.clone();
        self.refresh_environment(std::iter::once(file_path.as_path()));
        self.refresh_document_versions();

        let semantic = self.semantic_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .by_path
                .get(&file_path)
                .copied()
                .map(|file_index| (snapshot, file_index))
        });
        let Some(doc) = self.documents.get_mut(uri) else {
            return Vec::new();
        };
        let mut ctx = AnalyzeContext::new(&doc.path, &doc.text, &self.config)
            .with_environment(Some(&self.environment));
        let programs;
        let sources;
        let library;
        if let Some((snapshot, file_index)) = semantic {
            programs = snapshot
                .files
                .iter()
                .map(|file| &file.program)
                .collect::<Vec<_>>();
            sources = snapshot
                .files
                .iter()
                .map(|file| file.source.as_str())
                .collect::<Vec<_>>();
            library = library_unit(&snapshot.grouping, &programs, &sources, file_index);
            ctx = ctx
                .with_types(&snapshot.indexes.types)
                .with_identities(&snapshot.indexes.identities)
                .with_signatures(&snapshot.indexes.signatures)
                .with_library(&library);
        }
        let mut diagnostics = self.rules.run_all(&doc.program, &ctx);
        diagnostics.extend(doc.syntax_diagnostics.iter().cloned());
        apply_severities(&mut diagnostics, &self.config);
        diagnostics.sort_by(|a, b| a.span.start.cmp(&b.span.start).then(a.rule.cmp(b.rule)));
        doc.analyze_count += 1;
        doc.last_diagnostics = diagnostics.clone();
        debug!(uri, count = diagnostics.len(), "analyzed document");
        diagnostics
    }

    /// Re-analyze the union of open documents affected by several changed URIs.
    pub fn analyze_affected_many(&mut self, uris: &[String]) -> Vec<(String, Vec<Diagnostic>)> {
        let mut affected = uris
            .iter()
            .flat_map(|uri| self.affected_open_uris(uri))
            .collect::<Vec<_>>();
        affected.sort();
        affected.dedup();
        affected
            .into_iter()
            .map(|uri| {
                let diagnostics = self.analyze(&uri);
                (uri, diagnostics)
            })
            .collect()
    }

    /// Open documents affected by a semantic change in `uri`, including `uri`.
    pub(crate) fn affected_open_uris(&mut self, uri: &str) -> Vec<String> {
        if !self.resolve_semantics {
            return if self.documents.contains_key(uri) {
                vec![uri.to_string()]
            } else {
                Vec::new()
            };
        }
        self.ensure_semantic_snapshot();
        let changed_path = self.documents.get(uri).map_or_else(
            || canonical_or_lexical(&uri_to_path(uri)),
            |doc| doc.path.clone(),
        );
        let mut uris = self
            .semantic_snapshot
            .as_ref()
            .map(|snapshot| {
                affected_open_uris_in_snapshot(snapshot, &changed_path, &self.documents)
            })
            .unwrap_or_default();
        if let Some(previous) = self.pending_semantic_affected.remove(uri) {
            uris.extend(previous);
        }
        if self.documents.contains_key(uri) {
            uris.push(uri.to_string());
        }
        uris.retain(|affected_uri| self.documents.contains_key(affected_uri));
        uris.sort();
        uris.dedup();
        uris
    }

    /// Reload config and rule set, then re-analyze every open document
    /// against its cached AST (no re-parse). Returns per-document results
    /// for the caller to publish.
    pub fn reload_config(&mut self) -> Vec<(String, Vec<Diagnostic>)> {
        self.config = load_from(self.config_path.as_deref());
        self.rules = build_registry(&self.config);
        self.resolve_semantics = requires_resolution(&self.rules);
        self.cross_file_rules = build_cross_file_registry(&self.config);
        self.pending_semantic_affected.clear();
        self.invalidate_semantic_snapshot();
        if self
            .environment_stamps
            .as_ref()
            .is_some_and(|key| *key != self.environment.config_stamps())
        {
            self.reset_external_environment();
        }
        debug!(
            rule_count = self.rules.rules().len(),
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
    /// published; pending semantic dependent refreshes remain the caller's job.
    ///
    /// Only open docs are republished: an editor shows diagnostics for open
    /// buffers, and republishing unchanged docs would be redundant traffic.
    pub fn cross_file_pass(&mut self) -> Vec<(String, Vec<Diagnostic>)> {
        if self.cross_file_rules.is_empty() {
            return Vec::new();
        }
        self.refresh_unresolved_paths();
        // Compute the cross-file map first so its immutable borrow ends before the
        // per-document `analyze`/mutation loop below (avoids &self vs &mut self).
        let cross_file_map = self.cross_file_diagnostics();
        let mut published = Vec::new();
        for uri in self.open_uris() {
            let path = self.documents[&uri].path.to_string_lossy();
            let cross_file_diags = cross_file_map.get(path.as_ref());
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
    fn cross_file_diagnostics(&mut self) -> HashMap<String, Vec<Diagnostic>> {
        if self.cross_file_rules.is_empty() {
            return HashMap::new();
        }
        let files = self.collect_project_files(ProjectView::CrossFile);
        // Semantic views include unsaved buffers; cross-file views include manifests.
        // Reuse facts only when Dart revisions and package identities agree exactly.
        let indexes = self.semantic_snapshot.as_ref().and_then(|snapshot| {
            let mut dart_files = files.iter().filter(|file| {
                file.path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    == Some("dart")
            });
            let same_files = dart_files.clone().count() == snapshot.files.len()
                && dart_files.all(|file| {
                    snapshot.by_path.get(&file.path).is_some_and(|&index| {
                        let cached = &snapshot.files[index];
                        cached.source == file.source
                            && cached.has_parse_errors == file.has_parse_errors
                    })
                });
            if !same_files {
                return None;
            }
            let packages = package_identities(files.iter().map(|file| file.path.as_path()));
            let same_packages = packages.len() == snapshot.packages.len()
                && packages
                    .iter()
                    .zip(&snapshot.packages)
                    .all(|(current, cached)| {
                        current.name == cached.name && current.lib_root == cached.lib_root
                    });
            same_packages.then_some(&snapshot.indexes)
        });
        let diags = run_cross_file(&self.cross_file_rules, &files, &self.config, indexes);
        let mut grouped: HashMap<String, Vec<Diagnostic>> = HashMap::new();
        for diag in diags {
            grouped
                .entry(diag.file_path.clone())
                .or_default()
                .push(diag);
        }
        grouped
    }

    fn invalidate_semantic_snapshot(&mut self) {
        self.semantic_snapshot = None;
    }

    fn refresh_unresolved_paths(&mut self) {
        let resolved: Vec<_> = self
            .documents
            .iter()
            .filter(|(_, doc)| doc.unresolved_path)
            .filter_map(|(uri, _)| {
                let (path, unresolved) = document_path(uri);
                (!unresolved).then(|| (uri.clone(), path))
            })
            .collect();
        let mut changed = false;
        for (uri, path) in resolved {
            let doc = &self.documents[&uri];
            if path != doc.path {
                if let Some(snapshot) = self.semantic_snapshot.as_ref() {
                    self.pending_semantic_affected
                        .entry(uri.clone())
                        .or_default()
                        .extend(affected_open_uris_in_snapshot(
                            snapshot,
                            &doc.path,
                            &self.documents,
                        ));
                }
                let doc = self
                    .documents
                    .get_mut(&uri)
                    .expect("resolved open document");
                let file_path = path.to_string_lossy();
                for diagnostic in &mut doc.syntax_diagnostics {
                    diagnostic.file_path.clear();
                    diagnostic.file_path.push_str(&file_path);
                }
                doc.path = path;
                changed = true;
            }
            self.documents
                .get_mut(&uri)
                .expect("resolved open document")
                .unresolved_path = false;
        }
        if changed {
            self.invalidate_semantic_snapshot();
        }
    }

    fn ensure_semantic_snapshot(&mut self) {
        self.refresh_unresolved_paths();
        self.refresh_environment(std::iter::empty());
        self.refresh_document_versions();
        if self.semantic_snapshot.is_some() || !self.resolve_semantics {
            return;
        }
        let mut files = self.collect_project_files(ProjectView::Semantic);
        {
            let project_paths: HashSet<&Path> =
                files.iter().map(|file| file.path.as_path()).collect();
            let roots: Vec<(&Path, &Program, &str)> = files
                .iter()
                .map(|file| (file.path.as_path(), &file.program, file.source.as_str()))
                .collect();
            self.external_libraries
                .extend(&self.environment, &roots, |path| {
                    project_paths.contains(path)
                });
        };
        if !self
            .environment
            .covers(self.external_libraries.project_companions())
        {
            self.environment
                .extend(self.external_libraries.project_companions());
            self.environment_stamps = Some(self.environment.config_stamps());
            self.watched_dependency_generation += 1;
        }
        // An open dependency part promotes its whole library to the project
        // view. Keep the buffered member and read only its unopened companions.
        let mut seen: HashSet<PathBuf> = files.iter().map(|file| file.path.clone()).collect();
        for path in self.external_libraries.project_companions() {
            if !seen.insert(path.to_path_buf()) {
                continue;
            }
            match std::fs::read_to_string(path) {
                Ok(source) => {
                    let (program, errors) =
                        parse_for_package(&source, self.environment.package_language_version(path));
                    files.push(ProjectFile {
                        path: path.to_path_buf(),
                        source,
                        program,
                        has_parse_errors: !errors.is_empty(),
                    });
                }
                Err(error) => warn!("failed to read {}: {}", path.display(), error),
            }
        }
        let semantic_files = files
            .iter()
            .map(|file| (file.path.clone(), &file.program))
            .collect::<Vec<_>>();
        let external = ExternalSources {
            libraries: &self.external_libraries,
            environment: &self.environment,
        };
        let grouping = group_libraries(&semantic_files);
        let identity_sources = files
            .iter()
            .map(|file| IdentitySource {
                path: &file.path,
                program: &file.program,
                has_parse_errors: file.has_parse_errors,
            })
            .collect::<Vec<_>>();
        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        let sources: Vec<&str> = files.iter().map(|file| file.source.as_str()).collect();
        // A dependency importing a project file depends on mutable buffer facts.
        // Keep the ordinary combined build for that graph instead of freezing
        // unresolved/project references into the immutable external layer.
        let topology_matches = self.external_topology.as_ref().is_some_and(|cache| {
            cache.library_count == self.external_libraries.libraries().len()
                && cache.paths.iter().map(PathBuf::as_path).eq(self
                    .external_libraries
                    .libraries()
                    .iter()
                    .flat_map(|library| &library.files)
                    .map(|file| file.path.as_path()))
        });
        // URI resolution can canonicalize paths. Retain these immutable graph
        // edges rather than repeating filesystem work for every text edit.
        let new_targets = (!topology_matches).then(|| {
            let capacity = self
                .external_libraries
                .libraries()
                .iter()
                .flat_map(|library| &library.files)
                .map(|file| {
                    1 + file.facts.imports.len() + file.facts.exports.len() + file.doc_imports.len()
                })
                .sum();
            let mut targets = HashSet::with_capacity(capacity);
            for file in self
                .external_libraries
                .libraries()
                .iter()
                .flat_map(|library| &library.files)
            {
                targets.insert(file.path.clone());
                for uri in file
                    .facts
                    .imports
                    .iter()
                    .map(|import| import.uri.value.as_str())
                    .chain(
                        file.facts
                            .exports
                            .iter()
                            .map(|export| export.uri.value.as_str()),
                    )
                    .chain(file.doc_imports.iter().map(String::as_str))
                {
                    if let Some(path) =
                        self.external_libraries
                            .resolve_uri(&self.environment, &file.path, uri)
                    {
                        targets.insert(path);
                    }
                }
            }
            targets
        });
        let external_depends_on_project = new_targets
            .as_ref()
            .or_else(|| {
                self.external_topology
                    .as_ref()
                    .map(|cache| &cache.project_dependency_targets)
            })
            .is_some_and(|targets| files.iter().any(|file| targets.contains(&file.path)));
        if let Some(project_dependency_targets) = new_targets {
            self.external_topology = Some(ExternalTopology {
                library_count: self.external_libraries.libraries().len(),
                paths: self
                    .external_libraries
                    .libraries()
                    .iter()
                    .flat_map(|library| &library.files)
                    .map(|file| file.path.clone())
                    .collect(),
                project_dependency_targets,
            });
        }
        if external_depends_on_project {
            self.external_indexes = None;
        } else if !topology_matches || self.external_indexes.is_none() {
            let identities = Arc::new(IdentityIndex::build_external(external));
            let mut types = TypeIndex::from_resolved_files(std::iter::empty(), &identities);
            types.add_external(external.libraries, &identities);
            let types = Arc::new(types);
            let signatures = Arc::new(SignatureIndex::build(
                &[],
                Some(external.libraries),
                &identities,
                &types,
            ));
            self.external_indexes = Some(ExternalIndexCache {
                identities,
                types,
                signatures,
            });
            self.external_index_build_count += 1;
        }
        let identities = if let Some(cache) = &self.external_indexes {
            IdentityIndex::with_external(
                &identity_sources,
                &sources,
                &packages,
                external,
                Arc::clone(&cache.identities),
            )
        } else {
            self.external_index_build_count += 1;
            IdentityIndex::build(&identity_sources, &sources, &packages, Some(external))
        };
        let resolved_files = || {
            files.iter().enumerate().map(|(index, file)| {
                (
                    file.path.as_path(),
                    LibrarySource {
                        program: &file.program,
                        has_parse_errors: file.has_parse_errors,
                        has_unresolved_parts: grouping.is_unresolved(index),
                    },
                )
            })
        };
        let mut types = if let Some(cache) = &self.external_indexes {
            TypeIndex::with_external(resolved_files(), &identities, Arc::clone(&cache.types))
        } else {
            TypeIndex::from_resolved_files(resolved_files(), &identities)
        };
        if self.external_indexes.is_none() {
            types.add_external(external.libraries, &identities);
        }
        let signatures = if let Some(cache) = &self.external_indexes {
            SignatureIndex::with_external(
                &semantic_files,
                &identities,
                &types,
                Arc::clone(&cache.signatures),
            )
        } else {
            SignatureIndex::build(
                &semantic_files,
                Some(external.libraries),
                &identities,
                &types,
            )
        };
        let by_path = files
            .iter()
            .enumerate()
            .map(|(index, file)| (file.path.clone(), index))
            .collect();
        let reverse_dependencies = build_reverse_dependencies(
            &files,
            &grouping,
            &by_path,
            &packages,
            &mut self.semantic_topology_build_count,
        );
        self.semantic_snapshot = Some(SemanticSnapshot {
            files,
            indexes: SemanticIndexes {
                types,
                identities,
                signatures,
            },
            packages,
            grouping,
            by_path,
            reverse_dependencies,
        });
        self.semantic_snapshot_build_count += 1;
    }

    fn admits_project_dart(&self, path: &Path) -> bool {
        path.extension().and_then(|extension| extension.to_str()) == Some("dart")
            && self.config.files.includes.admits(&path.to_string_lossy())
    }

    /// Stable ownership while a debounced edit has dropped the snapshot.
    /// Missing files, directories and outside-target symlinks remain conservative.
    fn watched_project_source(&self, path: &Path) -> bool {
        path.starts_with(&self.workspace_root)
            && self.admits_project_dart(path)
            && path.is_file()
            && std::fs::canonicalize(path).ok().is_some_and(|resolved| {
                resolved.starts_with(canonical_or_lexical(&self.workspace_root))
            })
    }

    /// Assemble the [`ProjectFile`] set: every non-excluded `.dart` file under
    /// the workspace root, preferring an open buffer's text + cached AST over the
    /// on-disk copy. Semantic analysis also includes open buffers that do not exist
    /// on disk yet. Cross-file analysis can additionally include package manifests.
    fn collect_project_files(&mut self, view: ProjectView) -> Vec<ProjectFile> {
        let filter = &self.config.files.includes;
        let mut paths = Vec::new();
        // Never follow symlinks: Flutter's ephemeral/.plugin_symlinks point into
        // the pub cache (same hazard the CLI walker fixed) and a long-lived LSP
        // would hold all of it in memory.
        for entry in WalkDir::new(&self.workspace_root).follow_links(false) {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("error walking workspace: {}", e);
                    continue;
                }
            };
            let path = entry.path();
            let is_dart = path.extension().and_then(|e| e.to_str()) == Some("dart");
            let is_manifest = matches!(view, ProjectView::CrossFile)
                && path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml");
            if !is_dart && !is_manifest {
                continue;
            }
            let path_str = path.to_string_lossy();
            if !is_dart && filter.excludes(&path_str) {
                continue;
            }
            if is_dart && !self.admits_project_dart(path) {
                continue;
            }
            paths.push((canonical_or_lexical(path), is_dart));
        }
        self.refresh_environment(paths.iter().map(|(path, _)| path.as_path()));
        self.refresh_document_versions();
        let open_by_path: HashMap<&Path, &DocumentState> = self
            .documents
            .values()
            .map(|doc| (doc.path.as_path(), doc))
            .collect();
        let capacity = paths.len() + self.documents.len();
        let mut files = Vec::with_capacity(capacity);
        let mut seen = HashSet::with_capacity(capacity);
        for (normalized, is_dart) in paths {
            let path = normalized.as_path();
            seen.insert(normalized.clone());
            if let Some(doc) = open_by_path.get(path) {
                files.push(ProjectFile {
                    path: normalized,
                    source: doc.text.clone(),
                    program: doc.program.clone(),
                    has_parse_errors: doc.has_parse_errors,
                });
            } else {
                match std::fs::read_to_string(path) {
                    Ok(source) => {
                        let (program, errors) = if is_dart {
                            let package_version = self.environment.package_language_version(path);
                            parse_for_package(&source, package_version)
                        } else {
                            parse("")
                        };
                        files.push(ProjectFile {
                            path: normalized,
                            source,
                            program,
                            has_parse_errors: is_dart && !errors.is_empty(),
                        });
                    }
                    Err(e) => warn!("failed to read {}: {}", path.display(), e),
                }
            }
        }
        if matches!(view, ProjectView::Semantic) {
            for (path, doc) in open_by_path {
                if !seen.contains(path) {
                    files.push(ProjectFile {
                        path: path.to_path_buf(),
                        source: doc.text.clone(),
                        program: doc.program.clone(),
                        has_parse_errors: doc.has_parse_errors,
                    });
                }
            }
        }
        files
    }
}

fn build_reverse_dependencies(
    files: &[ProjectFile],
    grouping: &LibraryGrouping,
    by_path: &HashMap<PathBuf, usize>,
    packages: &[PackageIdentity],
    build_count: &mut u64,
) -> Vec<Vec<usize>> {
    *build_count += 1;
    let mut reverse_dependencies = vec![Vec::new(); files.len()];
    for (index, file) in files.iter().enumerate() {
        for &sibling in grouping.siblings(index) {
            reverse_dependencies[sibling].push(index);
        }
        for dependency in file
            .program
            .imports
            .iter()
            .map(|directive| directive.uri.value.as_str())
            .chain(
                file.program
                    .exports
                    .iter()
                    .map(|directive| directive.uri.value.as_str()),
            )
        {
            if let Some(target) = resolve_dependency(&file.path, dependency, by_path, packages) {
                reverse_dependencies[target].push(index);
            }
        }
    }
    reverse_dependencies
}

fn affected_open_uris_in_snapshot(
    snapshot: &SemanticSnapshot,
    changed_path: &Path,
    documents: &HashMap<String, DocumentState>,
) -> Vec<String> {
    let Some(&changed_index) = snapshot.by_path.get(changed_path) else {
        return Vec::new();
    };

    let mut affected = HashSet::from([changed_index]);
    let mut pending = VecDeque::from([changed_index]);
    while let Some(index) = pending.pop_front() {
        for &dependent in &snapshot.reverse_dependencies[index] {
            if affected.insert(dependent) {
                pending.push_back(dependent);
            }
        }
    }

    let open_by_path = documents
        .iter()
        .map(|(open_uri, doc)| (&doc.path, open_uri))
        .collect::<HashMap<_, _>>();
    affected
        .into_iter()
        .filter_map(|index| open_by_path.get(&snapshot.files[index].path).copied())
        .cloned()
        .collect()
}

fn resolve_dependency(
    from: &Path,
    uri: &str,
    by_path: &HashMap<PathBuf, usize>,
    packages: &[PackageIdentity],
) -> Option<usize> {
    by_path.get(&import_path(from, uri, packages)?).copied()
}

/// The directory the cross-file pass walks: the config file's parent, else the
/// current directory (`.` if even that is unavailable).
fn workspace_root_for(config_path: Option<&Path>) -> PathBuf {
    config_path
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
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

fn document_path(uri: &str) -> (PathBuf, bool) {
    let path = uri_to_path(uri);
    match std::fs::canonicalize(&path) {
        Ok(path) => (path, false),
        Err(_) => (falcon_analyze::resolve::lexical_normalize(&path), true),
    }
}

/// Best-effort conversion of a `file://` URI to a filesystem path for
/// diagnostic attribution and semantic dependency matching.
pub(crate) fn uri_to_path(uri: &str) -> PathBuf {
    Uri::from_str(uri)
        .ok()
        .filter(|parsed| {
            parsed
                .scheme()
                .is_some_and(|scheme| scheme.as_str() == "file")
        })
        .map(|parsed| {
            PathBuf::from(
                parsed
                    .path()
                    .as_estr()
                    .decode()
                    .into_string_lossy()
                    .into_owned(),
            )
        })
        .unwrap_or_else(|| PathBuf::from(uri.strip_prefix("file://").unwrap_or(uri)))
}

#[cfg(test)]
mod dependency_tests {
    use super::*;

    #[test]
    fn package_dependency_requires_matching_owner_name() {
        let by_path = HashMap::from([(PathBuf::from("/workspace/lib/foo.dart"), 0)]);
        let packages = [PackageIdentity {
            name: "workspace".to_string(),
            lib_root: PathBuf::from("/workspace/lib"),
        }];

        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/lib/main.dart"),
                "package:other/foo.dart",
                &by_path,
                &packages,
            ),
            None,
        );
        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/lib/main.dart"),
                "package:workspace/foo.dart",
                &by_path,
                &packages,
            ),
            Some(0),
        );
        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/lib/main.dart"),
                "foo.dart",
                &by_path,
                &packages,
            ),
            Some(0),
        );
    }

    #[test]
    fn package_dependency_resolves_matching_workspace_package() {
        let by_path = HashMap::from([(PathBuf::from("/workspace/b/lib/api.dart"), 0)]);
        let packages = [
            PackageIdentity {
                name: "a".to_string(),
                lib_root: PathBuf::from("/workspace/a/lib"),
            },
            PackageIdentity {
                name: "b".to_string(),
                lib_root: PathBuf::from("/workspace/b/lib"),
            },
        ];

        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/a/lib/main.dart"),
                "package:b/api.dart",
                &by_path,
                &packages,
            ),
            Some(0),
        );

        let ambiguous = [
            packages[1].clone(),
            PackageIdentity {
                name: "b".to_string(),
                lib_root: PathBuf::from("/workspace/other-b/lib"),
            },
        ];
        assert_eq!(
            resolve_dependency(
                Path::new("/workspace/a/lib/main.dart"),
                "package:b/api.dart",
                &by_path,
                &ambiguous,
            ),
            None,
        );
    }

    #[test]
    fn unreadable_nested_pubspec_remains_an_ownership_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let outer = dir.path();
        let nested = outer.join("nested");
        std::fs::create_dir_all(nested.join("lib")).unwrap();
        std::fs::write(outer.join("pubspec.yaml"), "name: outer\n").unwrap();
        std::fs::write(nested.join("pubspec.yaml"), [0xff, 0xfe]).unwrap();
        let path = nested.join("lib/main.dart");
        let (program, errors) = parse("");
        let files = [ProjectFile {
            path: path.clone(),
            source: String::new(),
            program,
            has_parse_errors: !errors.is_empty(),
        }];

        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        let nested_lib = canonical_or_lexical(&nested.join("lib"));
        let owner = packages
            .iter()
            .find(|package| package.lib_root == nested_lib)
            .expect("unreadable nested manifest must retain its package identity");
        assert!(owner.name.is_empty());
    }

    const VERSIONED_SWITCH: &str = "void f(Object o) { switch (o) { case 1 + 2: break; } }\n";

    fn write_package_version(root: &Path, version: &str) {
        let directory = root.join(".dart_tool");
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("package_config.json");
        let modified = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        std::fs::write(
            &path,
            format!(
                r#"{{"configVersion":2,"packages":[{{"name":"fixture","rootUri":"../","packageUri":"lib/","languageVersion":"{version}"}}]}}"#
            ),
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_times(
                std::fs::FileTimes::new()
                    .set_modified(modified + std::time::Duration::from_secs(1)),
            )
            .unwrap();
    }

    fn switch_has_pattern(program: &Program) -> bool {
        use falcon_syntax::ast::{FunctionBody, Stmt, SwitchCaseKind, TopLevelDecl};
        let TopLevelDecl::Function(function) = &program.declarations[0] else {
            panic!("expected function")
        };
        let Some(FunctionBody::Block(block)) = &function.body else {
            panic!("expected function body")
        };
        let Stmt::Switch(switch) = &block.stmts[0] else {
            panic!("expected switch")
        };
        matches!(switch.cases[0].cases[0], SwitchCaseKind::Pattern(..))
    }

    #[test]
    fn package_version_reload_reparses_only_changed_effective_versions() {
        for semantic in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let config_path = dir.path().join("falcon.json");
            let config = if semantic {
                r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#
            } else {
                r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"enabled":false}}"#
            };
            std::fs::write(&config_path, config).unwrap();
            write_package_version(dir.path(), "2.19");
            let unrelated = dir.path().join("nested");
            std::fs::create_dir_all(unrelated.join("lib")).unwrap();
            write_package_version(&unrelated, "2.19");
            let uri = format!("file://{}", dir.path().join("main.dart").display());
            let marker_uri = format!("file://{}", dir.path().join("marker.dart").display());
            let unrelated_uri = format!("file://{}", unrelated.join("lib/other.dart").display());
            let mut state = LspState::new(Some(config_path));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            let initial = state.open(&uri, VERSIONED_SWITCH.to_string(), Some(1));
            state.open(
                &marker_uri,
                format!("// @dart = 2.19\n{VERSIONED_SWITCH}"),
                Some(1),
            );
            state.open(&unrelated_uri, VERSIONED_SWITCH.to_string(), Some(1));
            assert!(!initial.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(!switch_has_pattern(&state.documents[&uri].program));
            assert_eq!(state.documents[&uri].parse_count, 1);

            write_package_version(dir.path(), "3.0");
            let published = state.reload_config();
            let diagnostics = &published.iter().find(|(key, _)| key == &uri).unwrap().1;
            assert!(diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(switch_has_pattern(&state.documents[&uri].program));
            assert_eq!(state.documents[&uri].parse_count, 2);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            assert_eq!(state.documents[&unrelated_uri].parse_count, 1);
            assert!(!switch_has_pattern(&state.documents[&marker_uri].program));
            state.analyze(&uri);
            state.reload_config();
            assert_eq!(state.documents[&uri].parse_count, 2);
            write_package_version(dir.path(), "3.1");
            state.watched_files_changed(&[dir.path().join(".dart_tool/package_config.json")]);
            state.analyze(&uri);
            assert_eq!(state.documents[&uri].parse_count, 3);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            if semantic {
                let snapshot = state.semantic_snapshot.as_ref().unwrap();
                let file = &snapshot.files[snapshot.by_path[&state.documents[&uri].path]];
                assert!(switch_has_pattern(&file.program));
                assert!(file.has_parse_errors);
            }

            write_package_version(dir.path(), "2.19");
            state.watched_files_changed(&[dir.path().join(".dart_tool/package_config.json")]);
            assert!(state.change(&uri, VERSIONED_SWITCH.to_string(), Some(2)));
            let diagnostics = state.analyze(&uri);
            assert!(!diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(!switch_has_pattern(&state.documents[&uri].program));
            assert_eq!(state.documents[&uri].parse_count, 5);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            assert_eq!(state.documents[&unrelated_uri].parse_count, 1);
        }
    }

    #[test]
    fn disk_collection_discovers_nested_versions_before_parsing_and_indexing() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("falcon.json");
        std::fs::write(
            &config_path,
            r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#,
        )
        .unwrap();
        let mut paths = Vec::with_capacity(2);
        for (package, version) in [("legacy", "2.19"), ("modern", "3.0")] {
            let root = dir.path().join(package);
            std::fs::create_dir_all(root.join("lib")).unwrap();
            write_package_version(&root, version);
            let path = root.join("lib/case.dart");
            std::fs::write(&path, VERSIONED_SWITCH).unwrap();
            paths.push(canonical_or_lexical(&path));
        }
        let mut state = LspState::new(Some(config_path));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let uri = format!("file://{}", dir.path().join("main.dart").display());
        state.open(&uri, "void main() {}\n".to_string(), Some(1));
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        for (path, pattern) in paths.iter().zip([false, true]) {
            let file = &snapshot.files[snapshot.by_path[path]];
            assert_eq!(switch_has_pattern(&file.program), pattern);
            assert_eq!(file.has_parse_errors, pattern);
        }
        let files = state.collect_project_files(ProjectView::CrossFile);
        for (path, pattern) in paths.iter().zip([false, true]) {
            let file = files.iter().find(|file| &file.path == path).unwrap();
            assert_eq!(switch_has_pattern(&file.program), pattern);
            assert_eq!(file.has_parse_errors, pattern);
        }
        assert_eq!(state.documents[&uri].parse_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn materialized_path_reparses_at_new_owner_version_once() {
        for event in ["analyze", "save", "change"] {
            let dir = tempfile::tempdir().unwrap();
            let config_path = dir.path().join("falcon.json");
            std::fs::write(
                &config_path,
                r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#,
            )
            .unwrap();
            write_package_version(dir.path(), "3.0");
            let real = dir.path().join("real");
            std::fs::create_dir_all(real.join("lib")).unwrap();
            write_package_version(&real, "2.19");
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(real.join("lib"), &link).unwrap();
            let path = link.join("new.dart");
            let uri = format!("file://{}", path.display());
            let marker_path = link.join("marker.dart");
            let marker_uri = format!("file://{}", marker_path.display());
            let marker = format!("// @dart = 2.19\n{VERSIONED_SWITCH}");
            let mut state = LspState::new(Some(config_path));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            state.open(&uri, VERSIONED_SWITCH.to_string(), Some(1));
            state.open(&marker_uri, marker.clone(), Some(1));
            assert!(switch_has_pattern(&state.documents[&uri].program));
            assert!(!switch_has_pattern(&state.documents[&marker_uri].program));
            std::fs::write(&path, VERSIONED_SWITCH).unwrap();
            std::fs::write(&marker_path, &marker).unwrap();
            let diagnostics = match event {
                "save" => state.save(&uri, None),
                "change" => {
                    state.change(&uri, VERSIONED_SWITCH.to_string(), Some(2));
                    state.analyze(&uri)
                }
                _ => state.analyze(&uri),
            };
            assert!(!diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(!switch_has_pattern(&state.documents[&uri].program));
            assert_eq!(state.documents[&uri].path, canonical_or_lexical(&path));
            assert_eq!(state.documents[&uri].parse_count, 2);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            state.analyze(&uri);
            assert_eq!(state.documents[&uri].parse_count, 2);
        }
    }

    #[test]
    fn cross_file_collection_includes_pubspec_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("falcon.json");
        let manifest_path = dir.path().join("pubspec.yaml");
        std::fs::write(&config_path, "{}").unwrap();
        std::fs::write(&manifest_path, "name: workspace\n").unwrap();
        std::fs::write(dir.path().join("main.dart"), "void main() {}\n").unwrap();
        let mut state = LspState::new(Some(config_path));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);

        let files = state.collect_project_files(ProjectView::CrossFile);
        let manifest = files
            .iter()
            .find(|file| file.path == canonical_or_lexical(&manifest_path))
            .expect("pubspec.yaml must be included in the cross-file project view");
        assert_eq!(manifest.source, "name: workspace\n");
        assert!(!manifest.has_parse_errors);
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_workspace_publishes_canonical_cross_file_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(workspace.join("lib")).unwrap();
        std::fs::write(
            workspace.join("falcon.json"),
            r#"{
            "linter": { "enabled": false },
            "cross-file": { "rules": {
                "recommended": false,
                "correctness": { "unused-files": "warn" }
            } }
        }"#,
        )
        .unwrap();
        std::fs::write(workspace.join("lib/orphan.dart"), "class Orphan {}\n").unwrap();
        std::fs::write(workspace.join("lib/main.dart"), "void main() {}\n").unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&workspace, &alias).unwrap();
        let uri = format!("file://{}", alias.join("lib/orphan.dart").display());
        let mut state = LspState::new(Some(alias.join("falcon.json")));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        state.open(&uri, "class Orphan {}\n".to_string(), Some(1));

        let published = state.cross_file_pass();
        let (_, diagnostics) = published
            .iter()
            .find(|(published_uri, _)| published_uri == &uri)
            .expect("cross-file diagnostics must publish through the original symlink URI");
        let diagnostic = diagnostics
            .iter()
            .find(|diag| diag.rule == "unused-files")
            .expect("canonical project path must find the orphan diagnostic");
        assert_eq!(
            diagnostic.file_path,
            canonical_or_lexical(&workspace.join("lib/orphan.dart")).to_string_lossy()
        );
        assert_eq!(state.document(&uri).unwrap().parse_count, 1);

        std::fs::write(
            workspace.join("lib/main.dart"),
            "import 'orphan.dart';\nvoid main() { Orphan(); }\n",
        )
        .unwrap();
        let published = state.cross_file_pass();
        let (_, diagnostics) = published
            .iter()
            .find(|(published_uri, _)| published_uri == &uri)
            .expect("the same symlink URI must receive cleared diagnostics");
        assert!(!diagnostics.iter().any(|diag| diag.rule == "unused-files"));
        assert_eq!(state.document(&uri).unwrap().parse_count, 1);
    }

    #[cfg(unix)]
    #[test]
    fn untouched_materialized_buffer_keeps_one_canonical_project_entry() {
        for (resolve, parent_segment) in
            [(true, false), (false, false), (true, true), (false, true)]
        {
            let dir = tempfile::tempdir().unwrap();
            let workspace = dir.path().join("workspace");
            std::fs::create_dir_all(workspace.join("lib/deep")).unwrap();
            let config_path = workspace.join("falcon.json");
            let config = serde_json::json!({
                "linter": {"rules": {"recommended": false,
                    "suspicious": {"avoid-ignoring-return-values": if resolve { "warn" } else { "off" }}}},
                "cross-file": {"rules": {"recommended": false,
                    "correctness": {"unused-files": "warn"}}}
            });
            std::fs::write(&config_path, config.to_string()).unwrap();
            let main = workspace.join("lib/main.dart");
            std::fs::write(&main, "void main() {}\n").unwrap();
            let alias = dir.path().join("alias");
            let target = if parent_segment {
                workspace.join("lib/deep")
            } else {
                workspace.clone()
            };
            std::os::unix::fs::symlink(target, &alias).unwrap();
            let relative = if parent_segment {
                "../orphan.dart"
            } else {
                "lib/orphan.dart"
            };
            let uri = format!("file://{}", alias.join(relative).display());
            let main_uri = format!("file://{}", main.display());
            let buffer = "int api() => 1;\n";
            let mut state = LspState::new(Some(config_path));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            assert_eq!(state.resolve_semantics, resolve);
            state.open(&main_uri, "void main() {}\n".into(), Some(1));
            state.open(&uri, buffer.into(), Some(1));
            assert!(state.document(&uri).unwrap().unresolved_path);
            let path = workspace.join("lib/orphan.dart");
            std::fs::write(&path, "int diskOnly() => 2;\n").unwrap();
            state.change(&main_uri, "void main() { print(1); }\n".into(), Some(2));
            state.analyze(&main_uri);
            let canonical = canonical_or_lexical(&path);
            let doc = state.document(&uri).unwrap();
            assert!(!doc.unresolved_path);
            assert_eq!(doc.path, canonical);
            assert_eq!(doc.parse_count, 1);
            for view in [ProjectView::Semantic, ProjectView::CrossFile] {
                let files = state.collect_project_files(view);
                assert_eq!(files.len(), 2);
                let file = files.iter().find(|file| file.path == canonical).unwrap();
                assert_eq!(file.source, buffer);
            }
            let published = state.cross_file_pass();
            let (_, diagnostics) = published.iter().find(|(key, _)| key == &uri).unwrap();
            assert!(diagnostics.iter().any(|diagnostic| {
                diagnostic.rule == "unused-files"
                    && diagnostic.file_path == canonical.to_string_lossy()
            }));
        }
    }

    #[cfg(unix)]
    #[test]
    fn saving_materialized_symlink_document_refreshes_project_identity() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().join("workspace");
        std::fs::create_dir_all(workspace.join("lib")).unwrap();
        let config_path = workspace.join("falcon.json");
        std::fs::write(
            &config_path,
            r#"{
                "linter": { "rules": {
                    "recommended": false,
                    "suspicious": { "avoid-ignoring-return-values": "warn" }
                } },
                "cross-file": { "rules": {
                    "recommended": false,
                    "correctness": { "unused-files": "warn" }
                } }
            }"#,
        )
        .unwrap();
        std::fs::write(workspace.join("lib/main.dart"), "void main() {}\n").unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(&workspace, &alias).unwrap();
        let path = workspace.join("lib/orphan.dart");
        let uri = format!("file://{}", alias.join("lib/orphan.dart").display());
        let source = "int api() => 1;\n";
        let mut state = LspState::new(Some(config_path));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        state.open(&uri, source.to_string(), Some(1));
        let old_path = state.document(&uri).unwrap().path.clone();
        std::fs::write(&path, source).unwrap();
        state.save(&uri, None);

        let canonical = canonical_or_lexical(&path);
        assert_ne!(old_path, canonical);
        assert_eq!(state.document(&uri).unwrap().path, canonical);
        let files = state.collect_project_files(ProjectView::Semantic);
        assert_eq!(
            files.len(),
            2,
            "saved alias must not duplicate the disk file"
        );
        assert!(files.iter().all(|file| file.path != old_path));
        let published = state.cross_file_pass();
        let (_, diagnostics) = published
            .iter()
            .find(|(published_uri, _)| published_uri == &uri)
            .expect("saved alias must receive cross-file diagnostics");
        assert!(diagnostics.iter().any(|diag| {
            diag.rule == "unused-files" && diag.file_path == canonical.to_string_lossy()
        }));
        assert_eq!(state.document(&uri).unwrap().parse_count, 1);

        let main_uri = format!("file://{}", workspace.join("lib/main.dart").display());
        state.open(
            &main_uri,
            "import 'orphan.dart';\nvoid main() { api(); }\n".to_string(),
            Some(1),
        );
        state.change(&uri, "void api() {}\n".to_string(), Some(2));
        assert!(
            state.affected_open_uris(&uri).contains(&main_uri),
            "dependency graph must use the saved canonical identity"
        );
    }

    #[test]
    fn opening_document_flushes_debounced_dependents_before_cross_file_publish() {
        use std::time::Duration;

        use lsp_server::{Connection, Message, Notification, Request};
        use lsp_types::{NumberOrString, PublishDiagnosticsParams};
        use serde_json::json;

        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("falcon.json");
        std::fs::write(
            &config_path,
            r#"{
                "linter": { "rules": {
                    "recommended": false,
                    "suspicious": { "avoid-ignoring-return-values": "warn" }
                } },
                "cross-file": { "rules": {
                    "recommended": false,
                    "correctness": { "unused-code": "warn" }
                } }
            }"#,
        )
        .unwrap();
        let a_uri = format!("file://{}", dir.path().join("a.dart").display());
        let b_uri = format!("file://{}", dir.path().join("b.dart").display());
        let c_uri = format!("file://{}", dir.path().join("c.dart").display());
        let a_source = "void api() {}\nvoid unusedA() {}\n";
        let b_source = "import 'a.dart';\nvoid main() { api(); }\n";
        std::fs::write(dir.path().join("a.dart"), a_source).unwrap();
        std::fs::write(dir.path().join("b.dart"), b_source).unwrap();
        let (server, client) = Connection::memory();
        let worker = std::thread::spawn(move || {
            crate::server::run_with_connection(
                server,
                crate::server::ServerOptions {
                    debounce: Duration::from_secs(60),
                    config_path: Some(config_path),
                },
            )
            .unwrap();
        });
        let receive_through = |id: i32| {
            let mut published = Vec::new();
            loop {
                match client
                    .receiver
                    .recv_timeout(Duration::from_secs(5))
                    .unwrap()
                {
                    Message::Response(response) => {
                        assert_eq!(response.id, id.into());
                        break;
                    }
                    Message::Notification(notification) => {
                        assert_eq!(notification.method, "textDocument/publishDiagnostics");
                        published.push(
                            serde_json::from_value::<PublishDiagnosticsParams>(notification.params)
                                .unwrap(),
                        );
                    }
                    other => panic!("unexpected server message: {other:?}"),
                }
            }
            published
        };
        let notify = |method: &str, params| {
            client
                .sender
                .send(Notification::new(method.to_string(), params).into())
                .unwrap();
        };
        let barrier = |id: i32| {
            client
                .sender
                .send(Request::new(id.into(), "falcon/barrier".to_string(), ()).into())
                .unwrap();
            receive_through(id)
        };
        client
            .sender
            .send(
                Request::new(
                    0.into(),
                    "initialize".to_string(),
                    json!({"capabilities": {}}),
                )
                .into(),
            )
            .unwrap();
        receive_through(0);
        notify("initialized", json!({}));
        for (uri, text) in [(&a_uri, a_source), (&b_uri, b_source)] {
            notify(
                "textDocument/didOpen",
                json!({"textDocument": {
                    "uri": uri, "languageId": "dart", "version": 1, "text": text
                }}),
            );
        }
        let code = Some(NumberOrString::String(
            "avoid-ignoring-return-values".to_string(),
        ));
        let initial = barrier(1);
        let initial_b = initial
            .iter()
            .rev()
            .find(|params| params.uri.as_str() == b_uri)
            .expect("B must publish its initial diagnostics");
        assert!(
            initial_b
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != code)
        );
        notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": a_uri, "version": 2},
                "contentChanges": [{"text": "int api() => 1;\nvoid unusedA() {}\n"}]
            }),
        );
        notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": c_uri, "languageId": "dart", "version": 1, "text": "void c() {}\n"
            }}),
        );
        let published = barrier(2);
        let refreshed = published
            .iter()
            .rev()
            .find(|params| params.uri.as_str() == b_uri)
            .expect("opening C must publish B's pending semantic refresh");
        assert!(
            refreshed
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code)
        );
        client
            .sender
            .send(Request::new(3.into(), "shutdown".to_string(), ()).into())
            .unwrap();
        receive_through(3);
        notify("exit", json!({}));
        worker.join().unwrap();
    }

    #[test]
    fn file_uri_decodes_percent_encoded_path() {
        assert_eq!(
            uri_to_path("file:///workspace/with%20space/main.dart"),
            PathBuf::from("/workspace/with space/main.dart")
        );
    }
}

#[cfg(test)]
mod environment_regressions {
    use super::*;

    fn state(root: &Path) -> LspState {
        let config = root.join("falcon.json");
        std::fs::write(&config,
            r#"{"linter":{"rules":{"recommended":false,"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#).unwrap();
        let mut state = LspState::new(Some(config));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        state
    }

    #[test]
    fn configuration_events_invalidate_even_after_discovery_recaptures_new_bytes() {
        let (dir, mut state, uri, _dependency) = dependency_fixture(false);
        let app = dir.path().join("app");
        let config = app.join(".dart_tool/package_config.json");
        std::fs::create_dir_all(dir.path().join("new/lib")).unwrap();
        std::fs::write(dir.path().join("new/lib/api.dart"), "class Replacement {}").unwrap();
        std::fs::write(&config,
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../new","packageUri":"lib/"}]}"#).unwrap();
        std::fs::create_dir_all(app.join("new_directory")).unwrap();
        let new_uri = format!("file://{}", app.join("new_directory/file.dart").display());
        state.open(&new_uri, "void other() {}".into(), Some(1));
        state.analyze(&new_uri);
        assert_eq!(
            state.environment_stamps.as_ref(),
            Some(&state.environment.config_stamps())
        );
        assert!(has_external_class(&state, "Original"));
        let resets = state.external_reset_count;
        state.watched_files_changed(&[config]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "Replacement"));
        assert!(!has_external_class(&state, "Original"));
        state.analyze(&uri);
        std::fs::rename(app.join(".dart_tool"), app.join("removed-dart-tool")).unwrap();
        state.watched_files_changed(&[app.join(".dart_tool")]);
        assert!(!has_external_class(&state, "Replacement"));
    }

    #[test]
    fn nested_project_sources_retain_cache_but_excluded_dependencies_invalidate() {
        let (dir, mut state, uri, _dependency) = dependency_fixture(false);
        let app = dir.path().join("app");
        let core = app.join("packages/core/lib/api.dart");
        std::fs::create_dir_all(core.parent().unwrap()).unwrap();
        std::fs::write(&core, "void coreApi() {} class NestedBefore {}").unwrap();
        let config = app.join(".dart_tool/package_config.json");
        std::fs::write(&config,
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"},{"name":"core","rootUri":"../packages/core","packageUri":"lib/"}]}"#).unwrap();
        state.watched_files_changed(&[config]);
        state.change(&uri,
            "import 'package:dep/api.dart'; import 'package:core/api.dart'; void main() { coreApi(); }".into(), Some(2));
        assert!(
            !state
                .analyze(&uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        );
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        let resets = state.external_reset_count;
        assert!(state.change(&uri,
            "import 'package:dep/api.dart'; import 'package:core/api.dart'; void main() { coreApi(); } // pending".into(), Some(3)));
        assert!(state.semantic_snapshot.is_none());
        std::fs::write(&core, "int coreApi() => 1; class NestedBefore {}").unwrap();
        let published = state.watched_files_changed(std::slice::from_ref(&core));
        assert!(published.iter().any(|(published_uri, diagnostics)| {
            published_uri == &uri
                && diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        }));
        assert_eq!(state.external_reset_count, resets);
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        assert!(state.change(&uri,
            "import 'package:dep/api.dart'; import 'package:core/api.dart'; void main() { coreApi(); } // next pending".into(), Some(4)));
        assert!(state.semantic_snapshot.is_none());
        let generated = core.with_file_name("new.g.dart");
        std::fs::write(&generated, "class NewlyGenerated {}").unwrap();
        state.watched_files_changed(&[generated]);
        assert_eq!(state.external_reset_count, resets);
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        let core_uri = format!("file://{}", core.display());
        state.open(
            &core_uri,
            "int coreApi() => 2; class Buffered {}".into(),
            Some(1),
        );
        state.analyze(&core_uri);
        assert!(
            state
                .watched_files_changed(std::slice::from_ref(&core))
                .is_empty()
        );
        assert_eq!(state.external_reset_count, resets);
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        state.close(&core_uri);
        state
            .config
            .files
            .includes
            .extend_excludes(&[format!("{}/**", app.join("packages/core").display())]);
        state.invalidate_semantic_snapshot();
        state.analyze(&uri);
        assert!(has_external_class(&state, "NestedBefore"));
        std::fs::write(&core, "void coreApi() {} class NestedAfter {}").unwrap();
        let resets = state.external_reset_count;
        state.watched_files_changed(&[core]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "NestedAfter"));
        assert!(!has_external_class(&state, "NestedBefore"));
    }

    fn dependency_fixture(hosted: bool) -> (tempfile::TempDir, LspState, String, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        let dependency = dir.path().join("dep/lib/api.dart");
        std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
        std::fs::write(&dependency, "class Original {}").unwrap();
        std::fs::write(app.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#).unwrap();
        if hosted {
            std::fs::write(
                app.join("pubspec.lock"),
                "packages:\n  dep:\n    source: hosted\n    version: '1.0.0'\n",
            )
            .unwrap();
        }
        let mut state = state(&app);
        let uri = format!("file://{}", app.join("main.dart").display());
        state.open(
            &uri,
            "import 'package:dep/api.dart'; void main() {}".into(),
            Some(1),
        );
        state.ensure_semantic_snapshot();
        (dir, state, uri, dependency)
    }

    fn has_external_class(state: &LspState, name: &str) -> bool {
        state.external_libraries.libraries().iter().flat_map(|library| &library.files)
            .flat_map(|file| &file.facts.declarations).any(|declaration| {
                matches!(declaration, falcon_syntax::ast::TopLevelDecl::Class(class) if class.name.name == name)
            })
    }

    #[test]
    fn project_edits_and_save_events_reuse_all_external_arcs_without_sdk() {
        let (_dir, mut state, uri, dependency) = dependency_fixture(false);
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let builds = state.external_index_build_count;
        assert!(!state.environment.has_sdk());
        assert!(state.change(
            &uri,
            "import 'package:dep/api.dart'; void main() { final value = Original(); }".into(),
            Some(2)
        ));
        state.analyze(&uri);
        let project = state.documents[&uri].path.clone();
        assert!(state.watched_files_changed(&[project]).is_empty());
        let cache = state.external_indexes.as_ref().unwrap();
        assert!(Arc::ptr_eq(&identities, &cache.identities));
        assert!(Arc::ptr_eq(&types, &cache.types));
        assert!(Arc::ptr_eq(&signatures, &cache.signatures));
        assert_eq!(state.external_index_build_count, builds);
        let resets = state.external_reset_count;
        assert!(
            state
                .watched_files_changed(&[dependency
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .parent()
                    .unwrap()
                    .join("irrelevant.dart")])
                .is_empty()
        );
        assert_eq!(state.external_reset_count, resets);
        std::fs::write(&dependency, "class Replaced {}").unwrap();
        state.watched_files_changed(&[dependency]);
        assert!(has_external_class(&state, "Replaced"));
        assert!(!has_external_class(&state, "Original"));
        assert!(!Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
    }

    #[test]
    fn closed_project_events_refresh_diagnostics_without_rebuilding_external_indexes() {
        let (dir, mut state, uri, _dependency) = dependency_fixture(false);
        let closed = dir.path().join("app/api.dart");
        std::fs::write(&closed, "void api() {}").unwrap();
        state.change(
            &uri,
            "import 'api.dart'; import 'package:dep/api.dart'; void main() { api(); }".into(),
            Some(2),
        );
        assert!(
            !state
                .analyze(&uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        );
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        let builds = state.external_index_build_count;
        std::fs::write(&closed, "int api() => 1;").unwrap();
        let published = state.watched_files_changed(&[closed]);
        assert!(published.iter().any(|(published_uri, diagnostics)| {
            published_uri == &uri
                && diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
        }));
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        assert_eq!(state.external_index_build_count, builds);
    }

    #[test]
    fn immutable_hosted_sources_wait_for_resolution_input_events() {
        let (dir, mut state, uri, dependency) = dependency_fixture(true);
        assert!(state.watched_dependency_roots().is_empty());
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        std::fs::write(&dependency, "class Modified {}").unwrap();
        state.change(
            &uri,
            "import 'package:dep/api.dart'; void main() { }".into(),
            Some(2),
        );
        state.analyze(&uri);
        assert!(state.watched_files_changed(&[dependency]).is_empty());
        assert!(has_external_class(&state, "Original"));
        assert!(!has_external_class(&state, "Modified"));
        assert!(Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        let lock = dir.path().join("app/pubspec.lock");
        let metadata = std::fs::metadata(&lock).unwrap();
        std::fs::write(
            &lock,
            "packages:\n  dep:\n    source: hosted\n    version: '2.0.0'\n",
        )
        .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&lock)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        state.watched_files_changed(&[lock]);
        assert!(has_external_class(&state, "Modified"));
        assert!(!Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
    }

    #[test]
    fn lock_source_and_git_reference_events_refresh_mutable_watch_roots() {
        let (dir, mut state, _uri, dependency) = dependency_fixture(true);
        let lock = dir.path().join("app/pubspec.lock");
        std::fs::write(
            &lock,
            "packages:\n  dep:\n    source: git\n    description:\n      resolved-ref: first\n",
        )
        .unwrap();
        state.watched_files_changed(std::slice::from_ref(&lock));
        assert!(
            state
                .watched_dependency_roots()
                .iter()
                .any(|root| dependency.starts_with(root))
        );
        let identities = Arc::clone(&state.external_indexes.as_ref().unwrap().identities);
        std::fs::write(&dependency, "class NewReference {}").unwrap();
        std::fs::write(
            &lock,
            "packages:\n  dep:\n    source: git\n    description:\n      resolved-ref: other\n",
        )
        .unwrap();
        state.watched_files_changed(&[lock]);
        assert!(has_external_class(&state, "NewReference"));
        assert!(!Arc::ptr_eq(
            &identities,
            &state.external_indexes.as_ref().unwrap().identities
        ));
        let overrides = dir.path().join("app/pubspec_overrides.yaml");
        std::fs::write(
            &overrides,
            "dependency_overrides:\n  dep:\n    path: ../dep\n",
        )
        .unwrap();
        let resets = state.external_reset_count;
        state.watched_files_changed(&[overrides]);
        assert_eq!(state.external_reset_count, resets + 1);
    }

    #[test]
    fn mutable_events_reload_fixed_timestamp_sources_missing_files_and_replaced_roots() {
        let (dir, mut state, _uri, dependency) = dependency_fixture(false);
        let metadata = std::fs::metadata(&dependency).unwrap();
        std::fs::write(&dependency, "class Replaced {}").unwrap();
        assert_eq!(
            std::fs::metadata(&dependency).unwrap().len(),
            metadata.len()
        );
        std::fs::File::options()
            .write(true)
            .open(&dependency)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        assert!(has_external_class(&state, "Replaced"));
        std::fs::remove_file(&dependency).unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        assert!(!has_external_class(&state, "Replaced"));
        std::fs::write(
            &dependency,
            "library dep; part 'generated.dart'; class Restored {}",
        )
        .unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        assert!(has_external_class(&state, "Restored"));
        let part = dependency.with_file_name("generated.dart");
        std::fs::write(&part, "part of dep; class Generated {}").unwrap();
        state.watched_files_changed(&[part]);
        assert!(has_external_class(&state, "Generated"));
        let root = dir.path().join("dep");
        std::fs::rename(&root, dir.path().join("old-dep")).unwrap();
        std::fs::create_dir_all(root.join("lib")).unwrap();
        std::fs::write(&dependency, "class NewRoot {}").unwrap();
        let resets = state.external_reset_count;
        state.watched_files_changed(&[root, dependency]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "NewRoot"));
        assert!(!has_external_class(&state, "Generated"));
        assert!(!has_external_class(&state, "Restored"));
    }

    #[test]
    fn doc_import_backedges_retain_topology_without_external_index_cache() {
        let (dir, mut state, uri, dependency) = dependency_fixture(false);
        std::fs::write(
            &dependency,
            "/// @docImport '../../app/main.dart';\nlibrary;\nclass Original {}",
        )
        .unwrap();
        state.watched_files_changed(&[dependency]);
        assert!(state.external_indexes.is_none());
        let paths = state.external_topology.as_ref().unwrap().paths.as_ptr();
        let target = canonical_or_lexical(&dir.path().join("app/main.dart"));
        assert!(
            state
                .external_topology
                .as_ref()
                .unwrap()
                .project_dependency_targets
                .contains(&target)
        );
        state.change(
            &uri,
            "import 'package:dep/api.dart'; class Editable {}".into(),
            Some(2),
        );
        state.analyze(&uri);
        assert!(state.external_indexes.is_none());
        assert_eq!(
            paths,
            state.external_topology.as_ref().unwrap().paths.as_ptr()
        );
    }

    #[test]
    fn first_package_configuration_refreshes_open_buffer_grammar() {
        for semantic in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let mut state = state(dir.path());
            state.resolve_semantics = semantic;
            let path = dir.path().join("main.dart");
            let uri = format!("file://{}", path.display());
            let source = "void f(Object o) { switch (o) { case 1 + 2: break; } }";
            state.open(&uri, source.into(), Some(1));
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::CURRENT
            );
            std::fs::create_dir_all(dir.path().join(".dart_tool")).unwrap();
            std::fs::write(dir.path().join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"","languageVersion":"2.19"}]}"#).unwrap();
            state.watched_files_changed(&[dir.path().join(".dart_tool/package_config.json")]);
            state.analyze(&uri);
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::new(2, 19)
            );
            assert_eq!(state.documents[&uri].parse_count, 2);
            assert!(!state.environment.has_sdk());
        }
    }

    #[test]
    fn doc_import_dependencies_refresh_and_yield_to_open_buffers_without_sdk() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        let dependency = dir.path().join("dep/lib/api.dart");
        std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
        std::fs::write(&dependency, "class Original {}").unwrap();
        std::fs::write(app.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#).unwrap();
        let mut state = state(&app);
        let uri = format!("file://{}", app.join("main.dart").display());
        state.open(
            &uri,
            "/// @docImport 'package:dep/api.dart';\nlibrary;\nvoid main() {}".into(),
            Some(1),
        );
        state.ensure_semantic_snapshot();
        assert_eq!(state.external_libraries.libraries().len(), 1);
        let resets = state.external_reset_count;
        std::fs::write(&dependency, "class ChangedDependency {}").unwrap();
        state.watched_files_changed(std::slice::from_ref(&dependency));
        state.ensure_semantic_snapshot();
        assert_eq!(state.external_reset_count, resets + 1);
        let file = &state.external_libraries.libraries()[0].files[0];
        let falcon_syntax::ast::TopLevelDecl::Class(class) = &file.facts.declarations[0] else {
            panic!("expected dependency class");
        };
        assert_eq!(class.name.name, "ChangedDependency");
        let dependency_uri = format!("file://{}", dependency.display());
        state.open(
            &dependency_uri,
            "class UnsavedDependency {}".into(),
            Some(1),
        );
        state.ensure_semantic_snapshot();
        assert!(state.external_libraries.libraries().is_empty());
        assert!(!state.environment.has_sdk());
    }

    #[test]
    fn opening_dependency_parts_preserves_defining_files_and_buffered_parts() {
        for warm in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let app = dir.path().join("app");
            let dep = dir.path().join("dep/lib");
            std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(&dep).unwrap();
            std::fs::write(app.join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":""},{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#).unwrap();
            std::fs::write(dep.join("bridge.dart"), "export 'leaf.dart';").unwrap();
            let defining = dep.join("leaf.dart");
            let part = dep.join("leaf_part.dart");
            std::fs::write(
                &defining,
                "library leaf; part 'leaf_part.dart'; class Leaf {}",
            )
            .unwrap();
            std::fs::write(&part, "part of leaf; class OnDisk {}").unwrap();
            let mut state = state(&app);
            let app_uri = format!("file://{}", app.join("main.dart").display());
            let part_uri = format!("file://{}", part.display());
            let buffered = "part of leaf; class Unsaved {}";
            if !warm {
                state.open(&part_uri, buffered.into(), Some(1));
            }
            state.open(
                &app_uri,
                "import 'package:dep/bridge.dart';".into(),
                Some(1),
            );
            if warm {
                state.ensure_semantic_snapshot();
                state.open(&part_uri, buffered.into(), Some(1));
            }
            state.ensure_semantic_snapshot();
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            assert!(snapshot.files.iter().any(|file| file.path == defining));
            let parts: Vec<_> = snapshot
                .files
                .iter()
                .filter(|file| file.path == part)
                .collect();
            assert_eq!(parts.len(), 1);
            assert_eq!(parts[0].source, buffered);
            assert!(state.external_libraries.libraries().iter().all(|library| {
                library
                    .files
                    .iter()
                    .all(|file| file.path != defining && file.path != part)
            }));
            state.close(&part_uri);
            state.ensure_semantic_snapshot();
            let leaf = state
                .external_libraries
                .libraries()
                .iter()
                .find(|library| library.files[0].path == defining)
                .unwrap();
            assert_eq!(leaf.files.len(), 2);
            assert_eq!(leaf.files[1].path, part);
        }
    }
}
