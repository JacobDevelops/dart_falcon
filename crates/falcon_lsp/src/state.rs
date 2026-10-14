//! LSP server state: document cache, cached ASTs, config-driven rule set.
//!
//! Text changes invalidate a document's AST; config changes rerun rules over
//! cached ASTs without reparsing.
//! Unresolved filesystem identities are retried before analysis when files materialize.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_types::Uri;

use falcon_analyze::{
    AnalyzeContext, CrossFileRuleRegistry, IdentityIndex, IdentitySource, LibraryGrouping,
    LibrarySource, PackageIdentity, ProjectFile, RuleRegistry, SemanticIndexes, SignatureIndex,
    TypeIndex, canonical_or_lexical, group_libraries, import_path, library_unit,
    syntax_error_diagnostics,
};
use falcon_config::{FalconConfig, load_config, load_or_default};
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    apply_severities, build_cross_file_registry, build_registry, package_identities,
    requires_resolution, run_cross_file,
};
use falcon_syntax::Program;
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
}

impl LspState {
    /// Create state with config from `config_path`, or discovery from the
    /// current directory when `None` (same order as the CLI: cwd → git root →
    /// `$HOME/.falcon.json` → defaults).
    pub fn new(config_path: Option<PathBuf>) -> Self {
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
        let (program, parse_errors) = parse(&text);
        let syntax_diagnostics = syntax_error_diagnostics(&path, &parse_errors);
        self.documents.insert(
            uri.to_string(),
            DocumentState {
                path,
                unresolved_path,
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
            if !self.pending_semantic_affected.contains_key(uri) {
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

        let doc = self.documents.get_mut(uri).expect("document checked above");
        let (program, parse_errors) = parse(&text);
        doc.syntax_diagnostics = syntax_error_diagnostics(&doc.path, &parse_errors);
        doc.text = text;
        doc.version = version;
        doc.program = program;
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
        let file_path = &self.documents[uri].path;

        let semantic = self.semantic_snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .by_path
                .get(file_path)
                .copied()
                .map(|file_index| (snapshot, file_index))
        });
        let Some(doc) = self.documents.get_mut(uri) else {
            return Vec::new();
        };
        let mut ctx = AnalyzeContext::new(&doc.path, &doc.text, &self.config);
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
    fn cross_file_diagnostics(&self) -> HashMap<String, Vec<Diagnostic>> {
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
        if self.semantic_snapshot.is_some() || !self.resolve_semantics {
            return;
        }
        let files = self.collect_project_files(ProjectView::Semantic);
        let semantic_files = files
            .iter()
            .map(|file| (file.path.clone(), &file.program))
            .collect::<Vec<_>>();
        let grouping = group_libraries(&semantic_files);
        let types = TypeIndex::from_library_files(files.iter().enumerate().map(|(index, file)| {
            LibrarySource {
                program: &file.program,
                has_parse_errors: file.has_parse_errors,
                has_unresolved_parts: grouping.is_unresolved(index),
            }
        }));
        let identity_sources = files
            .iter()
            .map(|file| IdentitySource {
                path: &file.path,
                program: &file.program,
                has_parse_errors: file.has_parse_errors,
            })
            .collect::<Vec<_>>();
        let packages = package_identities(files.iter().map(|file| file.path.as_path()));
        let identities = IdentityIndex::from_project_files(&identity_sources, &packages);
        let signatures = SignatureIndex::from_project_files(&semantic_files, &identities, &types);
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

    /// Assemble the [`ProjectFile`] set: every non-excluded `.dart` file under
    /// the workspace root, preferring an open buffer's text + cached AST over the
    /// on-disk copy. Semantic analysis also includes open buffers that do not exist
    /// on disk yet. Cross-file analysis can additionally include package manifests.
    fn collect_project_files(&self, view: ProjectView) -> Vec<ProjectFile> {
        let filter = &self.config.files.includes;
        let open_by_path: HashMap<PathBuf, &DocumentState> = self
            .documents
            .values()
            .map(|doc| (doc.path.clone(), doc))
            .collect();

        let mut files = Vec::new();
        let mut seen = HashSet::new();
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
            if is_dart && !filter.admits(&path_str) {
                continue;
            }
            let normalized = canonical_or_lexical(path);
            seen.insert(normalized.clone());
            if let Some(doc) = open_by_path.get(&normalized) {
                files.push(ProjectFile {
                    path: normalized,
                    source: doc.text.clone(),
                    program: doc.program.clone(),
                    has_parse_errors: doc.has_parse_errors,
                });
            } else {
                match std::fs::read_to_string(path) {
                    Ok(source) => {
                        let (program, errors) = if is_dart { parse(&source) } else { parse("") };
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
                if seen.insert(path.clone()) {
                    files.push(ProjectFile {
                        path,
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

    #[test]
    fn cross_file_collection_includes_pubspec_manifests() {
        let dir = tempfile::tempdir().unwrap();
        let config_path = dir.path().join("falcon.json");
        let manifest_path = dir.path().join("pubspec.yaml");
        std::fs::write(&config_path, "{}").unwrap();
        std::fs::write(&manifest_path, "name: workspace\n").unwrap();
        std::fs::write(dir.path().join("main.dart"), "void main() {}\n").unwrap();
        let state = LspState::new(Some(config_path));

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
