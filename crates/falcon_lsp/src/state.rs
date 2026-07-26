//! LSP server state: document cache, cached ASTs, config-driven rule set.
//!
//! Caching model (see `.omc/docs/LSP_CACHING_DESIGN.md`): a document's AST is
//! invalidated only by *text* changes; the rule set is invalidated only by
//! *config* changes. A config reload therefore re-runs the new rules over the
//! cached ASTs without re-parsing — by construction there is no
//! stale-AST-with-new-config state.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use lsp_types::Uri;

use falcon_analyze::{
    AnalyzeContext, CrossFileRuleRegistry, FileSuppressions, IdentityIndex, IdentitySource,
    LibraryGrouping, LibrarySource, PackageIdentity, ProgramSource, ProjectFile, ProjectIndex,
    Rule, SignatureIndex, TypeIndex, group_libraries, library_unit, syntax_error_diagnostics,
    with_rules_stack,
};
use falcon_config::{FalconConfig, load_config, load_or_default};
use falcon_dart_parser::parse;
use falcon_diagnostics::Diagnostic;
use falcon_rules::{
    apply_severities, meta::suppression_lookup, resolve_cross_file_rules, resolve_rules,
    rule_requires_resolution,
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

/// One immutable semantic view of the workspace. It owns the parsed files and
/// every index derived from them, so all documents affected by one edit share the
/// same facts instead of walking, parsing, and indexing the workspace repeatedly.
struct SemanticSnapshot {
    files: Vec<ProjectFile>,
    project: ProjectIndex,
    types: TypeIndex,
    identities: IdentityIndex,
    signatures: SignatureIndex,
    grouping: LibraryGrouping,
    by_path: HashMap<PathBuf, usize>,
    reverse_dependencies: Vec<Vec<usize>>,
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
        let resolved = resolve_rules(&config);
        let cross_file_rules = build_cross_file_registry(&config);
        let workspace_root = workspace_root_for(config_path.as_deref());
        let resolve_semantics = rules_require_resolution(&resolved.rules);
        Self {
            documents: HashMap::new(),
            config,
            config_path,
            rules: resolved.rules,
            cross_file_rules,
            workspace_root,
            resolve_semantics,
            semantic_snapshot: None,
            pending_semantic_affected: HashMap::new(),
            semantic_snapshot_build_count: 0,
            semantic_topology_build_count: 0,
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
        self.pending_semantic_affected.remove(uri);
        if self.resolve_semantics && !self.documents.is_empty() {
            self.ensure_semantic_snapshot();
            if let Some(snapshot) = self.semantic_snapshot.as_ref() {
                let path = normalize_path(&uri_to_path(uri));
                let affected = affected_open_uris_in_snapshot(snapshot, &path, &self.documents);
                self.pending_semantic_affected
                    .insert(uri.to_string(), affected.into_iter().collect());
            }
        }
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
        self.invalidate_semantic_snapshot();
        self.analyze(uri)
    }

    /// `textDocument/didChange` (full sync): replace text and re-parse the
    /// changed document only. Analysis is the caller's responsibility — the
    /// server loop defers it behind the debounce window.
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
                let changed_path = normalize_path(&uri_to_path(uri));
                let affected =
                    affected_open_uris_in_snapshot(snapshot, &changed_path, &self.documents);
                self.pending_semantic_affected
                    .entry(uri.to_string())
                    .or_default()
                    .extend(affected);
            }
        }

        let doc = self.documents.get_mut(uri).expect("document checked above");
        let (program, parse_errors) = parse(&text);
        doc.syntax_diagnostics = syntax_error_diagnostics(&uri_to_path(uri), &parse_errors);
        doc.text = text;
        doc.version = version;
        doc.program = program;
        doc.has_parse_errors = !parse_errors.is_empty();
        doc.parse_count += 1;
        self.invalidate_semantic_snapshot();
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
        self.pending_semantic_affected.remove(uri);
        if self.documents.remove(uri).is_some() {
            self.invalidate_semantic_snapshot();
        }
    }

    /// Run the enabled rules over the cached AST of `uri`. Diagnostics are
    /// sorted by span for deterministic publishing.
    pub fn analyze(&mut self, uri: &str) -> Vec<Diagnostic> {
        let file_path = normalize_path(&uri_to_path(uri));
        if !self.documents.contains_key(uri) {
            return Vec::new();
        }
        if self.resolve_semantics {
            self.ensure_semantic_snapshot();
        }

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
        let mut ctx = AnalyzeContext::new(&file_path, &doc.text, &self.config);
        let programs;
        let library;
        if let Some((snapshot, file_index)) = semantic {
            programs = snapshot
                .files
                .iter()
                .map(|file| &file.program)
                .collect::<Vec<_>>();
            library = library_unit(&snapshot.grouping, &programs, file_index);
            ctx = ctx
                .with_project(&snapshot.project)
                .with_types(&snapshot.types)
                .with_identities(&snapshot.identities)
                .with_signatures(&snapshot.signatures)
                .with_library(&library);
        }
        // Same large-stack protection as RuleRegistry::run_all — an open buffer
        // can hold a deep-but-legal AST that would overflow this thread.
        let mut diagnostics: Vec<Diagnostic> = with_rules_stack(|| {
            self.rules
                .iter()
                .flat_map(|rule| rule.analyze(&doc.program, &ctx))
                .collect()
        });
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

    /// Re-analyze `uri` and every open document whose semantic facts can depend
    /// on it through a library part, import, or export edge.
    pub fn analyze_affected(&mut self, uri: &str) -> Vec<(String, Vec<Diagnostic>)> {
        self.affected_open_uris(uri)
            .into_iter()
            .map(|affected_uri| {
                let diagnostics = self.analyze(&affected_uri);
                (affected_uri, diagnostics)
            })
            .collect()
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
        let changed_path = normalize_path(&uri_to_path(uri));
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
        let resolved = resolve_rules(&self.config);
        self.resolve_semantics = rules_require_resolution(&resolved.rules);
        self.rules = resolved.rules;
        self.cross_file_rules = build_cross_file_registry(&self.config);
        self.pending_semantic_affected.clear();
        self.invalidate_semantic_snapshot();
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
        let files = self.collect_project_files(false, true);
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

    fn invalidate_semantic_snapshot(&mut self) {
        self.semantic_snapshot = None;
    }

    fn ensure_semantic_snapshot(&mut self) {
        if self.semantic_snapshot.is_some() || !self.resolve_semantics {
            return;
        }
        let files = self.collect_project_files(true, false);
        let program_sources = files
            .iter()
            .map(|file| ProgramSource {
                program: &file.program,
                has_parse_errors: file.has_parse_errors,
            })
            .collect::<Vec<_>>();
        let project = ProjectIndex::from_project_files(&program_sources);
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
        let packages = package_identities(&files);
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
            project,
            types,
            identities,
            signatures,
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
    fn collect_project_files(
        &self,
        include_open_only: bool,
        include_manifests: bool,
    ) -> Vec<ProjectFile> {
        let exclude = compile_patterns(&self.config.files.exclude_patterns());
        let includes = compile_patterns(&self.config.files.include_patterns());
        let open_by_path: HashMap<PathBuf, &DocumentState> = self
            .documents
            .iter()
            .map(|(uri, doc)| (normalize_path(&uri_to_path(uri)), doc))
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
            let is_manifest = include_manifests
                && path.file_name().and_then(|name| name.to_str()) == Some("pubspec.yaml");
            if !is_dart && !is_manifest {
                continue;
            }
            let path_str = path.to_string_lossy();
            if exclude.iter().any(|p| p.matches(&path_str)) {
                continue;
            }
            if is_dart && !includes.is_empty() && !includes.iter().any(|p| p.matches(&path_str)) {
                continue;
            }
            let normalized = normalize_path(path);
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
        if include_open_only {
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
        .keys()
        .map(|open_uri| (normalize_path(&uri_to_path(open_uri)), open_uri))
        .collect::<HashMap<_, _>>();
    affected
        .into_iter()
        .filter_map(|index| open_by_path.get(&snapshot.files[index].path).copied())
        .cloned()
        .collect()
}

fn rules_require_resolution(rules: &[Box<dyn Rule>]) -> bool {
    rules
        .iter()
        .any(|rule| rule_requires_resolution(rule.name()))
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

fn package_identities(files: &[ProjectFile]) -> Vec<PackageIdentity> {
    // Dedupe the directories before stat'ing: every file in a package shares
    // the same ancestor chain.
    let mut directories = files
        .iter()
        .filter_map(|file| file.path.parent())
        .flat_map(Path::ancestors)
        .collect::<Vec<_>>();
    directories.sort_unstable();
    directories.dedup();
    directories
        .into_iter()
        .map(|directory| directory.join("pubspec.yaml"))
        .filter(|manifest| manifest.is_file())
        .filter_map(|path| {
            // An unreadable manifest still bounds ownership, so keep the entry
            // with an empty name (see `unreadable_nested_pubspec_...`).
            let name = std::fs::read(&path)
                .ok()
                .and_then(|source| serde_yaml::from_slice::<serde_yaml::Value>(&source).ok())
                .and_then(|manifest| manifest.get("name")?.as_str().map(str::to_owned))
                .unwrap_or_default();
            Some(PackageIdentity {
                name,
                lib_root: path.parent()?.join("lib"),
            })
        })
        .collect()
}

fn resolve_dependency(
    from: &Path,
    uri: &str,
    by_path: &HashMap<PathBuf, usize>,
    packages: &[PackageIdentity],
) -> Option<usize> {
    if uri.starts_with("dart:") {
        return None;
    }
    if let Some(rest) = uri.strip_prefix("package:") {
        let (package_name, subpath) = rest.split_once('/')?;
        let mut matches = packages
            .iter()
            .filter(|package| package.name == package_name);
        let package = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        let lib_root = normalize_path(&package.lib_root);
        let target = normalize_path(&lib_root.join(subpath));
        if !target.starts_with(&lib_root) {
            return None;
        }
        return by_path.get(&target).copied();
    }
    let parent = from.parent().unwrap_or_else(|| Path::new(""));
    by_path.get(&normalize_path(&parent.join(uri))).copied()
}

#[cfg(test)]
fn owning_package<'a>(from: &Path, packages: &'a [PackageIdentity]) -> Option<&'a PackageIdentity> {
    let from = normalize_path(from);
    packages
        .iter()
        .filter(|package| {
            package
                .lib_root
                .parent()
                .is_some_and(|root| from.starts_with(normalize_path(root)))
        })
        .max_by_key(|package| {
            package
                .lib_root
                .parent()
                .map_or(0, |root| normalize_path(root).components().count())
        })
}

fn normalize_path(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    if matches!(
                        normalized.components().next_back(),
                        Some(Component::Normal(_))
                    ) {
                        normalized.pop();
                    } else if !matches!(
                        normalized.components().next_back(),
                        Some(Component::RootDir)
                    ) {
                        normalized.push(component.as_os_str());
                    }
                }
                Component::CurDir => {}
                other => normalized.push(other.as_os_str()),
            }
        }
        normalized
    })
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
/// diagnostic attribution and semantic dependency matching.
pub fn uri_to_path(uri: &str) -> PathBuf {
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

        let packages = package_identities(&files);
        let owner = owning_package(&path, &packages).expect("nested manifest owns the file");
        assert_eq!(owner.lib_root, nested.join("lib"));
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

        let files = state.collect_project_files(false, true);
        let manifest = files
            .iter()
            .find(|file| file.path == normalize_path(&manifest_path))
            .expect("pubspec.yaml must be included in the cross-file project view");
        assert_eq!(manifest.source, "name: workspace\n");
        assert!(!manifest.has_parse_errors);
    }

    #[test]
    fn file_uri_decodes_percent_encoded_path() {
        assert_eq!(
            uri_to_path("file:///workspace/with%20space/main.dart"),
            PathBuf::from("/workspace/with space/main.dart")
        );
    }
}
