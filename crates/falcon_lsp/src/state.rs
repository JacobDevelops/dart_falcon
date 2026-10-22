//! LSP server state: document cache, cached syntax trees, config-driven rule set.
//!
//! Text changes invalidate a document's tree; config changes rerun rules over
//! cached trees, reparsing only when the effective language version changes.
//! Unresolved filesystem identities are retried before analysis when files materialize.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use lsp_types::{MessageType, ShowMessageParams, Uri};

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, CrossFileRuleRegistry, ExternalLibraries, ExternalOrigin,
    ExternalSources, IdentityIndex, IdentitySource, LibraryGrouping, LibrarySource,
    PackageIdentity, ParsedFile, ProjectFile, RuleRegistry, SemanticIndexes, SignatureIndex,
    TypeIndex, canonical_or_lexical, environment::ExternalTarget, group_libraries, import_path,
    library_unit, package_identities, syntax_error_diagnostics,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::{lex_tree, parse_for_package};
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    apply_severities, build_cross_file_registry, build_registry, cross_file_requires_resolution,
    load_config, load_or_default, requires_resolution, run_cross_file,
};
use falcon_syntax::{LanguageVersion, cst::Program};
use tracing::{debug, warn};
use walkdir::WalkDir;

/// One open document: full text, cached syntax tree, and instrumentation counters.
pub struct DocumentState {
    parsed: Arc<ParsedFile>,
    path: PathBuf,
    /// Only unresolved paths need filesystem checks between document events.
    unresolved_path: bool,
    pub version: Option<i32>,
    language_version: LanguageVersion,
    /// Whether the last parse produced errors — carried so the cross-file pass
    /// can populate [`ProjectFile::has_parse_errors`] without re-parsing.
    has_parse_errors: bool,
    /// Rejected buffers keep no text or syntax; only their size diagnostic survives.
    source_rejected: bool,
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

impl DocumentState {
    pub fn text(&self) -> &str {
        &self.parsed.source
    }
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

/// Reachability and instance contexts survive layering fallback to invalidate cached identities.
struct ExternalTopology {
    library_count: usize,
    paths: Vec<(PathBuf, Option<usize>, Option<usize>)>,
    project_dependency_targets: HashSet<PathBuf>,
}

impl ExternalTopology {
    fn matches(&self, external: &ExternalLibraries) -> bool {
        self.library_count == external.libraries().len()
            && self
                .paths
                .iter()
                .map(|(path, context, copy_context)| (path.as_path(), *context, *copy_context))
                .eq(external.libraries().iter().flat_map(|library| {
                    library.files.iter().map(move |file| {
                        (file.path.as_path(), library.context, library.copy_context)
                    })
                }))
    }

    /// Resolve immutable graph edges once, including each instance's project back-edges.
    fn build(external: &ExternalLibraries, environment: &AnalysisEnvironment) -> Self {
        let capacity = external
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .map(|file| 1 + file.doc_imports.len())
            .sum();
        let mut targets = HashSet::with_capacity(capacity);
        for (instance, library) in external.libraries().iter().enumerate() {
            for file in library.files.iter() {
                if !targets.contains(&file.path) {
                    targets.insert(file.path.clone());
                }
                for uri in file
                    .program()
                    .imports()
                    .map(|import| import.uri().value())
                    .chain(file.program().exports().map(|export| export.uri().value()))
                    .chain(
                        file.doc_imports
                            .iter()
                            .map(|uri| std::borrow::Cow::Borrowed(uri.as_str())),
                    )
                {
                    if let Some(ExternalTarget::Path(path)) =
                        external.resolve_target(environment, &file.path, Some(instance), &uri)
                    {
                        targets.insert(path);
                    }
                }
            }
        }
        Self {
            library_count: external.libraries().len(),
            paths: external
                .libraries()
                .iter()
                .flat_map(|library| {
                    library
                        .files
                        .iter()
                        .map(move |file| (file.path.clone(), library.context, library.copy_context))
                })
                .collect(),
            project_dependency_targets: targets,
        }
    }
}

/// External-only facts are shared by successive project overlays.
struct ExternalIndexCache {
    identities: Arc<IdentityIndex>,
    types: Arc<TypeIndex>,
    signatures: Arc<SignatureIndex>,
}

impl ExternalIndexCache {
    fn build(external: ExternalSources<'_>) -> Self {
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
        Self {
            identities,
            types,
            signatures,
        }
    }
}

/// Body edits rebuild only the project overlay, not the SDK/package graph.
struct CrossFileExternalCache {
    roots: Vec<(PathBuf, Vec<String>)>,
    libraries: ExternalLibraries,
    indexes: Option<ExternalIndexCache>,
    watched_paths: HashSet<PathBuf>,
}

impl CrossFileExternalCache {
    /// Whether `path` is a watched source this cache loaded or tried to load.
    fn loaded(&self, path: &Path) -> bool {
        self.watched_paths.contains(path)
    }
}

/// Resolver facts for the exact Dart sources admitted by the cross-file view.
struct CrossFileIndexCache {
    files: Vec<ProjectFile>,
    packages: Vec<PackageIdentity>,
    indexes: SemanticIndexes,
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
    pending_config_messages: Vec<ShowMessageParams>,
    last_config_messages: Vec<(MessageType, String)>,
    config_start_dir: PathBuf,
    rules: RuleRegistry,
    /// Cross-file rules; empty unless enabled by config. When empty the
    /// cross-file pass is skipped entirely (the whole-workspace walk is expensive).
    cross_file_rules: CrossFileRuleRegistry,
    cross_file_resolve_semantics: bool,
    cross_file_index_cache: Option<CrossFileIndexCache>,
    cross_file_index_build_count: u64,
    cross_file_external_cache: Option<CrossFileExternalCache>,
    cross_file_external_load_count: u64,
    /// Root under which the cross-file pass walks `.dart` files: the config's
    /// directory, falling back to the current directory.
    workspace_root: PathBuf,
    /// Whether any enabled per-file or cross-file rule consumes resolver-backed semantic facts.
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
    watched_local_paths: Vec<PathBuf>,
    watched_local_sources: Vec<PathBuf>,
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
        let start_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new_from(config_path, locator, start_dir)
    }

    fn new_from(
        config_path: Option<PathBuf>,
        locator: fn(
            Option<&falcon_analyze::environment::PackageConfig>,
        ) -> Option<falcon_analyze::environment::DartSdk>,
        config_start_dir: PathBuf,
    ) -> Self {
        let (config, workspace_root, pending_config_messages) =
            load_from(config_path.as_deref(), &config_start_dir);
        let config = config.unwrap_or_else(|| {
            let mut config = FalconConfig::default();
            config.resolve_paths(&workspace_root);
            config
        });
        let last_config_messages = pending_config_messages
            .iter()
            .map(|message| (message.typ, message.message.clone()))
            .collect();
        let rules = build_registry(&config);
        let cross_file_rules = build_cross_file_registry(&config);
        let cross_file_resolve_semantics = cross_file_requires_resolution(&config);
        let resolve_semantics = requires_resolution(&rules) || cross_file_resolve_semantics;
        Self {
            documents: HashMap::new(),
            config,
            config_path,
            pending_config_messages,
            last_config_messages,
            config_start_dir,
            rules,
            cross_file_rules,
            cross_file_resolve_semantics,
            cross_file_index_cache: None,
            cross_file_index_build_count: 0,
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
            watched_local_paths: Vec::new(),
            watched_local_sources: Vec::new(),
            watched_dependency_generation: 0,
            external_index_build_count: 0,
            cross_file_external_cache: None,
            cross_file_external_load_count: 0,
        }
    }

    /// Configuration warnings and errors awaiting delivery to the editor.
    pub fn take_config_messages(&mut self) -> Vec<ShowMessageParams> {
        std::mem::take(&mut self.pending_config_messages)
    }

    /// Mutable package roots for native LSP watching. Versioned hosted packages
    /// and SDK sources are intentionally absent.
    pub fn watched_dependency_roots(&self) -> Vec<PathBuf> {
        self.environment
            .watched_dependency_roots()
            .map(Path::to_path_buf)
            .collect()
    }

    /// Loaded local files use exact relative patterns, never recursive parent watches.
    pub fn watched_dependency_files(&self) -> &[PathBuf] {
        &self.watched_local_paths
    }

    fn loaded_project_paths(&self) -> impl Iterator<Item = &Path> {
        self.external_libraries
            .libraries()
            .iter()
            .filter(|library| library.origin == ExternalOrigin::Project)
            .flat_map(|library| library.files.iter())
            .map(|file| file.path.as_path())
            .chain(self.external_libraries.project_companions())
            .chain(self.external_libraries.attempted_project_paths())
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
        let paths: Vec<_> = paths.iter().map(|path| watched_path(path)).collect();
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
            .is_some_and(|key| paths.iter().any(|path| key.affected_by(path)))
            || paths
                .iter()
                .any(|path| self.environment.package_inputs_affected_by(path));
        let open = |path: &PathBuf| {
            self.documents
                .values()
                .any(|document| document.path == *path)
        };
        let dependency_changed = paths.iter().any(|path| {
            !open(path)
                && (self.watched_local_paths.binary_search(path).is_ok()
                    || self
                        .cross_file_external_cache
                        .as_ref()
                        .is_some_and(|cache| cache.loaded(path))
                    || (self
                        .environment
                        .watched_dependency_roots()
                        .any(|root| path.starts_with(root))
                        && !self
                            .semantic_snapshot
                            .as_ref()
                            .is_some_and(|snapshot| snapshot.by_path.contains_key(path))
                        && !self.watched_project_source(path)))
        });
        let project_changed = paths
            .iter()
            .any(|path| path.starts_with(&self.workspace_root) && !open(path));
        if !inputs_changed && !dependency_changed && !project_changed && !rules_changed {
            return Vec::new();
        }
        if inputs_changed || dependency_changed {
            self.reset_external_environment();
        }
        self.pending_semantic_affected.clear();
        self.cross_file_index_cache = None;
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
        self.cross_file_index_cache = None;
        self.cross_file_external_cache = None;
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
            self.cross_file_index_cache = None;
            self.cross_file_external_cache = None;
        }
    }

