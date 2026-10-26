//! LSP server state: document cache, cached syntax trees, config-driven rule set.
//!
//! Text changes invalidate a document's tree; config changes rerun rules over
//! cached trees, reparsing only when the effective language version changes.
//! Unresolved filesystem identities are retried before analysis when files materialize.

use std::collections::{HashMap, HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use lsp_types::{MessageType, ShowMessageParams, Uri};

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, CrossFileRuleRegistry, ExternalIndexParts,
    ExternalLibraries, ExternalOrigin, ExternalSources, ExternalTarget, IdentityIndex,
    IdentitySource, LibraryGrouping, PackageIdentity, ParsedFile, ProjectFile, RuleRegistry,
    SemanticIndexes, SignatureIndex, TypeIndex, build_semantic_indexes, canonical_or_lexical,
    import_path, library_unit, package_identities, syntax_error_diagnostics,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::try_parse_for_package;
use falcon_diagnostics::Diagnostic;
use falcon_project::{
    DiscoveryWatch, ProjectAnalysis, analyze_document, cross_file_diagnostics,
    discover_project_with_watches, load_project_config_with_errors, manifest_file, resolve_document_positions,
    sort_diagnostics,
};
use falcon_syntax::{
    LanguageVersion,
    cst::{FunctionDecl, GetterDecl, MethodDecl, NodeKind, OperatorDecl, Program},
};
use tracing::{debug, warn};

pub(crate) mod code_actions;

struct DocumentParse {
    tree: falcon_syntax::cst::SyntaxTree,
    diagnostics: Vec<Diagnostic>,
    has_errors: bool,
    worker_failed: bool,
}

fn declaration_signature(parsed: &ParsedFile) -> u64 {
    let mut signature = DefaultHasher::new();
    for uri in falcon_analyze::doc_imports(parsed.program(), &parsed.source) {
        uri.hash(&mut signature);
    }
    let mut cursor = 0;
    let mut body_start = 0;
    let mut body_end = 0;
    for node in parsed.program().syntax().descendants() {
        if node.span().start >= body_start && node.span().start < body_end {
            continue;
        }
        if node.kind() == NodeKind::TypeParam {
            node.span().start.hash(&mut signature);
        }
        let body = match node.kind() {
            NodeKind::FunctionDecl => FunctionDecl::cast(node)
                .filter(|function| function.return_type().is_some())
                .and_then(|function| function.body()),
            NodeKind::MethodDecl => MethodDecl::cast(node)
                .filter(|method| method.return_type().is_some())
                .and_then(|method| method.body()),
            NodeKind::GetterDecl => GetterDecl::cast(node)
                .filter(|getter| getter.return_type().is_some())
                .and_then(|getter| getter.body()),
            NodeKind::OperatorDecl => OperatorDecl::cast(node)
                .filter(|operator| operator.return_type().is_some())
                .and_then(|operator| operator.body()),
            _ => None,
        };
        if let Some(body) = body {
            let span = body.span();
            parsed.source[cursor..span.start].hash(&mut signature);
            cursor = span.end;
            body_start = span.start;
            body_end = span.end;
        }
    }
    parsed.source[cursor..].hash(&mut signature);
    signature.finish()
}

fn document_parse_result(
    path: &Path,
    result: std::io::Result<falcon_dart_parser::ParseResult>,
) -> DocumentParse {
    match result {
        Ok(parsed) => DocumentParse {
            diagnostics: syntax_error_diagnostics(path, &parsed.errors),
            has_errors: !parsed.errors.is_empty(),
            tree: parsed.tree,
            worker_failed: false,
        },
        Err(error) => DocumentParse {
            tree: falcon_syntax::cst::SyntaxTree::build("", &[], Vec::new()),
            diagnostics: vec![falcon_analyze::rules_thread_diagnostic(path, &error)],
            has_errors: true,
            worker_failed: true,
        },
    }
}

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
    /// Rejected buffers keep no usable syntax and publish only ingestion diagnostics.
    source_rejected: bool,
    /// Parse and worker diagnostics from the last parse, kept so `analyze` can
    /// publish them alongside lints without re-parsing (parity with the CLI).
    syntax_diagnostics: Vec<Diagnostic>,
    /// Most recent published output (byte spans) — read by hover. May include
    /// cross-file diagnostics after a cross-file pass; never an input to analysis.
    pub last_diagnostics: Vec<Diagnostic>,
    /// Parse revision represented by the cached diagnostics.
    diagnostics_parse_count: u64,
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

struct CachedDiskFile {
    parsed: Arc<ParsedFile>,
    has_parse_errors: bool,
    language_version: Option<LanguageVersion>,
}

struct RootConfig {
    config: FalconConfig,
    analysis: ProjectAnalysis,
}

/// Server-side cache: open documents, active config, enabled rule set.
pub struct LspState {
    documents: HashMap<String, DocumentState>,
    code_action_resolve: bool,
    workspace_document_changes: bool,
    config: FalconConfig,
    additional_root_configs: Vec<RootConfig>,
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
    /// Primary root used for configuration discovery and unresolved document paths.
    workspace_root: PathBuf,
    /// Client roots override the config-derived fallback for project discovery.
    workspace_roots: Vec<PathBuf>,
    workspace_initialized: bool,
    /// Whether any enabled per-file or cross-file rule consumes resolver-backed semantic facts.
    resolve_semantics: bool,
    /// Lazily rebuilt after document, config, or watched-workspace changes.
    semantic_snapshot: Option<SemanticSnapshot>,
    /// A failed project parse or semantic build invalidates the complete view until its inputs change.
    semantic_snapshot_error: Option<std::io::Error>,
    #[cfg(test)]
    parser_failure_path: Option<PathBuf>,
    #[cfg(test)]
    build_panic: bool,
    /// Dependents from the graph before an edit, retained until that edit is analyzed.
    pending_semantic_affected: HashMap<String, HashSet<String>>,
    body_only_changes: HashSet<String>,
    /// Instrumentation for incremental tests: number of semantic snapshot builds.
    semantic_snapshot_build_count: u64,
    /// Instrumentation for incremental tests: number of dependency topology builds.
    semantic_topology_build_count: u64,
    /// Package configurations, language versions and SDKs of analyzed files.
    environment: AnalysisEnvironment,
    /// Resolution inputs captured on discovery or watched configuration events,
    /// never polled on ordinary project edits.
    environment_stamps: Option<falcon_analyze::EnvironmentCacheKey>,
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
    disk_parse_count: u64,
    /// Cache-miss source reads, including the bounded reader's file metadata check.
    disk_read_count: u64,
    workspace_walk_count: u64,
    disk_files: HashMap<PathBuf, CachedDiskFile>,
    project_paths: Option<Vec<(PathBuf, bool)>>,
    project_path_patterns: Vec<String>,
    project_discovery_watches: Vec<DiscoveryWatch>,
}

impl LspState {
    /// Create state with config from `config_path`, or discovery from the
    /// current directory when `None` (same order as the CLI: cwd → git root →
    /// `$HOME/.falcon.json` → defaults).
    pub fn new(config_path: Option<PathBuf>) -> Self {
        let start_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new_from_environment(config_path, AnalysisEnvironment::default(), start_dir)
    }

    /// Create state with explicit SDK discovery, allowing hermetic fixtures.
    pub fn new_with_sdk_locator(
        config_path: Option<PathBuf>,
        locator: fn(Option<&falcon_analyze::PackageConfig>) -> Option<falcon_analyze::DartSdk>,
    ) -> Self {
        let start_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self::new_from(config_path, locator, start_dir)
    }

    fn new_from(
        config_path: Option<PathBuf>,
        locator: fn(Option<&falcon_analyze::PackageConfig>) -> Option<falcon_analyze::DartSdk>,
        config_start_dir: PathBuf,
    ) -> Self {
        Self::new_from_environment(
            config_path,
            AnalysisEnvironment::with_sdk_locator(locator),
            config_start_dir,
        )
    }

    fn new_from_environment(
        config_path: Option<PathBuf>,
        environment: AnalysisEnvironment,
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
        let analysis = ProjectAnalysis::new(&config);
        let rules = analysis.registry;
        let cross_file_rules = analysis.cross_file_registry;
        let cross_file_resolve_semantics = analysis.cross_file_resolve;
        let resolve_semantics = analysis.resolve;
        Self {
            documents: HashMap::new(),
            code_action_resolve: false,
            workspace_document_changes: false,
            config,
            additional_root_configs: Vec::new(),
            config_path,
            pending_config_messages,
            last_config_messages,
            config_start_dir,
            rules,
            cross_file_rules,
            cross_file_resolve_semantics,
            cross_file_index_cache: None,
            cross_file_index_build_count: 0,
            workspace_roots: vec![workspace_root.clone()],
            workspace_initialized: false,
            workspace_root,
            resolve_semantics,
            semantic_snapshot: None,
            semantic_snapshot_error: None,
            #[cfg(test)]
            parser_failure_path: None,
            #[cfg(test)]
            build_panic: false,
            pending_semantic_affected: HashMap::new(),
            body_only_changes: HashSet::new(),
            semantic_snapshot_build_count: 0,
            semantic_topology_build_count: 0,
            environment,
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
            disk_parse_count: 0,
            disk_read_count: 0,
            workspace_walk_count: 0,
            disk_files: HashMap::new(),
            project_paths: None,
            project_path_patterns: Vec::new(),
            project_discovery_watches: Vec::new(),
        }
    }

    /// Apply initialize roots before opening documents, resolving each folder's config.
    pub(crate) fn initialize_workspace_roots(&mut self, roots: Vec<PathBuf>) {
        let mut roots = roots
            .into_iter()
            .map(|root| canonical_or_lexical(&root))
            .collect::<Vec<_>>();
        let mut seen = HashSet::new();
        roots.retain(|root| seen.insert(root.clone()));
        let Some(primary) = roots.first().cloned() else {
            return;
        };
        let environment = std::mem::take(&mut self.environment);
        *self = Self::new_from_environment(self.config_path.clone(), environment, primary.clone());
        self.workspace_root = primary;
        self.workspace_roots = roots;
        self.workspace_initialized = true;
        let messages = self.reload_additional_root_configs();
        for message in messages {
            if !self.pending_config_messages.iter().any(|existing| {
                existing.typ == message.typ && existing.message == message.message
            }) {
                self.pending_config_messages.push(message);
            }
        }
        self.last_config_messages = self
            .pending_config_messages
            .iter()
            .map(|message| (message.typ, message.message.clone()))
            .collect();
    }

    fn reload_additional_root_configs(&mut self) -> Vec<ShowMessageParams> {
        let mut messages = Vec::new();
        self.additional_root_configs.reserve(
            self.workspace_roots.len().saturating_sub(1 + self.additional_root_configs.len()),
        );
        for (index, root) in self.workspace_roots.iter().skip(1).enumerate() {
            let (config, config_root, root_messages) = load_from(self.config_path.as_deref(), root);
            messages.extend(root_messages);
            let config = config.or_else(|| {
                self.additional_root_configs.get(index).map(|entry| entry.config.clone())
            }).unwrap_or_else(|| {
                let mut config = FalconConfig::default();
                config.resolve_paths(&config_root);
                config
            });
            let analysis = ProjectAnalysis::new(&config);
            self.resolve_semantics |= analysis.resolve;
            self.cross_file_resolve_semantics |= analysis.cross_file_resolve;
            let entry = RootConfig { config, analysis };
            if let Some(existing) = self.additional_root_configs.get_mut(index) {
                *existing = entry;
            } else {
                self.additional_root_configs.push(entry);
            }
        }
        messages
    }

    fn root_index(&self, path: &Path) -> usize {
        self.workspace_roots.iter().enumerate()
            .filter(|(_, root)| path.starts_with(root))
            .max_by_key(|(_, root)| root.components().count())
            .map_or(0, |(index, _)| index)
    }

    fn config_for_root(&self, index: usize) -> &FalconConfig {
        if index == 0 { &self.config } else { &self.additional_root_configs[index - 1].config }
    }

    fn config_for_path(&self, path: &Path) -> &FalconConfig {
        self.config_for_root(self.root_index(path))
    }

    /// Configuration warnings and errors awaiting delivery to the editor.
    pub fn take_config_messages(&mut self) -> Vec<ShowMessageParams> {
        self.pending_config_messages
            .extend(self.environment.take_warnings().into_iter().map(|message| {
                ShowMessageParams {
                    typ: MessageType::WARNING,
                    message,
                }
            }));
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

    pub(crate) fn ancestor_manifests(&self) -> impl Iterator<Item = PathBuf> + '_ {
        self.workspace_roots
            .iter()
            .enumerate()
            .flat_map(|(index, root)| {
                root.ancestors().skip(1)
                    .map(|directory| directory.join("pubspec.yaml"))
                    .filter(move |path| !self.config_for_root(index).files.includes.excludes(&path.to_string_lossy()))
            })
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
        // Discovery can capture new disk bytes while retaining a known context.
        // Check original link locations before watcher normalization follows them.
        let inputs_changed = paths.iter().any(|path| {
            self.environment
                .inputs_affected_by(self.environment_stamps.as_ref(), path)
        });
        let paths: Vec<_> = paths.iter().map(|path| watched_path(path)).collect();
        if paths.iter().any(|path| {
            self.workspace_roots
                .iter()
                .any(|root| path.starts_with(root))
                && !(path.is_file()
                    && self.project_paths.as_ref().is_some_and(|files| {
                        files
                            .binary_search_by(|(candidate, _)| candidate.cmp(path))
                            .is_ok()
                    }))
        }) {
            self.project_paths = None;
        }
        self.disk_files
            .retain(|cached, _| !paths.iter().any(|path| cached.starts_with(path)));
        let rules_changed = paths.iter().any(|path| {
            matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("falcon.json" | ".falcon.json" | "melos.yaml" | "pubspec.yaml")
            )
        });
        // A delivered input event must invalidate even an equal key.
        let inputs_changed = inputs_changed
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
        let project_changed = paths.iter().any(|path| {
            !open(path)
                && (self
                    .workspace_roots
                    .iter()
                    .any(|root| path.starts_with(root))
                    || self.ancestor_manifests().any(|manifest| manifest == *path))
        });
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
            let refreshed = self.reload_config();
            if !refreshed.is_empty()
                || !paths.iter().any(|path| {
                    matches!(
                        path.file_name().and_then(|name| name.to_str()),
                        Some("pubspec.yaml" | "melos.yaml")
                    )
                })
            {
                return refreshed;
            }
            // A malformed rules config must not suppress manifest-driven
            // analysis: retain the last good rules and refresh open documents.
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
        self.reset_external_layer();
    }

    fn reset_external_layer(&mut self) {
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
            let parsed = document_parse_result(
                &doc.path,
                try_parse_for_package(doc.text(), Some(language_version)),
            );
            let tree = parsed.tree;
            if let Some(parsed) = Arc::get_mut(&mut doc.parsed) {
                parsed.tree = tree;
            } else {
                doc.parsed = Arc::new(ParsedFile {
                    source: doc.text().into(),
                    tree,
                });
            }
            doc.has_parse_errors = parsed.has_errors;
            doc.source_rejected = parsed.worker_failed;
            doc.syntax_diagnostics = parsed.diagnostics;
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
        let (path, unresolved_path) = document_path(uri, &self.workspace_roots);
        let source_error = reject_oversized_buffer(&path, &mut text);
        self.pending_semantic_affected.remove(uri);
        if self.resolve_semantics && !self.documents.is_empty() {
            self.ensure_semantic_snapshot();
        }
        self.refresh_environment(std::iter::once(path.as_path()));
        let package_version = self.environment.package_language_version(&path);
        let result = try_parse_for_package(&text, package_version);
        #[cfg(test)]
        let result = self.inject_parser_failure(&path, result);
        let parsed = document_parse_result(&path, result);
        let tree = parsed.tree;
        let language_version = self.environment.language_version(&path, &text);
        let mut syntax_diagnostics = parsed.diagnostics;
        let source_rejected = source_error.is_some() || parsed.worker_failed;
        syntax_diagnostics.extend(source_error);
        let next_parsed = Arc::new(ParsedFile {
            source: text.into_boxed_str(),
            tree,
        });
        let unchanged_api = !source_rejected
            && !parsed.has_errors
            && self.semantic_snapshot.as_ref().is_some_and(|snapshot| {
                snapshot.by_path.get(&path).is_some_and(|&index| {
                    let previous = &snapshot.files[index];
                    !previous.has_parse_errors
                        && declaration_signature(&previous.parsed)
                            == declaration_signature(&next_parsed)
                })
            });
        self.documents.insert(
            uri.to_string(),
            DocumentState {
                path,
                unresolved_path,
                parsed: next_parsed,
                version,
                language_version,
                has_parse_errors: source_rejected || parsed.has_errors,
                source_rejected,
                syntax_diagnostics,
                last_diagnostics: Vec::new(),
                diagnostics_parse_count: 0,
                had_cross_file_diags: false,
                parse_count: 1,
                analyze_count: 0,
            },
        );
        self.update_document_snapshot(uri, unchanged_api);
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

        self.refresh_unresolved_paths();
        let path = self.documents[uri].path.clone();
        let source_error = reject_oversized_buffer(&path, &mut text);
        self.refresh_environment(std::iter::once(path.as_path()));
        let package_version = self.environment.package_language_version(&path);
        let result = try_parse_for_package(&text, package_version);
        #[cfg(test)]
        let result = self.inject_parser_failure(&path, result);
        let parsed = document_parse_result(&path, result);
        let doc = self.documents.get_mut(uri).expect("document checked above");
        let tree = parsed.tree;
        let next_parsed = Arc::new(ParsedFile {
            source: text.into_boxed_str(),
            tree,
        });
        let unchanged_api = !doc.has_parse_errors
            && !doc.source_rejected
            && source_error.is_none()
            && !parsed.has_errors
            && !parsed.worker_failed
            && declaration_signature(&doc.parsed) == declaration_signature(&next_parsed);
        doc.syntax_diagnostics = parsed.diagnostics;
        doc.source_rejected = source_error.is_some() || parsed.worker_failed;
        doc.syntax_diagnostics.extend(source_error);
        doc.parsed = next_parsed;
        doc.version = version;
        doc.language_version = self.environment.language_version(&doc.path, doc.text());
        doc.has_parse_errors = doc.source_rejected || parsed.has_errors;
        doc.parse_count += 1;
        self.update_document_snapshot(uri, unchanged_api);
        true
    }

    /// `textDocument/didSave`: refresh filesystem identity when resolvable and
    /// any supplied text (re-parsing only when it actually differs), then analyze.
    pub fn save(&mut self, uri: &str, text: Option<String>) -> Vec<Diagnostic> {
        let saved_path = watched_path(&uri_to_path(uri));
        self.disk_files.remove(&saved_path);
        if !self.project_paths.as_ref().is_some_and(|files| {
            files
                .binary_search_by(|(candidate, _)| candidate.cmp(&saved_path))
                .is_ok()
        }) {
            self.project_paths = None;
        }
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

    /// `textDocument/didClose`: restore the disk revision before dropping the buffer.
    pub fn close(&mut self, uri: &str) {
        self.pending_semantic_affected.remove(uri);
        let Some(document) = self.documents.get(uri) else {
            return;
        };
        let path = document.path.clone();
        if self.semantic_snapshot.is_none() {
            self.disk_files.remove(&path);
            self.documents.remove(uri);
            self.invalidate_semantic_snapshot();
            return;
        }
        let buffer = Arc::clone(&document.parsed);
        let buffer_has_parse_errors = document.has_parse_errors;
        let buffer_rejected = document.source_rejected;
        let language_version = self.environment.package_language_version(&path);
        // Read only this file: an unsaved buffer is never evidence of disk contents.
        self.disk_read_count += 1;
        let disk = falcon_analyze::read_source(&path).ok().and_then(|source| {
            if let Some(cached) = self.disk_files.get(&path)
                && cached.language_version == language_version
                && cached.parsed.source.as_ref() == source
            {
                return Some((Arc::clone(&cached.parsed), cached.has_parse_errors));
            }
            if !buffer_rejected && buffer.source.as_ref() == source {
                return Some((buffer, buffer_has_parse_errors));
            }
            self.disk_parse_count += 1;
            let result = try_parse_for_package(&source, language_version);
            #[cfg(test)]
            let result = self.inject_parser_failure(&path, result);
            result.ok().map(|parsed| {
                (
                    Arc::new(ParsedFile {
                        source: source.into_boxed_str(),
                        tree: parsed.tree,
                    }),
                    !parsed.errors.is_empty(),
                )
            })
        });
        if let Some((parsed, has_parse_errors)) = disk {
            let document = self.documents.get_mut(uri).expect("open document");
            let unchanged_api = !document.source_rejected
                && !document.has_parse_errors
                && !has_parse_errors
                && declaration_signature(&document.parsed) == declaration_signature(&parsed);
            document.parsed = Arc::clone(&parsed);
            document.has_parse_errors = has_parse_errors;
            self.disk_files.insert(
                path,
                CachedDiskFile {
                    parsed,
                    has_parse_errors,
                    language_version,
                },
            );
            self.update_document_snapshot(uri, unchanged_api);
        } else {
            self.disk_files.remove(&path);
            self.update_document_snapshot(uri, false);
        }
        self.documents.remove(uri);
        self.body_only_changes.remove(uri);
    }

    /// Run enabled rules over the cached CST, in shared project diagnostic order.
    pub fn analyze(&mut self, uri: &str) -> Vec<Diagnostic> {
        if !self.documents.contains_key(uri) {
            return Vec::new();
        }
        let doc = self.documents.get_mut(uri).expect("open document");
        doc.diagnostics_parse_count = doc.parse_count;
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
        let root_index = self.root_index(&file_path);
        let (config, rules) = if root_index == 0 {
            (&self.config, &self.rules)
        } else {
            let entry = &self.additional_root_configs[root_index - 1];
            (&entry.config, &entry.analysis.registry)
        };
        let Some(doc) = self.documents.get_mut(uri) else {
            return Vec::new();
        };
        let mut ctx = AnalyzeContext::new(&doc.path, config, &doc.parsed)
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
        let mut diagnostics = analyze_document(
            rules,
            config,
            &ctx,
            doc.syntax_diagnostics.clone(),
        );
        if let Some(error) = &self.semantic_snapshot_error {
            diagnostics.push(falcon_analyze::rules_thread_diagnostic(&doc.path, error));
        }
        resolve_document_positions(&mut diagnostics, doc.text());
        sort_diagnostics(&mut diagnostics);
        doc.analyze_count += 1;
        doc.diagnostics_parse_count = doc.parse_count;
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
        if self.body_only_changes.remove(uri)
            && !self.pending_semantic_affected.contains_key(uri)
            && self.semantic_snapshot.is_some()
        {
            return vec![uri.to_string()];
        }
        if !self.resolve_semantics {
            return if self.documents.contains_key(uri) {
                vec![uri.to_string()]
            } else {
                Vec::new()
            };
        }
        self.ensure_semantic_snapshot();
        let changed_path = self.documents.get(uri).map_or_else(
            || document_path(uri, &self.workspace_roots).0,
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
        let (config, workspace_root, mut messages) =
            load_from(self.config_path.as_deref(), &self.config_start_dir);
        messages.extend(self.reload_additional_root_configs());
        let mut seen = Vec::with_capacity(messages.len());
        messages.retain(|message| {
            let fingerprint = (message.typ, message.message.clone());
            if seen.contains(&fingerprint) {
                false
            } else {
                seen.push(fingerprint);
                true
            }
        });
        let fingerprints: Vec<_> = messages
            .iter()
            .map(|message| (message.typ, message.message.clone()))
            .collect();
        if fingerprints != self.last_config_messages {
            self.pending_config_messages.extend(messages);
            self.last_config_messages = fingerprints;
        }
        let config = match config {
            Some(config) => config,
            None if self.additional_root_configs.is_empty() => return Vec::new(),
            None => self.config.clone(),
        };
        if !self.workspace_initialized {
            self.workspace_roots = vec![workspace_root.clone()];
            self.workspace_root = workspace_root;
        }
        self.config = config;
        self.project_paths = None;
        self.watched_dependency_generation += 1;
        let analysis = ProjectAnalysis::new(&self.config);
        self.rules = analysis.registry;
        self.cross_file_resolve_semantics = analysis.cross_file_resolve;
        self.resolve_semantics = analysis.resolve;
        self.cross_file_rules = analysis.cross_file_registry;
        self.resolve_semantics |= self.additional_root_configs.iter().any(|entry| entry.analysis.resolve);
        self.cross_file_resolve_semantics |= self.additional_root_configs.iter().any(|entry| entry.analysis.cross_file_resolve);
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
            || self.additional_root_configs.iter().any(|entry| !entry.analysis.cross_file_registry.is_empty())
    }

    /// Run the cross-file rules over the whole workspace and republish the
    /// merged (per-file + cross-file) diagnostics for every open document whose
    /// cross-file set changed. Returns the `(uri, merged)` pairs actually
    /// published; pending semantic dependent refreshes remain the caller's job.
    ///
    /// Only open docs are republished: an editor shows diagnostics for open
    /// buffers, and republishing unchanged docs would be redundant traffic.
    pub fn cross_file_pass(&mut self) -> Vec<(String, Vec<Diagnostic>)> {
        if !self.cross_file_rules_enabled() {
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
            // Project worker failures belong to per-file publication, never both maps.
            if !has_now && !had_before && self.semantic_snapshot_error.is_none() {
                continue;
            }
            let mut merged = self.analyze(&uri);
            if let Some(diags) = cross_file_diags {
                let cross_file_start = merged.len();
                merged.extend(diags.iter().cloned());
                resolve_document_positions(
                    &mut merged[cross_file_start..],
                    self.documents[&uri].text(),
                );
                sort_diagnostics(&mut merged);
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
        if !self.cross_file_rules_enabled()
            || self.documents.values().any(|doc| doc.source_rejected)
        {
            return HashMap::new();
        }
        if self.resolve_semantics && !self.documents.is_empty() {
            self.ensure_semantic_snapshot();
        }
        if self.semantic_snapshot_error.is_some() {
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
            // Dependency waves would otherwise parse on Rayon's global small-stack workers.
            if let Err(error) =
                falcon_dart_parser::with_stack(|| self.ensure_cross_file_indexes(&files))
            {
                warn!("cross-file index worker failed: {error}");
                // Per-file publication carries the failure until inputs change, without reloading.
                self.semantic_snapshot = None;
                self.semantic_snapshot_error = Some(error);
                return HashMap::new();
            }
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
        let result = (0..self.workspace_roots.len()).try_fold(Vec::new(), |mut all, index| {
            let (config, rules) = if index == 0 {
                (&self.config, &self.cross_file_rules)
            } else {
                let entry = &self.additional_root_configs[index - 1];
                (&entry.config, &entry.analysis.cross_file_registry)
            };
            if !rules.is_empty() {
                let diagnostics = cross_file_diagnostics(
                    &self.environment, rules, &files, config, indexes,
                )?;
                all.extend(diagnostics.into_iter().filter(|diagnostic| {
                    self.root_index(Path::new(&diagnostic.file_path)) == index
                }));
            }
            Ok::<_, std::io::Error>(all)
        });
        let diags = match result {
            Ok(diags) => diags,
            Err(error) => {
                // Editors only show open buffers, so every open document carries the failure.
                warn!("cross-file rules worker failed: {error}");
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
        #[cfg(test)]
        assert!(!self.build_panic, "semantic build regression");
        let dart_files = || files.iter().filter(|file| is_dart_file(&file.path));
        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        if self.cross_file_index_cache.as_ref().is_some_and(|cache| {
            dart_files().count() == cache.files.len()
                && same_packages(&packages, &cache.packages)
                && dart_files().zip(&cache.files).all(|(file, cached)| {
                    file.path == cached.path
                        && (same_revision(file, cached)
                            || (!file.has_parse_errors
                                && !cached.has_parse_errors
                                && declaration_signature(&file.parsed)
                                    == declaration_signature(&cached.parsed)
                                && cache.indexes.identities.file(&file.path).is_some()))
                })
        }) {
            let cache = self.cross_file_index_cache.as_mut().expect("cache checked");
            for (file, cached) in dart_files().zip(&mut cache.files) {
                if same_revision(file, cached) {
                    continue;
                }
                let id = cache
                    .indexes
                    .identities
                    .file(&file.path)
                    .expect("identity checked");
                cache
                    .indexes
                    .signatures
                    .refresh_file_body(id, file.program());
                cached.parsed = Arc::clone(&file.parsed);
                cached.has_parse_errors = file.has_parse_errors;
            }
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
                uris.extend(falcon_analyze::doc_imports(program, file.source()).map(str::to_owned));
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
        let identity_sources: Vec<_> = files
            .iter()
            .map(|file| IdentitySource {
                path: &file.path,
                parsed: &file.parsed,
                has_parse_errors: file.has_parse_errors,
            })
            .collect();
        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        let (indexes, grouping) = build_semantic_indexes(
            &identity_sources,
            &packages,
            Some(ExternalSources {
                libraries: external,
                environment: &self.environment,
            }),
            cache.map(|cache| ExternalIndexParts {
                identities: &cache.identities,
                types: &cache.types,
                signatures: &cache.signatures,
            }),
        );
        (indexes, grouping, packages)
    }

    fn invalidate_semantic_snapshot(&mut self) {
        self.semantic_snapshot = None;
        self.semantic_snapshot_error = None;
        self.body_only_changes.clear();
    }

    fn update_document_snapshot(&mut self, uri: &str, unchanged_api: bool) {
        let doc = &self.documents[uri];
        if unchanged_api
            && let Some(snapshot) = self.semantic_snapshot.as_mut()
            && let Some(&index) = snapshot.by_path.get(&doc.path)
            && let Some(file) = snapshot.indexes.identities.file(&doc.path)
        {
            snapshot
                .indexes
                .signatures
                .refresh_file_body(file, doc.parsed.program());
            snapshot.files[index].parsed = Arc::clone(&doc.parsed);
            snapshot.files[index].has_parse_errors = doc.has_parse_errors;
            self.body_only_changes.insert(uri.to_string());
            return;
        }
        if let Some(snapshot) = self.semantic_snapshot.as_ref() {
            self.pending_semantic_affected
                .entry(uri.to_string())
                .or_default()
                .extend(affected_open_uris_in_snapshot(
                    snapshot,
                    &doc.path,
                    &self.documents,
                ));
        }
        self.invalidate_semantic_snapshot();
    }

    fn record_semantic_worker_failure(&mut self, path: &Path, error: std::io::Error) {
        self.semantic_snapshot = None;
        self.semantic_snapshot_error = Some(error);
        if !self
            .watched_local_sources
            .iter()
            .any(|source| source == path)
        {
            self.watched_local_sources.push(path.to_path_buf());
            self.watched_local_sources.sort_unstable();
        }
        let watched = watched_path(path);
        if !self.watched_local_paths.contains(&watched) {
            self.watched_local_paths.push(watched);
            self.watched_local_paths.sort_unstable();
            self.watched_dependency_generation += 1;
        }
    }

    #[cfg(test)]
    fn inject_parser_failure(
        &self,
        path: &Path,
        result: std::io::Result<falcon_dart_parser::ParseResult>,
    ) -> std::io::Result<falcon_dart_parser::ParseResult> {
        if self.parser_failure_path.as_deref() == Some(path) {
            falcon_dart_parser::with_stack(|| panic!("project parser regression"))
        } else {
            result
        }
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
        if self.semantic_snapshot.is_some()
            || self.semantic_snapshot_error.is_some()
            || !self.resolve_semantics
        {
            return;
        }
        // Dependency waves and recursive index builds need the large-stack workers.
        if let Err(error) = falcon_dart_parser::with_stack(|| self.build_semantic_snapshot()) {
            // Keep the environment so its dependency roots stay watched for recovery.
            self.reset_external_layer();
            self.semantic_snapshot_error = Some(error);
        }
    }

    fn build_semantic_snapshot(&mut self) {
        #[cfg(test)]
        assert!(!self.build_panic, "semantic build regression");
        let (mut files, _) = self.collect_project_files(ProjectView::Semantic);
        if self.semantic_snapshot_error.is_some() {
            return;
        }
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
        let mut companion_failure = None;
        for path in self.external_libraries.project_companions() {
            if !seen.insert(path.to_path_buf()) {
                continue;
            }
            let language_version = self.environment.package_language_version(path);
            if let Some(cached) = self.disk_files.get(path)
                && cached.language_version == language_version
            {
                files.push(ProjectFile {
                    path: path.to_path_buf(),
                    parsed: Arc::clone(&cached.parsed),
                    has_parse_errors: cached.has_parse_errors,
                });
                continue;
            }
            self.disk_read_count += 1;
            match falcon_analyze::read_source(path) {
                Ok(source) => {
                    self.disk_parse_count += 1;
                    let result = try_parse_for_package(&source, language_version);
                    #[cfg(test)]
                    let result = self.inject_parser_failure(path, result);
                    let parsed = match result {
                        Ok(parsed) => parsed,
                        Err(error) => {
                            companion_failure = Some((
                                path.to_path_buf(),
                                std::io::Error::other(format!(
                                    "parsing companion {}: {error}",
                                    path.display()
                                )),
                            ));
                            break;
                        }
                    };
                    let falcon_dart_parser::ParseResult { tree, errors } = parsed;
                    let parsed = Arc::new(ParsedFile {
                        source: source.into_boxed_str(),
                        tree,
                    });
                    let has_parse_errors = !errors.is_empty();
                    self.disk_files.insert(
                        path.to_path_buf(),
                        CachedDiskFile {
                            parsed: Arc::clone(&parsed),
                            has_parse_errors,
                            language_version,
                        },
                    );
                    files.push(ProjectFile {
                        path: path.to_path_buf(),
                        parsed,
                        has_parse_errors,
                    });
                }
                Err(error) => warn!("failed to read {}: {}", path.display(), error),
            }
        }
        if let Some((path, error)) = companion_failure {
            self.record_semantic_worker_failure(&path, error);
            return;
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
        let by_path: HashMap<_, _> = files
            .iter()
            .enumerate()
            .map(|(index, file)| (file.path.clone(), index))
            .collect();
        self.disk_files.retain(|path, _| {
            by_path.contains_key(path)
                || self.project_paths.as_ref().is_some_and(|paths| {
                    paths
                        .binary_search_by(|(candidate, _)| candidate.cmp(path))
                        .is_ok()
                })
        });
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
        is_dart_file(path) && self.config_for_path(path).files.includes.admits(&path.to_string_lossy())
    }

    /// Stable ownership while a debounced edit has dropped the snapshot.
    /// Missing files, directories and outside-target symlinks remain conservative.
    fn watched_project_source(&self, path: &Path) -> bool {
        self.workspace_roots
            .iter()
            .any(|root| path.starts_with(root))
            && self.admits_project_dart(path)
            && path.is_file()
            && std::fs::canonicalize(path).ok().is_some_and(|resolved| {
                self.workspace_roots
                    .iter()
                    .any(|root| resolved.starts_with(root))
            })
    }

    /// Assemble the [`ProjectFile`] set: every non-excluded `.dart` file under
    /// the workspace root, preferring an open buffer's text + cached CST over the
    /// on-disk copy. Semantic analysis also includes open buffers that do not exist
    /// on disk yet. Cross-file analysis can additionally include package manifests.
    /// The boolean is false when oversized ingestion left the workspace incomplete;
    /// cross-file rules must not infer absence from that partial set.
    fn collect_project_files(&mut self, view: ProjectView) -> (Vec<ProjectFile>, bool) {
        if self.project_path_patterns != self.config.files.includes.patterns() {
            self.project_paths = None;
        }
        let paths = self.project_paths.take().unwrap_or_else(|| {
            self.workspace_walk_count += 1;
            self.project_path_patterns = self.config.files.includes.patterns().to_vec();
            let mut paths = Vec::new();
            self.project_discovery_watches.clear();
            for (index, root) in self.workspace_roots.iter().enumerate() {
                let discovery = discover_project_with_watches(
                    std::slice::from_ref(root),
                    &self.config_for_root(index).files.includes,
                    true,
                );
                self.project_discovery_watches.extend(discovery.watches);
                paths.extend(discovery.files.into_iter()
                    .map(|path| (canonical_or_lexical(&path), true)));
                paths.extend(discovery.manifests.into_iter()
                    .map(|path| (canonical_or_lexical(&path), false)));
            }
            paths.retain(|(path, is_dart)| {
                let includes = &self.config_for_path(path).files.includes;
                if *is_dart { includes.admits(&path.to_string_lossy()) }
                else { !includes.excludes(&path.to_string_lossy()) }
            });
            paths.sort_unstable_by(|a, b| a.0.cmp(&b.0));
            paths.dedup_by(|a, b| a.0 == b.0);
            let companions: HashSet<_> = self.external_libraries.project_companions().collect();
            self.disk_files.retain(|path, _| {
                paths
                    .binary_search_by(|(candidate, _)| candidate.cmp(path))
                    .is_ok()
                    || companions.contains(path.as_path())
            });
            paths
        });
        let mut complete = true;
        self.refresh_environment(
            paths
                .iter()
                .filter(|(_, is_dart)| *is_dart || matches!(view, ProjectView::CrossFile))
                .map(|(path, _)| path.as_path()),
        );
        self.refresh_document_versions();
        let open_by_path: HashMap<&Path, &DocumentState> = self
            .documents
            .values()
            .map(|doc| (doc.path.as_path(), doc))
            .collect();
        let capacity = paths.len() + self.documents.len();
        let mut files = Vec::with_capacity(capacity);
        let mut seen = HashSet::with_capacity(capacity);
        let mut worker_failure = None;
        for (normalized, is_dart) in &paths {
            if !is_dart && matches!(view, ProjectView::Semantic) {
                continue;
            }
            let path = normalized.as_path();
            seen.insert(path);
            if let Some(doc) = open_by_path.get(path) {
                if doc.source_rejected {
                    complete = false;
                    continue;
                }
                files.push(ProjectFile {
                    path: normalized.clone(),
                    parsed: Arc::clone(&doc.parsed),
                    has_parse_errors: doc.has_parse_errors,
                });
                continue;
            }
            let language_version = is_dart
                .then(|| self.environment.package_language_version(path))
                .flatten();
            if let Some(cached) = self.disk_files.get(path)
                && cached.language_version == language_version
            {
                files.push(ProjectFile {
                    path: normalized.clone(),
                    parsed: Arc::clone(&cached.parsed),
                    has_parse_errors: cached.has_parse_errors,
                });
                continue;
            }
            self.disk_read_count += 1;
            let source = match falcon_analyze::read_source(path) {
                Ok(source) => source,
                Err(e) => {
                    warn!("failed to read {}: {}", path.display(), e);
                    if falcon_analyze::is_source_too_large(&e) {
                        complete = false;
                    }
                    continue;
                }
            };
            let (tree, has_parse_errors) = if *is_dart {
                let package_version = language_version;
                self.disk_parse_count += 1;
                let result = try_parse_for_package(&source, package_version);
                #[cfg(test)]
                let result = self.inject_parser_failure(path, result);
                let parsed = match result {
                    Ok(parsed) => parsed,
                    Err(error) => {
                        worker_failure = Some((
                            normalized.clone(),
                            std::io::Error::other(format!("parsing {}: {error}", path.display())),
                        ));
                        complete = false;
                        break;
                    }
                };
                (parsed.tree, !parsed.errors.is_empty())
            } else {
                let file = manifest_file(normalized.clone(), source);
                self.disk_files.insert(
                    file.path.clone(),
                    CachedDiskFile {
                        parsed: Arc::clone(&file.parsed),
                        has_parse_errors: file.has_parse_errors,
                        language_version,
                    },
                );
                files.push(file);
                continue;
            };
            let parsed = Arc::new(ParsedFile {
                source: source.into_boxed_str(),
                tree,
            });
            self.disk_files.insert(
                normalized.clone(),
                CachedDiskFile {
                    parsed: Arc::clone(&parsed),
                    has_parse_errors,
                    language_version,
                },
            );
            files.push(ProjectFile {
                path: normalized.clone(),
                parsed,
                has_parse_errors,
            });
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
        self.project_paths = Some(paths);
        if let Some((path, error)) = worker_failure {
            self.record_semantic_worker_failure(&path, error);
        }
        (files, complete)
    }
}

/// Drop rejected editor text before any parsing, language-version scanning, or
/// semantic retention. A subsequent valid full-sync change recovers normally.
fn reject_oversized_buffer(path: &Path, text: &mut String) -> Option<Diagnostic> {
    let diagnostic = falcon_analyze::source_size_diagnostic(path, text.len() as u64)?;
    *text = String::new();
    Some(diagnostic)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod lifecycle_tests;

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
    let loaded = load_project_config_with_errors(path, start_dir);
    let label = discovered.as_deref().unwrap_or(start_dir).display();
    match loaded {
        Ok((mut loaded, errors)) => {
            if loaded.config.root_dir().is_none() {
                loaded.config.resolve_paths(&workspace_root);
            }
            let workspace_root = loaded
                .config
                .root_dir()
                .map_or(workspace_root, Path::to_path_buf);
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
                    .chain(errors.into_iter().map(|error| ShowMessageParams {
                        typ: MessageType::ERROR,
                        message: format!("Falcon: failed to load member config: {error}"),
                    }))
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
    (Arc::ptr_eq(&file.parsed, &other.parsed) || file.source() == other.source())
        && file.has_parse_errors == other.has_parse_errors
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
fn document_path(uri: &str, workspace_roots: &[PathBuf]) -> (PathBuf, bool) {
    let path = uri_to_path(uri);
    match std::fs::canonicalize(&path) {
        Ok(path) => (path, false),
        Err(_) => {
            let path = falcon_analyze::lexical_normalize(&path);
            for ancestor in path.ancestors() {
                let resolved = canonical_or_lexical(ancestor);
                if let Some(root) = workspace_roots.iter().find(|root| **root == resolved) {
                    let suffix = path.strip_prefix(ancestor).expect("path ancestor");
                    return (root.join(suffix), true);
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