    /// Keep cached grammar in sync with package ownership and marker overrides.
    fn refresh_document_versions(&mut self) {
        let mut changed = false;
        for doc in self.documents.values_mut() {
            if doc.source_rejected {
                continue;
            }
            let language_version = self.environment.language_version(&doc.path, doc.text());
            if language_version == doc.language_version {
                continue;
            }
            let falcon_dart_parser::ParseResult { tree, errors } =
                parse_for_package(doc.text(), Some(language_version));
            if let Some(parsed) = Arc::get_mut(&mut doc.parsed) {
                parsed.tree = tree;
            } else {
                doc.parsed = Arc::new(ParsedFile {
                    source: doc.text().into(),
                    tree,
                });
            }
            doc.has_parse_errors = !errors.is_empty();
            doc.syntax_diagnostics = syntax_error_diagnostics(&doc.path, &errors);
            doc.language_version = language_version;
            doc.parse_count += 1;
            changed = true;
        }
        if changed {
            self.cross_file_index_cache = None;
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
    pub fn open(&mut self, uri: &str, mut text: String, version: Option<i32>) -> Vec<Diagnostic> {
        let (path, unresolved_path) = document_path(uri, &self.workspace_root);
        let source_error = reject_oversized_buffer(&path, &mut text);
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
        let falcon_dart_parser::ParseResult {
            tree,
            errors: parse_errors,
        } = parse_for_package(&text, package_version);
        let language_version = self.environment.language_version(&path, &text);
        let mut syntax_diagnostics = syntax_error_diagnostics(&path, &parse_errors);
        let source_rejected = source_error.is_some();
        syntax_diagnostics.extend(source_error);
        self.documents.insert(
            uri.to_string(),
            DocumentState {
                path,
                unresolved_path,
                parsed: Arc::new(ParsedFile {
                    source: text.into_boxed_str(),
                    tree,
                }),
                version,
                language_version,
                has_parse_errors: source_rejected || !parse_errors.is_empty(),
                source_rejected,
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
    pub fn change(&mut self, uri: &str, mut text: String, version: Option<i32>) -> bool {
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
        let source_error = reject_oversized_buffer(&path, &mut text);
        self.refresh_environment(std::iter::once(path.as_path()));
        let package_version = self.environment.package_language_version(&path);
        let doc = self.documents.get_mut(uri).expect("document checked above");
        let falcon_dart_parser::ParseResult {
            tree,
            errors: parse_errors,
        } = parse_for_package(&text, package_version);
        doc.syntax_diagnostics = syntax_error_diagnostics(&doc.path, &parse_errors);
        doc.source_rejected = source_error.is_some();
        doc.syntax_diagnostics.extend(source_error);
        doc.parsed = Arc::new(ParsedFile {
            source: text.into_boxed_str(),
            tree,
        });
        doc.version = version;
        doc.language_version = self.environment.language_version(&doc.path, doc.text());
        doc.has_parse_errors = doc.source_rejected || !parse_errors.is_empty();
        doc.parse_count += 1;
        self.invalidate_semantic_snapshot();
        true
    }

    /// `textDocument/didSave`: refresh filesystem identity when resolvable and
    /// any supplied text (re-parsing only when it actually differs), then analyze.
    pub fn save(&mut self, uri: &str, text: Option<String>) -> Vec<Diagnostic> {
        if let Some(doc) = self.documents.get(uri)
            && let Ok(path) = std::fs::canonicalize(uri_to_path(uri))
        {
            if path != doc.path {
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
                .unresolved_path = false;
        }
        if let Some(text) = text {
            let differs = self
                .documents
                .get(uri)
                .is_some_and(|doc| doc.text() != text);
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

    /// Run the enabled rules over the cached CST of `uri`. Diagnostics are
    /// sorted by span for deterministic publishing.
    pub fn analyze(&mut self, uri: &str) -> Vec<Diagnostic> {
        if !self.documents.contains_key(uri) {
            return Vec::new();
        }
        if let Some(doc) = self
            .documents
            .get_mut(uri)
            .filter(|doc| doc.source_rejected)
        {
            doc.analyze_count += 1;
            doc.last_diagnostics = doc.syntax_diagnostics.clone();
            return doc.last_diagnostics.clone();
        }
        if self.resolve_semantics {
            self.ensure_semantic_snapshot();
        } else {
            self.refresh_unresolved_paths();
        }
        if !self.admits_project_dart(&self.documents[uri].path) {
            let doc = self.documents.get_mut(uri).expect("open document");
            doc.analyze_count += 1;
            doc.last_diagnostics.clear();
            return Vec::new();
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
        let mut ctx = AnalyzeContext::new(&doc.path, &self.config, &doc.parsed)
            .with_environment(Some(&self.environment));
        let parsed;
        let library;
        if let Some((snapshot, file_index)) = semantic {
            parsed = snapshot
                .files
                .iter()
                .map(|file| &file.parsed)
                .collect::<Vec<_>>();
            library = library_unit(&snapshot.grouping, &parsed, file_index);
            ctx = ctx
                .with_types(&snapshot.indexes.types)
                .with_identities(&snapshot.indexes.identities)
                .with_signatures(&snapshot.indexes.signatures)
                .with_library(&library);
        }
        let mut diagnostics = self.rules.run_all(doc.parsed.program(), &ctx);
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
            || document_path(uri, &self.workspace_root).0,
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
    /// against its cached CST (no re-parse). Returns per-document results
    /// for the caller to publish.
    pub fn reload_config(&mut self) -> Vec<(String, Vec<Diagnostic>)> {
        let (config, workspace_root, messages) =
            load_from(self.config_path.as_deref(), &self.config_start_dir);
        let fingerprints: Vec<_> = messages
            .iter()
            .map(|message| (message.typ, message.message.clone()))
            .collect();
        if fingerprints != self.last_config_messages {
            self.pending_config_messages.extend(messages);
            self.last_config_messages = fingerprints;
        }
        let Some(config) = config else {
            return Vec::new();
        };
        self.workspace_root = workspace_root;
        self.config = config;
        self.rules = build_registry(&self.config);
        self.cross_file_resolve_semantics = cross_file_requires_resolution(&self.config);
        self.resolve_semantics =
            requires_resolution(&self.rules) || self.cross_file_resolve_semantics;
        self.cross_file_rules = build_cross_file_registry(&self.config);
        self.cross_file_index_cache = None;
        self.cross_file_external_cache = None;
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
    /// path. Open buffers contribute their in-memory text and cached CST; every
    /// other `.dart` file under the workspace root is read and parsed from disk.
    fn cross_file_diagnostics(&mut self) -> HashMap<String, Vec<Diagnostic>> {
        // Rejected references are unknown, not absent: a partial workspace must
        // not produce unused-file/declaration diagnostics for otherwise valid files.
        if self.cross_file_rules.is_empty()
            || self.documents.values().any(|doc| doc.source_rejected)
        {
            return HashMap::new();
        }
        let (files, complete) = self.collect_project_files(ProjectView::CrossFile);
        if !complete {
            return HashMap::new();
        }
        // Semantic views include unsaved buffers; cross-file views include manifests.
        // Reuse facts only when Dart revisions and package identities agree exactly.
        let indexes = self.semantic_snapshot.as_ref().and_then(|snapshot| {
            let mut dart_files = files.iter().filter(|file| is_dart_file(&file.path));
            let same_files = dart_files.clone().count() == snapshot.files.len()
                && dart_files.all(|file| {
                    snapshot
                        .by_path
                        .get(&file.path)
                        .is_some_and(|&index| same_revision(&snapshot.files[index], file))
                });
            if !same_files {
                return None;
            }
            let packages = package_identities(files.iter().map(|file| file.path.as_path()));
            same_packages(&packages, &snapshot.packages).then_some(&snapshot.indexes)
        });
        let reuse_snapshot = indexes.is_some();
        if !reuse_snapshot && self.cross_file_resolve_semantics {
            self.ensure_cross_file_indexes(&files);
        }
        let indexes = if reuse_snapshot {
            self.semantic_snapshot
                .as_ref()
                .map(|snapshot| &snapshot.indexes)
        } else {
            self.cross_file_index_cache
                .as_ref()
                .map(|cache| &cache.indexes)
        };
        let diags = match run_cross_file(
            &self.environment,
            &self.cross_file_rules,
            &files,
            &self.config,
            indexes,
        ) {
            Ok(diags) => diags,
            Err(error) => {
                // Editors only show open buffers, so every open document carries the failure.
                warn!("could not start cross-file rules thread: {error}");
                return self
                    .documents
                    .values()
                    .map(|doc| {
                        (
                            doc.path.to_string_lossy().into_owned(),
                            vec![falcon_analyze::rules_thread_diagnostic(&doc.path, &error)],
                        )
                    })
                    .collect();
            }
        };
        let mut grouped: HashMap<String, Vec<Diagnostic>> = HashMap::new();
        for diag in diags {
            grouped
                .entry(diag.file_path.clone())
                .or_default()
                .push(diag);
        }
        grouped
    }

    fn ensure_cross_file_indexes(&mut self, files: &[ProjectFile]) {
        let dart_files = || files.iter().filter(|file| is_dart_file(&file.path));
        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        if self.cross_file_index_cache.as_ref().is_some_and(|cache| {
            dart_files().count() == cache.files.len()
                && same_packages(&packages, &cache.packages)
                && dart_files()
                    .zip(&cache.files)
                    .all(|(file, cached)| file.path == cached.path && same_revision(file, cached))
        }) {
            return;
        }
        let indexes = self.cross_file_indexes(files);
        self.cross_file_index_cache = Some(CrossFileIndexCache {
            files: dart_files()
                .map(|file| ProjectFile {
                    path: file.path.clone(),
                    parsed: Arc::clone(&file.parsed),
                    has_parse_errors: file.has_parse_errors,
                })
                .collect(),
            indexes,
            packages,
        });
        self.cross_file_index_build_count += 1;
    }

    /// Resolve the cross-file view independently when open buffers or disk edits
    /// prevent reuse of the semantic snapshot.
    fn cross_file_indexes(&mut self, files: &[ProjectFile]) -> SemanticIndexes {
        let files: Vec<_> = files
            .iter()
            .filter(|file| is_dart_file(&file.path))
            .collect();
        let paths: HashSet<_> = files.iter().map(|file| file.path.as_path()).collect();
        // The external layer excludes semantic project files. URI imports of
        // removed SDK/package files may be absent from the local dependency graph.
        let cache = self.external_indexes.as_ref().filter(|_| {
            self.semantic_snapshot.as_ref().is_some_and(|snapshot| {
                files.iter().all(|file| {
                    snapshot
                        .by_path
                        .get(&file.path)
                        .is_some_and(|&index| same_revision(&snapshot.files[index], file))
                }) && snapshot.files.iter().enumerate().all(|(index, file)| {
                    paths.contains(file.path.as_path())
                        || (!files.iter().any(|retained| {
                            self.environment
                                .sdk(&retained.path)
                                .is_some_and(|sdk| file.path.starts_with(&sdk.root))
                                || self.environment.package_config(&retained.path).is_some_and(
                                    |config| {
                                        config
                                            .packages
                                            .iter()
                                            .any(|package| file.path.starts_with(&package.lib_root))
                                    },
                                )
                        }) && snapshot.reverse_dependencies[index]
                            .iter()
                            .all(|&dependent| {
                                !paths.contains(snapshot.files[dependent].path.as_path())
                            }))
                })
            })
        });
        if let Some(cache) = cache {
            return self
                .build_semantic_indexes(&files, &self.external_libraries, Some(cache))
                .0;
        }
        let graph_roots: Vec<_> = files
            .iter()
            .map(|file| {
                let program = file.program();
                let mut uris: Vec<_> = program
                    .imports()
                    .map(|import| import.uri().value().into_owned())
                    .chain(
                        program
                            .exports()
                            .map(|export| export.uri().value().into_owned()),
                    )
                    .chain(
                        program
                            .part_directives()
                            .map(|part| part.uri().value().into_owned()),
                    )
                    .collect();
                if let Some(uri) = program.part_of_directive().and_then(|part| part.uri()) {
                    uris.push(uri.value().into_owned());
                }
                uris.extend(
                    falcon_analyze::resolve::doc_imports(program, file.source()).map(str::to_owned),
                );
                (file.path.clone(), uris)
            })
            .collect();
        if !self
            .cross_file_external_cache
            .as_ref()
            .is_some_and(|cache| cache.roots == graph_roots)
        {
            let roots: Vec<_> = files
                .iter()
                .map(|file| (file.path.as_path(), file.program(), file.source()))
                .collect();
            let libraries =
                ExternalLibraries::load(&self.environment, &roots, |path| paths.contains(path));
            let topology = ExternalTopology::build(&libraries, &self.environment);
            let depends_on_project = files
                .iter()
                .any(|file| topology.project_dependency_targets.contains(&file.path));
            let indexes = (!depends_on_project).then(|| {
                ExternalIndexCache::build(ExternalSources {
                    libraries: &libraries,
                    environment: &self.environment,
                })
            });
            let watched_paths = libraries
                .libraries()
                .iter()
                .flat_map(|library| library.files.iter())
                .map(|file| file.path.as_path())
                .chain(libraries.attempted_project_paths())
                .chain(libraries.project_companions())
                .map(watched_path)
                .collect();
            self.cross_file_external_cache = Some(CrossFileExternalCache {
                roots: graph_roots,
                libraries,
                indexes,
                watched_paths,
            });
            self.cross_file_external_load_count += 1;
        }
        let cache = self.cross_file_external_cache.as_ref().unwrap();
        self.build_semantic_indexes(&files, &cache.libraries, cache.indexes.as_ref())
            .0
    }

    /// Build both LSP views with identical grouping and resolver layering.
    fn build_semantic_indexes(
        &self,
        files: &[&ProjectFile],
        external: &ExternalLibraries,
        cache: Option<&ExternalIndexCache>,
    ) -> (SemanticIndexes, LibraryGrouping, Vec<PackageIdentity>) {
        let grouping_files: Vec<_> = files
            .iter()
            .map(|file| (file.path.clone(), file.program()))
            .collect();
        let grouping = group_libraries(&grouping_files);
        let identity_sources: Vec<_> = files
            .iter()
            .map(|file| IdentitySource {
                path: &file.path,
                parsed: &file.parsed,
                has_parse_errors: file.has_parse_errors,
            })
            .collect();
        let sources: Vec<_> = files.iter().map(|file| file.source()).collect();
        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        let identities = IdentityIndex::build_layer(
            &identity_sources,
            &sources,
            &packages,
            Some(ExternalSources {
                libraries: external,
                environment: &self.environment,
            }),
            cache.map(|cache| Arc::clone(&cache.identities)),
            Some(&grouping),
        );
        let resolved_files = || {
            files.iter().enumerate().map(|(index, file)| {
                (
                    file.path.as_path(),
                    LibrarySource {
                        program: file.program(),
                        has_parse_errors: file.has_parse_errors,
                        has_unresolved_parts: grouping.is_unresolved(index),
                    },
                )
            })
        };
        let mut types = if let Some(cache) = cache {
            TypeIndex::with_external(resolved_files(), &identities, Arc::clone(&cache.types))
        } else {
            TypeIndex::from_resolved_files(resolved_files(), &identities)
        };
        if cache.is_none() {
            types.add_external(external, &identities);
        }
        let signature_files: Vec<_> = files
            .iter()
            .map(|file| (file.path.clone(), Arc::clone(&file.parsed)))
            .collect();
        let signatures = if let Some(cache) = cache {
            SignatureIndex::with_external(
                &signature_files,
                &identities,
                &types,
                Arc::clone(&cache.signatures),
            )
        } else {
            SignatureIndex::build(&signature_files, Some(external), &identities, &types)
        };
        (
            SemanticIndexes {
                types,
                identities,
                signatures,
            },
            grouping,
            packages,
        )
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
                std::fs::canonicalize(uri_to_path(uri))
                    .ok()
                    .map(|path| (uri.clone(), path))
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
        let (mut files, _) = self.collect_project_files(ProjectView::Semantic);
        {
            let project_paths: HashSet<&Path> = files
                .iter()
                .map(|file| file.path.as_path())
                .chain(
                    self.documents
                        .values()
                        .filter(|doc| doc.source_rejected)
                        .map(|doc| doc.path.as_path()),
                )
                .collect();
            let roots: Vec<(&Path, Program<'_>, &str)> = files
                .iter()
                .map(|file| (file.path.as_path(), file.program(), file.source()))
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
        seen.extend(
            self.documents
                .values()
                .filter(|doc| doc.source_rejected)
                .map(|doc| doc.path.clone()),
        );
        for path in self.external_libraries.project_companions() {
            if !seen.insert(path.to_path_buf()) {
                continue;
            }
            match falcon_analyze::source::read_source(path) {
                Ok(source) => {
                    let falcon_dart_parser::ParseResult { tree, errors } =
                        parse_for_package(&source, self.environment.package_language_version(path));
                    files.push(ProjectFile {
                        path: path.to_path_buf(),
                        parsed: Arc::new(ParsedFile {
                            source: source.into_boxed_str(),
                            tree,
                        }),
                        has_parse_errors: !errors.is_empty(),
                    });
                }
                Err(error) => warn!("failed to read {}: {}", path.display(), error),
            }
        }
        let external = ExternalSources {
            libraries: &self.external_libraries,
            environment: &self.environment,
        };
        // A dependency importing a project file depends on mutable buffer facts.
        // Keep the ordinary combined build for that graph instead of freezing
        // unresolved/project references into the immutable external layer.
        let topology_matches = self
            .external_topology
            .as_ref()
            .is_some_and(|cache| cache.matches(&self.external_libraries));
        let mut local_sources: Vec<_> = self.loaded_project_paths().collect();
        local_sources.sort_unstable();
        local_sources.dedup();
        if !self
            .watched_local_sources
            .iter()
            .map(PathBuf::as_path)
            .eq(local_sources.iter().copied())
        {
            let mut paths: Vec<_> = local_sources
                .iter()
                .map(|path| watched_path(path))
                .collect();
            paths.sort_unstable();
            paths.dedup();
            let sources = local_sources.into_iter().map(Path::to_path_buf).collect();
            self.watched_local_sources = sources;
            if self.watched_local_paths != paths {
                self.watched_local_paths = paths;
                self.watched_dependency_generation += 1;
            }
        }
        // URI resolution can canonicalize paths. Retain these immutable graph
        // edges rather than repeating filesystem work for every text edit.
        if !topology_matches {
            self.external_topology = Some(ExternalTopology::build(
                &self.external_libraries,
                &self.environment,
            ));
        }
        let external_depends_on_project = self.external_topology.as_ref().is_some_and(|cache| {
            files
                .iter()
                .any(|file| cache.project_dependency_targets.contains(&file.path))
        });
        if external_depends_on_project {
            self.external_indexes = None;
        } else if !topology_matches || self.external_indexes.is_none() {
            self.external_indexes = Some(ExternalIndexCache::build(external));
            self.external_index_build_count += 1;
        }
        if self.external_indexes.is_none() {
            self.external_index_build_count += 1;
        }
        let project_files: Vec<_> = files.iter().collect();
        let (indexes, grouping, packages) = self.build_semantic_indexes(
            &project_files,
            &self.external_libraries,
            self.external_indexes.as_ref(),
        );
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
            indexes,
            packages,
            grouping,
            by_path,
            reverse_dependencies,
        });
        self.semantic_snapshot_build_count += 1;
    }

    fn admits_project_dart(&self, path: &Path) -> bool {
        is_dart_file(path) && self.config.files.includes.admits(&path.to_string_lossy())
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
    /// the workspace root, preferring an open buffer's text + cached CST over the
    /// on-disk copy. Semantic analysis also includes open buffers that do not exist
    /// on disk yet. Cross-file analysis can additionally include package manifests.
    /// The boolean is false when oversized ingestion left the workspace incomplete;
    /// cross-file rules must not infer absence from that partial set.
    fn collect_project_files(&mut self, view: ProjectView) -> (Vec<ProjectFile>, bool) {
        let mut complete = true;
        let filter = &self.config.files.includes;
        let mut paths = Vec::new();
        // Never follow symlinks: Flutter's ephemeral/.plugin_symlinks point into
        // the pub cache (same hazard the CLI walker fixed) and a long-lived LSP
        // would hold all of it in memory.
        for entry in WalkDir::new(&self.workspace_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| {
                !entry.file_type().is_dir() || !filter.excludes_directory(entry.path())
            })
        {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    warn!("error walking workspace: {}", e);
                    continue;
                }
            };
            let path = entry.path();
            let is_dart = is_dart_file(path);
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
                if doc.source_rejected {
                    complete = false;
                    continue;
                }
                files.push(ProjectFile {
                    path: normalized,
                    parsed: Arc::clone(&doc.parsed),
                    has_parse_errors: doc.has_parse_errors,
                });
            } else {
                match falcon_analyze::source::read_source(path) {
                    Ok(source) => {
                        let (tree, has_parse_errors) = if is_dart {
                            let package_version = self.environment.package_language_version(path);
                            let parsed = parse_for_package(&source, package_version);
                            (parsed.tree, !parsed.errors.is_empty())
                        } else {
                            (lex_tree(&source), false)
                        };
                        files.push(ProjectFile {
                            path: normalized,
                            parsed: Arc::new(ParsedFile {
                                source: source.into_boxed_str(),
                                tree,
                            }),
                            has_parse_errors,
                        });
                    }
                    Err(e) => {
                        warn!("failed to read {}: {}", path.display(), e);
                        if falcon_analyze::source::is_source_too_large(&e) {
                            complete = false;
                        }
                    }
                }
            }
        }
        if matches!(view, ProjectView::Semantic) {
            for (path, doc) in open_by_path {
                if !doc.source_rejected && !seen.contains(path) {
                    files.push(ProjectFile {
                        path: path.to_path_buf(),
                        parsed: Arc::clone(&doc.parsed),
                        has_parse_errors: doc.has_parse_errors,
                    });
                }
            }
        }
        (files, complete)
    }
}

/// Drop rejected editor text before any parsing, language-version scanning, or
/// semantic retention. A subsequent valid full-sync change recovers normally.
fn reject_oversized_buffer(path: &Path, text: &mut String) -> Option<Diagnostic> {
    let diagnostic = falcon_analyze::parallel::source_size_diagnostic(path, text.len() as u64)?;
    *text = String::new();
    Some(diagnostic)
}

#[cfg(test)]
mod source_size_tests {
    use super::*;

    #[test]
    fn ordinary_disk_read_errors_keep_project_collection_complete() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("main.dart");
        std::fs::write(&valid, "void main() {}").unwrap();
        std::fs::write(directory.path().join("invalid.dart"), [0xff]).unwrap();
        std::fs::write(directory.path().join("pubspec.yaml"), [0xff]).unwrap();
        std::fs::create_dir(directory.path().join("directory.dart")).unwrap();
        let config = directory.path().join("falcon.json");
        std::fs::write(&config, "{}").unwrap();
        let mut state = LspState::new(Some(config));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, canonical_or_lexical(&valid));
    }

    #[test]
    fn ordinary_walk_errors_keep_project_collection_complete() {
        let directory = tempfile::tempdir().unwrap();
        let mut state = LspState::new(None);
        state.workspace_root = directory.path().join("missing");
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        assert!(files.is_empty());
    }

    #[test]
    fn rejected_buffer_diagnostic_survives_refresh_and_recovers_on_change() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("main.dart"), "void main() {}").unwrap();
        let uri = format!("file://{}", directory.path().join("main.dart").display());
        let mut state = LspState::new(None);
        state.workspace_root = directory.path().to_path_buf();
        state.open(&uri, String::new(), Some(1));
        let doc = state.documents.get_mut(&uri).unwrap();
        doc.source_rejected = true;
        doc.has_parse_errors = true;
        doc.syntax_diagnostics = vec![
            falcon_analyze::parallel::source_size_diagnostic(&doc.path, u64::from(u32::MAX) + 1)
                .unwrap(),
        ];
        let parse_count = doc.parse_count;
        let diagnostics = state.analyze(&uri);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "syntax-error")
        );
        assert_eq!(state.document(&uri).unwrap().parse_count, parse_count);
        assert_eq!(diagnostics.len(), 1);
        assert!(
            state
                .collect_project_files(ProjectView::Semantic)
                .0
                .is_empty()
        );
        assert!(
            state
                .collect_project_files(ProjectView::CrossFile)
                .0
                .is_empty()
        );
        assert!(state.cross_file_diagnostics().is_empty());

        assert!(state.change(&uri, "void main() {}".into(), Some(2)));
        assert!(!state.document(&uri).unwrap().source_rejected);
        assert!(
            !state
                .analyze(&uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == "syntax-error")
        );
    }

    #[test]
    fn supported_buffer_text_is_not_modified() {
        let mut text = "// 🦅\nvoid main() {}".to_string();
        let original = text.clone();
        assert!(reject_oversized_buffer(Path::new("main.dart"), &mut text).is_none());
        assert_eq!(text, original);
    }

    #[test]
    fn oversized_disk_sources_clear_cross_file_diagnostics_and_recover() {
        for name in ["oversized.dart", "pubspec.yaml"] {
            let directory = tempfile::tempdir().unwrap();
            let config = directory.path().join("falcon.json");
            std::fs::write(
                &config,
                r#"{"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false,"correctness":{"unused-files":"warn"}}}}"#,
            )
            .unwrap();
            let lib = directory.path().join("lib");
            std::fs::create_dir(&lib).unwrap();
            std::fs::write(lib.join("main.dart"), "void main() {}").unwrap();
            let orphan = lib.join("orphan.dart");
            std::fs::write(&orphan, "class Orphan {}").unwrap();
            let uri = format!("file://{}", orphan.display());
            let mut state = LspState::new(Some(config));
            state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);
            state.open(&uri, "class Orphan {}".into(), Some(1));
            assert!(state.cross_file_pass().iter().any(|(_, diagnostics)| {
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unused-files")
            }));

            let rejected = directory.path().join(name);
            std::fs::File::create(&rejected)
                .unwrap()
                .set_len(u64::from(u32::MAX) + 1)
                .unwrap();
            let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
            assert!(!complete);
            assert!(files.iter().all(|file| file.path != rejected));
            assert!(state.cross_file_diagnostics().is_empty());
            let published = state.cross_file_pass();
            let (_, diagnostics) = published.iter().find(|(key, _)| key == &uri).unwrap();
            assert!(
                diagnostics
                    .iter()
                    .all(|diagnostic| diagnostic.rule != "unused-files")
            );

            std::fs::remove_file(rejected).unwrap();
            assert!(state.cross_file_diagnostics().values().any(|diagnostics| {
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unused-files")
            }));
        }
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
            .program()
            .imports()
            .map(|directive| directive.uri().value())
            .chain(
                file.program()
                    .exports()
                    .map(|directive| directive.uri().value()),
            )
        {
            if let Some(target) = resolve_dependency(&file.path, &dependency, by_path, packages) {
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

/// The HOME config supplies settings, not a workspace to traverse.
fn workspace_root_for(config_path: Option<&Path>, start_dir: &Path) -> PathBuf {
    let root = config_path
        .filter(|path| !falcon_config::is_home_config(path))
        .and_then(Path::parent)
        .unwrap_or(start_dir);
    canonical_or_lexical(root)
}

fn load_from(
    path: Option<&Path>,
    start_dir: &Path,
) -> (Option<FalconConfig>, PathBuf, Vec<ShowMessageParams>) {
    let discovered = path
        .map(Path::to_path_buf)
        .or_else(|| falcon_config::find_config(start_dir));
    let workspace_root = workspace_root_for(discovered.as_deref(), start_dir);
    let loaded = match discovered.as_deref() {
        Some(path) => load_config(path),
        None => load_or_default(start_dir),
    };
    let label = discovered.as_deref().unwrap_or(start_dir).display();
    match loaded {
        Ok(mut loaded) => {
            if loaded.config.root_dir() != Some(workspace_root.as_path()) {
                loaded.config.resolve_paths(&workspace_root);
            }
            (
                Some(loaded.config),
                workspace_root,
                loaded
                    .warnings
                    .into_iter()
                    .map(|message| ShowMessageParams {
                        typ: MessageType::WARNING,
                        message: format!("Falcon: {}: {message}", label),
                    })
                    .collect(),
            )
        }
        Err(error) => {
            let message = format!("Falcon: failed to load config {}: {error}", label);
            warn!("{message}");
            (
                None,
                workspace_root,
                vec![ShowMessageParams {
                    typ: MessageType::ERROR,
                    message,
                }],
            )
        }
    }
}

fn is_dart_file(path: &Path) -> bool {
    path.extension().and_then(|extension| extension.to_str()) == Some("dart")
}

/// Whether two parses of one path carry the same source and recovery state.
fn same_revision(file: &ProjectFile, other: &ProjectFile) -> bool {
    file.source() == other.source() && file.has_parse_errors == other.has_parse_errors
}

fn same_packages(current: &[PackageIdentity], cached: &[PackageIdentity]) -> bool {
    current.len() == cached.len()
        && current.iter().zip(cached).all(|(current, cached)| {
            current.name == cached.name && current.lib_root == cached.lib_root
        })
}

fn watched_path(path: &Path) -> PathBuf {
    for ancestor in path.ancestors() {
        if let Ok(resolved) = std::fs::canonicalize(ancestor) {
            let suffix = path.strip_prefix(ancestor).expect("path ancestor");
            return if suffix.as_os_str().is_empty() {
                resolved
            } else {
                resolved.join(suffix)
            };
        }
    }
    canonical_or_lexical(path)
}

/// Keep missing inner symlinks lexical until materialization, resolving only the workspace prefix.
fn document_path(uri: &str, workspace_root: &Path) -> (PathBuf, bool) {
    let path = uri_to_path(uri);
    match std::fs::canonicalize(&path) {
        Ok(path) => (path, false),
        Err(_) => {
            let path = falcon_analyze::resolve::lexical_normalize(&path);
            for ancestor in path.ancestors() {
                if canonical_or_lexical(ancestor) == workspace_root {
                    let suffix = path.strip_prefix(ancestor).expect("path ancestor");
                    return (workspace_root.join(suffix), true);
                }
            }
            (path, true)
        }
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
mod config_tests {
    use super::*;

    #[test]
    fn canonicalizing_loader_preserves_legacy_rule_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(
            &path,
            r#"{
            "linter": {"rules": {"recommended": false, "complexity": {"avoid_nested_if": "error"}}},
            "cross-file": {"enabled": false}
        }"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path), |_| None);
        assert!(state.config.linter.rules.groups["complexity"].contains_key("avoid-nested-if"));
        assert!(!state.config.linter.rules.groups["complexity"].contains_key("avoid_nested_if"));
        assert!(
            state
                .take_config_messages()
                .iter()
                .any(|message| message.typ == MessageType::WARNING
                    && message.message.contains("avoid_nested_if"))
        );
        let uri = format!("file://{}", dir.path().join("main.dart").display());
        let diagnostics = state.open(
            &uri,
            "void f(bool a, bool b, bool c) { if (a) { if (b) { if (c) { print(a); } } } }\n"
                .to_string(),
            Some(1),
        );
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-nested-if"
                    && diagnostic.severity == falcon_diagnostics::Severity::Error)
        );
    }

    const SOURCE: &str = "void f() { dynamic value = 1; print(value); }\n";

    #[test]
    fn relative_filters_and_overrides_match_absolute_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(
            &path,
            r#"{
            "files": {"includes": ["lib/**", "!lib/generated/**"]},
            "linter": {"rules": {"recommended": false, "suspicious": {"avoid-dynamic": "warn"}}},
            "cross-file": {"enabled": false},
            "overrides": [{
                "includes": ["lib/quiet/*.dart"],
                "linter": {"rules": {"suspicious": {"avoid-dynamic": "off"}}}
            }]
        }"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path), |_| None);
        for (relative, expected) in [
            ("lib/main.dart", true),
            ("lib/quiet/main.dart", false),
            ("lib/quiet/deep/main.dart", true),
            ("lib/generated/main.dart", false),
            ("test/main.dart", false),
        ] {
            let uri = format!("file://{}", dir.path().join(relative).display());
            let diagnostics = state.open(&uri, SOURCE.to_string(), Some(1));
            assert_eq!(
                diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "avoid-dynamic"),
                expected,
                "{relative}: {diagnostics:?}"
            );
        }
    }

    #[test]
    fn discovered_config_root_drives_workspace_collection() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".git")).unwrap();
        std::fs::create_dir_all(dir.path().join("nested")).unwrap();
        std::fs::write(dir.path().join("falcon.json"), "{}").unwrap();
        let main = dir.path().join("main.dart");
        std::fs::write(&main, "void main() {}\n").unwrap();
        let mut state = LspState::new_from(None, |_| None, dir.path().join("nested"));
        assert_eq!(state.workspace_root, canonical_or_lexical(dir.path()));
        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        assert!(
            files
                .iter()
                .any(|file| file.path == canonical_or_lexical(&main))
        );
    }

    #[test]
    fn excluded_subtrees_do_not_make_workspace_collection_incomplete() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("falcon.json");
        std::fs::write(
            &config,
            r#"{
            "files":{"includes":["!build/**"]},
            "linter":{"rules":{"recommended":false}},
            "cross-file":{"enabled":false}
        }"#,
        )
        .unwrap();
        let excluded = dir.path().join("build/deep");
        std::fs::create_dir_all(&excluded).unwrap();
        for name in ["oversized.dart", "pubspec.yaml"] {
            std::fs::File::create(excluded.join(name))
                .unwrap()
                .set_len(u64::from(u32::MAX) + 1)
                .unwrap();
        }
        let main = dir.path().join("main.dart");
        std::fs::write(&main, "void main() {}\n").unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        for view in [ProjectView::Semantic, ProjectView::CrossFile] {
            let (files, complete) = state.collect_project_files(view);
            assert!(complete);
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].path, canonical_or_lexical(&main));
        }
    }

    #[test]
    fn home_fallback_settings_do_not_expand_workspace_to_home() {
        const CHILD: &str = "FALCON_TEST_LSP_HOME_CONFIG_ROOT";
        if std::env::var_os(CHILD).is_none() {
            let dir = tempfile::tempdir().unwrap();
            let workspace = dir.path().join("project");
            std::fs::create_dir_all(&workspace).unwrap();
            std::fs::write(
                dir.path().join(".falcon.json"),
                r#"{
                    "files":{"includes":["lib/**", "!lib/generated/**"]},
                    "linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"warn"}}},
                    "cross-file":{"enabled":false},
                    "overrides":[{
                        "includes":["lib/quiet/**"],
                        "linter":{"rules":{"suspicious":{"avoid-dynamic":"off"}}}
                    }]
                }"#,
            )
            .unwrap();
            #[cfg(not(unix))]
            let home = dir.path().to_path_buf();
            #[cfg(unix)]
            let home = {
                let alias = dir.path().join("home-alias");
                std::os::unix::fs::symlink(dir.path(), &alias).unwrap();
                alias
            };
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "state::config_tests::home_fallback_settings_do_not_expand_workspace_to_home",
                ])
                .env(CHILD, "1")
                .env("HOME", home)
                .current_dir(&workspace)
                .output()
                .unwrap();
            let stdout = String::from_utf8_lossy(&output.stdout);
            assert!(
                output.status.success() && stdout.contains("1 passed"),
                "{stdout}\n{}",
                String::from_utf8_lossy(&output.stderr)
            );
            return;
        }

        let workspace = std::env::current_dir().unwrap();
        let home_config = workspace.parent().unwrap().join(".falcon.json");
        let other_workspace = workspace.join("other");
        std::fs::create_dir_all(&other_workspace).unwrap();
        for (config_path, workspace) in [
            (None, workspace.clone()),
            (Some(home_config.clone()), workspace.clone()),
            (Some(PathBuf::from("../.falcon.json")), workspace.clone()),
            (Some(home_config), other_workspace),
        ] {
            let mut state = LspState::new_from(config_path, |_| None, workspace.clone());
            assert_eq!(state.workspace_root, canonical_or_lexical(&workspace));
            assert_eq!(
                state.config.root_dir(),
                Some(state.workspace_root.as_path())
            );
            for (relative, expected) in [
                ("lib/main.dart", true),
                ("lib/quiet/main.dart", false),
                ("lib/generated/main.dart", false),
                ("test/main.dart", false),
            ] {
                let uri = format!("file://{}", workspace.join(relative).display());
                let diagnostics = state.open(&uri, SOURCE.to_string(), Some(1));
                assert_eq!(
                    diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.rule == "avoid-dynamic"),
                    expected,
                    "{relative}: {diagnostics:?}"
                );
            }
        }

        let local = workspace.join("settings/.falcon.json");
        std::fs::create_dir_all(local.parent().unwrap()).unwrap();
        std::fs::write(&local, "{}").unwrap();
        let state = LspState::new_from(Some(local), |_| None, workspace.clone());
        assert_eq!(
            state.workspace_root,
            canonical_or_lexical(&workspace.join("settings")),
            "a non-HOME config still determines the workspace root"
        );
    }

    #[test]
    fn invalid_reload_preserves_rules_root_and_open_document_diagnostics() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(
            &path,
            r#"{
            "linter":{"rules":{"recommended":false,"suspicious":{"avoid-dynamic":"error"}}},
            "cross-file":{"enabled":false}
        }"#,
        )
        .unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path.clone()), |_| None);
        let root = state.workspace_root.clone();
        let uri = format!("file://{}", dir.path().join("main.dart").display());
        let before = state.open(&uri, SOURCE.to_string(), Some(1));
        std::fs::write(&path, r#"{"linter":{"enabledd":false}}"#).unwrap();
        assert!(state.reload_config().is_empty());
        assert_eq!(state.workspace_root, root);
        assert!(!state.cross_file_rules_enabled());
        assert_eq!(state.config.linter.rules.recommended, Some(false));
        let after = state.analyze(&uri);
        assert_eq!(before.len(), after.len());
        assert!(
            after
                .iter()
                .any(|diagnostic| diagnostic.rule == "avoid-dynamic"
                    && diagnostic.severity == falcon_diagnostics::Severity::Error)
        );
        let messages = state.take_config_messages();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].message.contains(&path.display().to_string()));
        assert!(messages[0].message.contains("linter.enabledd"));
        state.reload_config();
        assert!(state.take_config_messages().is_empty());
    }

    #[test]
    fn nested_config_events_do_not_repeat_unchanged_active_warnings() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(&path, r#"{"max_errors":10}"#).unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path.clone()), |_| None);
        let messages = state.take_config_messages();
        assert!(
            messages
                .iter()
                .all(|message| message.message.contains(&path.display().to_string()))
        );
        state.reload_config();
        assert!(state.take_config_messages().is_empty());
        let nested = dir.path().join("nested/falcon.json");
        std::fs::create_dir_all(nested.parent().unwrap()).unwrap();
        std::fs::write(&nested, "{}").unwrap();
        state.watched_files_changed(&[nested]);
        assert!(state.take_config_messages().is_empty());
    }

    #[test]
    fn config_messages_are_drained_and_reloaded() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("falcon.json");
        std::fs::write(&path, r#"{"max_errors": 10}"#).unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(path.clone()), |_| None);
        let messages = state.take_config_messages();
        assert!(
            messages
                .iter()
                .any(|message| message.typ == MessageType::WARNING
                    && message.message.contains("max_errors"))
        );
        assert!(state.take_config_messages().is_empty());
        std::fs::write(&path, r#"{"overrides": [{"incldues": ["lib/**"]}]}"#).unwrap();
        state.reload_config();
        let messages = state.take_config_messages();
        assert!(
            messages
                .iter()
                .any(|message| message.typ == MessageType::ERROR
                    && message.message.contains("overrides[0].incldues"))
        );
        assert!(state.take_config_messages().is_empty());
    }
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
        let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse("");
        let files = [ProjectFile {
            path: path.clone(),
            parsed: Arc::new(ParsedFile {
                source: "".into(),
                tree,
            }),
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

    fn switch_has_pattern(program: Program<'_>) -> bool {
        use falcon_syntax::cst::{FunctionBody, Stmt, SwitchCaseKind, TopLevelDecl};
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!("expected function")
        };
        let Some(FunctionBody::Block(block)) = function.body() else {
            panic!("expected function body")
        };
        let Stmt::Switch(switch) = block.stmts().next().unwrap() else {
            panic!("expected switch")
        };
        matches!(
            switch.cases().next().unwrap().cases().next().unwrap(),
            SwitchCaseKind::Pattern(..)
        )
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
            assert!(!switch_has_pattern(state.documents[&uri].parsed.program()));
            assert_eq!(state.documents[&uri].parse_count, 1);

            write_package_version(dir.path(), "3.0");
            let published = state.reload_config();
            let diagnostics = &published.iter().find(|(key, _)| key == &uri).unwrap().1;
            assert!(diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(switch_has_pattern(state.documents[&uri].parsed.program()));
            assert_eq!(state.documents[&uri].parse_count, 2);
            assert_eq!(state.documents[&marker_uri].parse_count, 1);
            assert_eq!(state.documents[&unrelated_uri].parse_count, 1);
            assert!(!switch_has_pattern(
                state.documents[&marker_uri].parsed.program()
            ));
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
                assert!(switch_has_pattern(file.program()));
                assert!(file.has_parse_errors);
            }

            write_package_version(dir.path(), "2.19");
            state.watched_files_changed(&[dir.path().join(".dart_tool/package_config.json")]);
            assert!(state.change(&uri, VERSIONED_SWITCH.to_string(), Some(2)));
            let diagnostics = state.analyze(&uri);
            assert!(!diagnostics.iter().any(|diag| diag.rule == "syntax-error"));
            assert!(!switch_has_pattern(state.documents[&uri].parsed.program()));
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
            assert_eq!(switch_has_pattern(file.program()), pattern);
            assert_eq!(file.has_parse_errors, pattern);
        }
        let (files, _) = state.collect_project_files(ProjectView::CrossFile);
        for (path, pattern) in paths.iter().zip([false, true]) {
            let file = files.iter().find(|file| &file.path == path).unwrap();
            assert_eq!(switch_has_pattern(file.program()), pattern);
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
            assert!(switch_has_pattern(state.documents[&uri].parsed.program()));
            assert!(!switch_has_pattern(
                state.documents[&marker_uri].parsed.program()
            ));
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
            assert!(!switch_has_pattern(state.documents[&uri].parsed.program()));
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
        let source = "name: workspace\nvalue: '$Simple ${Braced}'\n";
        std::fs::write(&manifest_path, source).unwrap();
        std::fs::write(dir.path().join("main.dart"), "void main() {}\n").unwrap();
        let mut state = LspState::new(Some(config_path));
        state.environment = AnalysisEnvironment::with_sdk_locator(|_| None);

        let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
        assert!(complete);
        let manifest = files
            .iter()
            .find(|file| file.path == canonical_or_lexical(&manifest_path))
            .expect("pubspec.yaml must be included in the cross-file project view");
        assert_eq!(manifest.source(), source);
        assert!(!manifest.has_parse_errors);
        assert!(manifest.program().declarations().is_empty());
        assert_eq!(
            manifest
                .tree()
                .tokens(manifest.source())
                .next()
                .unwrap()
                .text(),
            "name"
        );
        assert_eq!(
            manifest
                .tree()
                .tokens(manifest.source())
                .last()
                .unwrap()
                .kind(),
            falcon_syntax::TokenKind::Eof
        );
        assert_eq!(
            manifest
                .tree()
                .identifier_tokens(manifest.source())
                .map(|token| token.text())
                .collect::<Vec<_>>(),
            ["name", "workspace", "value", "Simple", "Braced"]
        );
    }

    #[test]
    fn opening_files_outside_cross_file_view_preserves_nullable_diagnostics() {
        for opened in ["external", "excluded", "dependency"] {
            let dir = tempfile::tempdir().unwrap();
            let app = dir.path().join("app");
            let flutter = dir.path().join("flutter");
            let sdk = flutter.join("bin/cache/dart-sdk/lib");
            std::fs::create_dir_all(sdk.join("core")).unwrap();
            std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(app.join("lib")).unwrap();
            let dependency = dir.path().join("dep/lib/api.dart");
            std::fs::create_dir_all(dependency.parent().unwrap()).unwrap();
            let dependency_source = "int value() => 1;\n";
            std::fs::write(&dependency, dependency_source).unwrap();
            std::fs::write(
                sdk.join("libraries.json"),
                r#"{"vm":{"libraries":{"core":{"uri":"core/core.dart"}}}}"#,
            )
            .unwrap();
            std::fs::write(
                sdk.join("core/core.dart"),
                "library dart.core; class Object {} abstract final class int {}",
            )
            .unwrap();
            std::fs::write(
                app.join(".dart_tool/package_config.json"),
                format!(
                    r#"{{"configVersion":2,"flutterRoot":"file://{}","packages":[
                        {{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.9"}},
                        {{"name":"dep","rootUri":"../../dep","packageUri":"lib/","languageVersion":"3.9"}}
                    ]}}"#,
                    flutter.display(),
                ),
            )
            .unwrap();
            let config = app.join("falcon.json");
            std::fs::write(
                &config,
                r#"{
                    "linter":{"enabled":false},
                    "cross-file":{"rules":{"recommended":false,
                        "correctness":{"unnecessary-nullable":"warn"}}}
                }"#,
            )
            .unwrap();
            let path = app.join("lib/main.dart");
            let source = "import 'package:dep/api.dart';\nvoid _take(int? value) {}\nvoid main() { _take(value()); }\n";
            std::fs::write(&path, source).unwrap();
            let other = match opened {
                "excluded" => app.join("lib/excluded.dart"),
                "dependency" => dependency,
                _ => dir.path().join("external.dart"),
            };
            let other_source = if opened == "dependency" {
                dependency_source
            } else {
                "void unrelated() {}\n"
            };
            std::fs::write(&other, other_source).unwrap();
            let mut state = LspState::new_with_sdk_locator(Some(config), |config| {
                falcon_analyze::environment::DartSdk::load(
                    &config?.flutter_root.as_ref()?.join("bin/cache/dart-sdk"),
                )
            });
            if opened == "excluded" {
                state
                    .config
                    .files
                    .includes
                    .extend_excludes(&[other.to_string_lossy().into_owned()]);
            }
            let uri = format!("file://{}", path.display());
            state.open(&uri, source.into(), Some(1));
            assert!(state.environment.has_sdk());
            state.cross_file_pass();
            assert!(
                state.documents[&uri]
                    .last_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
            );

            let other_uri = format!("file://{}", other.display());
            state.open(&other_uri, other_source.into(), Some(1));
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            assert!(snapshot.by_path.contains_key(&canonical_or_lexical(&other)));
            let builds = state.semantic_snapshot_build_count;
            // An unrelated external buffer can reuse the loaded SDK layer.
            // Removed package sources conservatively require a fresh layer.
            if opened == "external" {
                std::fs::remove_file(sdk.join("core/core.dart")).unwrap();
            }
            let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
            assert!(complete);
            state.ensure_cross_file_indexes(&files);
            assert!(
                state
                    .cross_file_index_cache
                    .as_ref()
                    .unwrap()
                    .indexes
                    .identities
                    .has_sdk_sources()
            );
            let index_builds = state.cross_file_index_build_count;
            state.cross_file_pass();
            assert!(
                state.documents[&uri]
                    .last_diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
            );
            assert_eq!(state.semantic_snapshot_build_count, builds);
            state.cross_file_pass();
            assert_eq!(state.cross_file_index_build_count, index_builds);
            assert_eq!(state.semantic_snapshot_build_count, builds);
            if opened != "external" {
                let external_loads = state.cross_file_external_load_count;
                assert!(external_loads > 0);
                for version in [2, 3] {
                    assert!(state.change(
                        &uri,
                        format!("{source}// body edit {version}\n"),
                        Some(version),
                    ));
                    state.analyze(&uri);
                    state.cross_file_pass();
                    assert_eq!(state.cross_file_external_load_count, external_loads);
                    assert!(
                        state.documents[&uri]
                            .last_diagnostics
                            .iter()
                            .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
                    );
                }
                let added = dir.path().join("dep/lib/added.dart");
                std::fs::write(&added, "int addedValue() => 1;\n").unwrap();
                let changed_imports = source
                    .replace("package:dep/api.dart", "package:dep/added.dart")
                    .replace("_take(value())", "_take(addedValue())");
                assert!(state.change(&uri, changed_imports, Some(4)));
                state.analyze(&uri);
                state.cross_file_pass();
                assert_eq!(state.cross_file_external_load_count, external_loads + 1);
                assert!(
                    state
                        .cross_file_external_cache
                        .as_ref()
                        .unwrap()
                        .libraries
                        .libraries()
                        .iter()
                        .any(|library| {
                            library
                                .files
                                .iter()
                                .any(|file| file.path == canonical_or_lexical(&added))
                        })
                );
                assert!(
                    state.documents[&uri]
                        .last_diagnostics
                        .iter()
                        .any(|diagnostic| diagnostic.rule == "unnecessary-nullable")
                );
                std::fs::write(&added, "int? addedValue() => null;\n").unwrap();
                state.watched_files_changed(std::slice::from_ref(&added));
                state.cross_file_pass();
                assert_eq!(state.cross_file_external_load_count, external_loads + 2);
                assert!(
                    state.documents[&uri]
                        .last_diagnostics
                        .iter()
                        .all(|diagnostic| diagnostic.rule != "unnecessary-nullable")
                );
            }
        }
    }

    #[test]
    fn cross_file_external_cache_uses_watched_path_snapshot() {
        let path = PathBuf::from("/missing/dependency.dart");
        let cache = CrossFileExternalCache {
            roots: Vec::new(),
            libraries: ExternalLibraries::default(),
            indexes: None,
            watched_paths: HashSet::from([path.clone()]),
        };
        assert!(cache.loaded(&path));
        assert!(!cache.loaded(Path::new("/missing/other.dart")));
    }

    #[test]
    fn cross_file_external_cache_invalidates_watched_excluded_companions() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("app");
        std::fs::create_dir_all(app.join("lib")).unwrap();
        std::fs::create_dir_all(dir.path().join("dep")).unwrap();
        let config = app.join("falcon.json");
        std::fs::write(
            &config,
            r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,
                "correctness":{"unnecessary-nullable":"warn"}}}}"#,
        )
        .unwrap();
        std::fs::write(
            app.join("lib/main.dart"),
            "library; part 'excluded.dart'; void main() {}",
        )
        .unwrap();
        let companion = app.join("lib/excluded.dart");
        // Enhanced parts can introduce dependencies without changing their owner.
        std::fs::write(
            &companion,
            "part of 'main.dart'; import '../../dep/first.dart';",
        )
        .unwrap();
        let first = dir.path().join("dep/first.dart");
        let second = dir.path().join("dep/second.dart");
        std::fs::write(&first, "class First {}").unwrap();
        std::fs::write(&second, "class Second {}").unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        state
            .config
            .files
            .includes
            .extend_excludes(&[companion.to_string_lossy().into_owned()]);
        let has_external = |state: &LspState, path: &Path| {
            let path = canonical_or_lexical(path);
            state
                .cross_file_external_cache
                .as_ref()
                .unwrap()
                .libraries
                .libraries()
                .iter()
                .any(|library| library.files.iter().any(|file| file.path == path))
        };
        state.cross_file_diagnostics();
        assert!(has_external(&state, &first));
        assert!(!has_external(&state, &second));
        let loads = state.cross_file_external_load_count;
        std::fs::write(
            &companion,
            "part of 'main.dart'; import '../../dep/second.dart';",
        )
        .unwrap();
        state.watched_files_changed(std::slice::from_ref(&companion));
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_external_load_count, loads + 1);
        assert!(has_external(&state, &second));
        assert!(!has_external(&state, &first));
    }

    #[test]
    fn cross_file_indexes_cache_tracks_disk_revisions_and_file_set() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("falcon.json");
        std::fs::write(
            &config,
            r#"{"linter":{"enabled":false},"cross-file":{"rules":{"recommended":false,
                "correctness":{"unnecessary-nullable":"warn"}}}}"#,
        )
        .unwrap();
        let path = dir.path().join("main.dart");
        std::fs::write(&path, "void _take(int? value) {} void main() { _take(1); }").unwrap();
        let mut state = LspState::new_with_sdk_locator(Some(config), |_| None);
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 1);
        // Disk collection reparses files, so Arc identity is not a revision key.
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 1);
        std::fs::write(
            &path,
            "void _take(int? value) {} void main() { _take(null); }",
        )
        .unwrap();
        let diagnostics = state.cross_file_diagnostics();
        assert!(
            diagnostics
                .values()
                .flatten()
                .all(|diagnostic| diagnostic.rule != "unnecessary-nullable")
        );
        assert_eq!(state.cross_file_index_build_count, 2);
        let peer = dir.path().join("peer.dart");
        std::fs::write(&peer, "void peer() {}").unwrap();
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 3);
        std::fs::remove_file(&peer).unwrap();
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 4);
        state.cross_file_diagnostics();
        assert_eq!(state.cross_file_index_build_count, 4);
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
                let (files, _) = state.collect_project_files(view);
                assert_eq!(files.len(), 2);
                let file = files.iter().find(|file| file.path == canonical).unwrap();
                assert_eq!(file.source(), buffer);
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
        let alias = workspace.join("alias");
        std::os::unix::fs::symlink(workspace.join("lib"), &alias).unwrap();
        let path = workspace.join("lib/orphan.dart");
        let uri = format!("file://{}", alias.join("orphan.dart").display());
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
        let (files, _) = state.collect_project_files(ProjectView::Semantic);
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
    fn watched_file_normalization_preserves_filesystem_metadata() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("api.dart");
        std::fs::write(&file, "class C {}").unwrap();
        let expected = std::fs::canonicalize(&file).unwrap();
        let normalized = watched_path(&file);
        assert_eq!(normalized.as_os_str(), expected.as_os_str());
        assert!(normalized.is_file());
        std::fs::remove_file(&file).unwrap();
        assert_eq!(watched_path(&file).as_os_str(), expected.as_os_str());
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
        state.config.files.includes.extend_excludes(&[format!(
            "{}/**",
            canonical_or_lexical(&app.join("packages/core")).display()
        )]);
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
        state.external_libraries.libraries().iter().flat_map(|library| library.files.iter())
            .flat_map(|file| file.program().declarations()).any(|declaration| {
                matches!(declaration, falcon_syntax::cst::TopLevelDecl::Class(class) if class.name().name() == name)
            })
    }

    #[test]
    fn watched_workspace_part_refreshes_project_without_resetting_external_facts() {
        let (dir, mut state, uri, _) = dependency_fixture(false);
        let generated = dir.path().join("app/generated.dart");
        std::fs::write(&generated, "part of 'main.dart'; class Before {}").unwrap();
        state.change(
            &uri,
            "import 'package:dep/api.dart'; part 'generated.dart'; void main() {}".into(),
            Some(2),
        );
        state.ensure_semantic_snapshot();
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let resets = state.external_reset_count;
        std::fs::write(&generated, "part of 'main.dart'; class After {}").unwrap();
        let published = state.watched_files_changed(std::slice::from_ref(&generated));
        assert_eq!(published.len(), 1);
        assert_eq!(state.external_reset_count, resets);
        let cache = state.external_indexes.as_ref().unwrap();
        assert!(Arc::ptr_eq(&identities, &cache.identities));
        assert!(Arc::ptr_eq(&types, &cache.types));
        assert!(Arc::ptr_eq(&signatures, &cache.signatures));
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        let generated = canonical_or_lexical(&generated);
        assert!(
            snapshot.files[snapshot.by_path[&generated]]
                .source()
                .contains("class After")
        );
    }

    #[test]
    fn part_uri_typing_updates_watches_without_rebuilding_external_indexes() {
        let (_dir, mut state, uri, _) = dependency_fixture(false);
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let builds = state.external_index_build_count;
        for (version, part) in [(2, "../missing-one.dart"), (3, "../missing-two.dart")] {
            let generation = state.watched_dependency_generation();
            state.change(
                &uri,
                format!("import 'package:dep/api.dart'; part '{part}'; void main() {{}}"),
                Some(version),
            );
            state.ensure_semantic_snapshot();
            let cache = state.external_indexes.as_ref().unwrap();
            assert!(Arc::ptr_eq(&identities, &cache.identities));
            assert!(Arc::ptr_eq(&types, &cache.types));
            assert!(Arc::ptr_eq(&signatures, &cache.signatures));
            assert_eq!(state.external_index_build_count, builds);
            assert!(state.watched_dependency_generation() > generation);
            assert!(
                state
                    .watched_dependency_files()
                    .iter()
                    .any(|path| { path.file_name() == Path::new(part).file_name() })
            );
        }
        assert!(
            state
                .watched_dependency_files()
                .windows(2)
                .all(|paths| paths[0] < paths[1])
        );
        let generation = state.watched_dependency_generation();
        state.change(
            &uri,
            "import 'package:dep/api.dart'; part '../missing-two.dart'; void main() { }".into(),
            Some(4),
        );
        state.ensure_semantic_snapshot();
        assert_eq!(state.watched_dependency_generation(), generation);
        assert_eq!(state.external_index_build_count, builds);
    }

    #[cfg(unix)]
    #[test]
    fn watched_workspace_alias_event_refreshes_unopened_project_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("workspace");
        let alias = dir.path().join("workspace-alias");
        std::fs::create_dir(&root).unwrap();
        std::os::unix::fs::symlink(&root, &alias).unwrap();
        let peer = root.join("peer.dart");
        std::fs::write(&peer, "class Before {}").unwrap();
        let mut state = state(&alias);
        assert_eq!(state.workspace_root, canonical_or_lexical(&root));
        let uri = format!("file://{}", alias.join("main.dart").display());
        state.open(&uri, "void main() {}".into(), Some(1));
        state.ensure_semantic_snapshot();
        let resets = state.external_reset_count;
        std::fs::write(&peer, "class After {}").unwrap();
        let published = state.watched_files_changed(&[alias.join("peer.dart")]);
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, uri);
        assert_eq!(state.external_reset_count, resets);
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        let peer = canonical_or_lexical(&peer);
        assert!(
            snapshot.files[snapshot.by_path[&peer]]
                .source()
                .contains("class After")
        );
    }

    #[cfg(unix)]
    #[test]
    fn watched_local_alias_changes_and_deletions_invalidate_loaded_facts() {
        let (dir, mut state, uri, _) = dependency_fixture(false);
        let local = dir.path().join("local");
        let alias = dir.path().join("alias");
        std::fs::create_dir(&local).unwrap();
        std::os::unix::fs::symlink(&local, &alias).unwrap();
        let file = local.join("api.dart");
        std::fs::write(&file, "class Before {}").unwrap();
        state.change(
            &uri,
            "import '../alias/api.dart'; void main() {}".into(),
            Some(2),
        );
        state.ensure_semantic_snapshot();
        assert!(has_external_class(&state, "Before"));
        let resets = state.external_reset_count;
        std::fs::write(&file, "class After {}").unwrap();
        state.watched_files_changed(&[alias.join("api.dart")]);
        assert_eq!(state.external_reset_count, resets + 1);
        assert!(has_external_class(&state, "After"));
        assert!(!has_external_class(&state, "Before"));
        std::fs::remove_file(&file).unwrap();
        state.watched_files_changed(&[alias.join("api.dart")]);
        assert_eq!(state.external_reset_count, resets + 2);
        assert!(!has_external_class(&state, "After"));
        std::fs::write(&file, "class Recreated {}").unwrap();
        state.watched_files_changed(&[alias.join("api.dart")]);
        assert_eq!(state.external_reset_count, resets + 3);
        assert!(has_external_class(&state, "Recreated"));
    }

    #[test]
    fn watched_companion_absent_from_external_topology_invalidates_external_facts() {
        let (dir, mut state, uri, _) = dependency_fixture(false);
        let companion = dir.path().join("companion.dart");
        std::fs::write(&companion, "part of 'app/main.dart'; class Before {}").unwrap();
        let companion = canonical_or_lexical(&companion);
        state.change(
            &uri,
            "import 'package:dep/api.dart'; part '../companion.dart'; void main() {}".into(),
            Some(2),
        );
        state.ensure_semantic_snapshot();
        assert!(state.loaded_project_paths().any(|path| path == companion));
        assert!(
            !state
                .external_topology
                .as_ref()
                .unwrap()
                .paths
                .iter()
                .any(|(path, _, _)| path == &companion)
        );
        assert!(
            state
                .semantic_snapshot
                .as_ref()
                .unwrap()
                .by_path
                .contains_key(&companion)
        );
        let cache = state.external_indexes.as_ref().unwrap();
        let identities = Arc::clone(&cache.identities);
        let types = Arc::clone(&cache.types);
        let signatures = Arc::clone(&cache.signatures);
        let resets = state.external_reset_count;
        std::fs::write(&companion, "part of 'app/main.dart'; class After {}").unwrap();
        let published = state.watched_files_changed(std::slice::from_ref(&companion));
        assert_eq!(published.len(), 1);
        assert_eq!(published[0].0, uri);
        assert_eq!(state.external_reset_count, resets + 1);
        let cache = state.external_indexes.as_ref().unwrap();
        assert!(!Arc::ptr_eq(&identities, &cache.identities));
        assert!(!Arc::ptr_eq(&types, &cache.types));
        assert!(!Arc::ptr_eq(&signatures, &cache.signatures));
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        let file = &snapshot.files[snapshot.by_path[&companion]];
        assert!(file.source().contains("class After"));
        assert!(!file.source().contains("class Before"));
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
        let dependency_path = canonical_or_lexical(&dependency);
        assert!(
            state
                .watched_dependency_roots()
                .iter()
                .any(|root| dependency_path.starts_with(root))
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
        let target = canonical_or_lexical(&dir.path().join("app")).join("main.dart");
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

    #[cfg(unix)]
    #[test]
    fn missing_workspace_alias_buffer_refreshes_grammar_and_preserves_uri() {
        for semantic in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("workspace");
            let alias = dir.path().join("alias");
            std::fs::create_dir(&root).unwrap();
            std::os::unix::fs::symlink(&root, &alias).unwrap();
            let mut state = state(&alias);
            state.resolve_semantics = semantic;
            let uri = format!("file://{}", alias.join("new_directory/main.dart").display());
            let source = "void f(Object o) { switch (o) { case 1 + 2: break; } }";
            state.open(&uri, source.into(), Some(1));
            let expected = canonical_or_lexical(&root).join("new_directory/main.dart");
            assert_eq!(state.documents[&uri].path, expected);
            assert!(state.documents[&uri].unresolved_path);
            assert!(!expected.exists());
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::CURRENT
            );
            std::fs::create_dir(alias.join(".dart_tool")).unwrap();
            let config = alias.join(".dart_tool/package_config.json");
            std::fs::write(&config,
                r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"","languageVersion":"2.19"}]}"#).unwrap();
            let published = state.watched_files_changed(&[config]);
            assert_eq!(published.len(), 1);
            assert_eq!(published[0].0, uri);
            assert_eq!(state.open_uris(), std::slice::from_ref(&uri));
            assert_eq!(state.documents[&uri].path, expected);
            assert_eq!(
                state.documents[&uri].language_version,
                LanguageVersion::new(2, 19)
            );
            assert_eq!(state.documents[&uri].parse_count, 2);
            state.analyze(&uri);
            assert_eq!(state.documents[&uri].parse_count, 2);
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
        let falcon_syntax::cst::TopLevelDecl::Class(class) =
            file.program().declarations().next().unwrap()
        else {
            panic!("expected dependency class");
        };
        assert_eq!(class.name().name(), "ChangedDependency");
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
    fn divergent_shared_dependency_survives_snapshot_edits_open_close_and_regroup() {
        use falcon_analyze::{ResolvedType, SemanticModel};

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let shared = root.join("shared/lib/bar.dart");
        std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
        let shared_source = "import 'package:foo/foo.dart'; Foo makeValue() => Foo();";
        std::fs::write(&shared, shared_source).unwrap();
        for copy in ["first", "second"] {
            let foo = root.join(format!("deps/{copy}/lib/foo.dart"));
            std::fs::create_dir_all(foo.parent().unwrap()).unwrap();
            std::fs::write(foo, "class Foo {}").unwrap();
        }
        let configure = |app: &str, copy: &str| {
            let directory = root.join(format!("{app}/.dart_tool"));
            std::fs::create_dir_all(&directory).unwrap();
            let path = directory.join("package_config.json");
            std::fs::write(
                &path,
                format!(
                    r#"{{"configVersion":2,"packages":[
                        {{"name":"foo","rootUri":"../../deps/{copy}","packageUri":"lib/"}},
                        {{"name":"bar","rootUri":"../../shared","packageUri":"lib/"}}
                    ]}}"#
                ),
            )
            .unwrap();
            path
        };
        let source = "import 'package:foo/foo.dart'; import 'package:bar/bar.dart'; void main() {}";
        let apps = ["first", "second", "third"].map(|app| {
            configure(app, if app == "first" { "first" } else { "second" });
            let path = root.join(format!("{app}/main.dart"));
            std::fs::write(&path, source).unwrap();
            canonical_or_lexical(&path)
        });
        let uris = apps
            .each_ref()
            .map(|path| format!("file://{}", path.display()));
        let shared = canonical_or_lexical(&shared);
        let shared_uri = format!("file://{}", shared.display());
        let mut state = state(root);
        state.config.files.includes.extend_excludes(&[
            format!("{}/**", canonical_or_lexical(&root.join("deps")).display()),
            format!(
                "{}/**",
                canonical_or_lexical(&root.join("shared")).display()
            ),
        ]);
        for uri in &uris {
            state.open(uri, source.into(), Some(1));
        }
        state.ensure_semantic_snapshot();

        let assert_aliases = |state: &LspState, copies: [&str; 3]| {
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            let indexes = &snapshot.indexes;
            let functions = apps.each_ref().map(|app| {
                let model = SemanticModel::new(
                    app,
                    &snapshot.files[snapshot.by_path[app]].parsed,
                    &indexes.identities,
                    Some(&indexes.types),
                );
                let foo = model.resolve_name("Foo").unwrap();
                let function = model.resolve_value("makeValue").unwrap();
                assert!(foo.is_package("foo", "Foo"));
                assert!(function.is_package("bar", "makeValue"));
                assert_eq!(
                    indexes.signatures.function(&function).unwrap().return_type,
                    ResolvedType::interface_identity(foo)
                );
                function
            });
            for (app, copy) in apps.iter().zip(copies) {
                let model = SemanticModel::new(
                    app,
                    &snapshot.files[snapshot.by_path[app]].parsed,
                    &indexes.identities,
                    Some(&indexes.types),
                );
                let foo = model.resolve_name("Foo").unwrap();
                let expected =
                    canonical_or_lexical(&root.join(format!("deps/{copy}/lib/foo.dart")));
                assert_eq!(
                    indexes.identities.declaration_library(&foo),
                    Some(
                        indexes
                            .identities
                            .library(indexes.identities.file(&expected).unwrap())
                    )
                );
            }
            assert_ne!(functions[0], functions[1]);
            if copies[2] == copies[0] {
                assert_eq!(functions[2], functions[0]);
            } else {
                assert_eq!(functions[2], functions[1]);
            }
        };
        assert_aliases(&state, ["first", "second", "second"]);

        let assert_reused_after_edit = |state: &mut LspState, version| {
            let topology = state.external_topology.as_ref().unwrap().paths.as_ptr();
            let cache = state.external_indexes.as_ref().unwrap();
            let identities = Arc::clone(&cache.identities);
            let types = Arc::clone(&cache.types);
            let signatures = Arc::clone(&cache.signatures);
            let builds = state.external_index_build_count;
            assert!(state.change(&uris[0], format!("{source} // {version}"), Some(version)));
            state.analyze(&uris[0]);
            assert_eq!(
                topology,
                state.external_topology.as_ref().unwrap().paths.as_ptr()
            );
            let cache = state.external_indexes.as_ref().unwrap();
            assert!(Arc::ptr_eq(&identities, &cache.identities));
            assert!(Arc::ptr_eq(&types, &cache.types));
            assert!(Arc::ptr_eq(&signatures, &cache.signatures));
            assert_eq!(state.external_index_build_count, builds);
        };
        assert_reused_after_edit(&mut state, 2);
        assert_aliases(&state, ["first", "second", "second"]);

        let buffered = "class Buffered {} Buffered makeValue() => Buffered();";
        state.open(&shared_uri, buffered.into(), Some(1));
        for version in [1, 2] {
            if version == 2 {
                assert!(state.change(&shared_uri, format!("{buffered} // edited"), Some(version)));
            }
            state.ensure_semantic_snapshot();
            assert!(
                state
                    .external_libraries
                    .libraries()
                    .iter()
                    .all(|library| { library.files.iter().all(|file| file.path != shared) })
            );
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            let indexes = &snapshot.indexes;
            let owner = SemanticModel::new(
                &shared,
                &snapshot.files[snapshot.by_path[&shared]].parsed,
                &indexes.identities,
                Some(&indexes.types),
            );
            let buffered = owner.resolve_name("Buffered").unwrap();
            let function = owner.resolve_value("makeValue").unwrap();
            for app in &apps {
                let model = SemanticModel::new(
                    app,
                    &snapshot.files[snapshot.by_path[app]].parsed,
                    &indexes.identities,
                    Some(&indexes.types),
                );
                assert_eq!(model.resolve_name("Buffered"), Some(buffered.clone()));
                assert_eq!(model.resolve_value("makeValue"), Some(function.clone()));
                assert_eq!(
                    indexes.signatures.function(&function).unwrap().return_type,
                    ResolvedType::interface_identity(buffered.clone())
                );
            }
        }
        state.close(&shared_uri);
        state.ensure_semantic_snapshot();
        assert_aliases(&state, ["first", "second", "second"]);
        assert_reused_after_edit(&mut state, 3);

        let configuration = configure("third", "first");
        state.watched_files_changed(&[configuration]);
        state.ensure_semantic_snapshot();
        assert_aliases(&state, ["first", "second", "first"]);
        assert_reused_after_edit(&mut state, 4);
        assert_aliases(&state, ["first", "second", "first"]);

        for uri in &uris {
            state.close(uri);
        }
        configure("third", "second");
        let foo_only = "import 'package:foo/foo.dart';";
        for (app, text) in apps.iter().zip([foo_only, foo_only, source]) {
            std::fs::write(app, text).unwrap();
        }
        std::fs::write(
            shared.with_file_name("other.dart"),
            "import 'package:foo/foo.dart'; Foo otherValue() => Foo();",
        )
        .unwrap();
        state.reset_external_environment();
        state.refresh_environment(apps.iter().map(PathBuf::as_path));
        state.open(&uris[2], source.into(), Some(1));
        let first_source =
            format!("{foo_only} import 'package:bar/bar.dart'; import 'package:bar/other.dart';");
        state.open(&uris[0], first_source, Some(1));
        let second_source = format!("{foo_only} import 'package:bar/other.dart';");
        state.open(&uris[1], second_source.clone(), Some(1));
        state.ensure_semantic_snapshot();
        let primary_function = |state: &LspState| {
            let snapshot = state.semantic_snapshot.as_ref().unwrap();
            let indexes = &snapshot.indexes;
            SemanticModel::new(
                &apps[2],
                &snapshot.files[snapshot.by_path[&apps[2]]].parsed,
                &indexes.identities,
                Some(&indexes.types),
            )
            .resolve_value("makeValue")
            .unwrap()
        };
        let before = primary_function(&state);
        let builds = state.external_index_build_count;
        let library_count = state.external_libraries.libraries().len();
        assert!(state.change(
            &uris[1],
            format!("{second_source} import 'package:bar/bar.dart';"),
            Some(2),
        ));
        state.ensure_semantic_snapshot();
        assert_eq!(state.external_libraries.libraries().len(), library_count);
        assert!(state.external_index_build_count > builds);
        let after = primary_function(&state);
        assert_ne!(before, after);
        let fresh = IdentityIndex::build_external(ExternalSources {
            libraries: &state.external_libraries,
            environment: &state.environment,
        });
        let shared_file = state
            .external_libraries
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .find(|file| file.path == shared)
            .unwrap();
        assert_eq!(
            after,
            SemanticModel::new(&shared, &shared_file.parsed, &fresh, None)
                .resolve_value("makeValue")
                .unwrap()
        );
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
            let defining = canonical_or_lexical(&defining);
            let part = canonical_or_lexical(&part);
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
            assert_eq!(parts[0].source(), buffered);
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
