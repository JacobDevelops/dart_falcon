//! Loading libraries omitted from the requested project files.
//! External declarations never supply lint diagnostics or cross-file rule inputs.

use std::path::{Path, PathBuf};
use std::sync::{Arc, PoisonError, RwLock};

use falcon_dart_parser::parser::ParseError;
use falcon_dart_parser::try_parse_for_package;
use falcon_syntax::cst::visit::{Visitor, walk_top_level_decl};
use falcon_syntax::cst::{ClassMember, FunctionDecl, Program, Span, TopLevelDecl};
use rayon::prelude::*;
use rustc_hash::{FxHashMap, FxHashSet};
use tracing::warn;

use super::{AnalysisEnvironment, LanguageVersion};
use crate::intern::{InternedString, StringInterner};
use crate::resolve::canonical_or_lexical;

/// Canonical forms of raw resolution targets; `None` when already canonical.
type CanonicalPaths = RwLock<FxHashMap<PathBuf, Option<PathBuf>>>;

/// Where an external library comes from; determines its declarations'
/// [`crate::DeclarationIdentity`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ExternalOrigin {
    /// A `dart:` library, by URI (`dart:core`).
    Sdk(InternedString),
    /// A library of a dependency package, by package name.
    Package(InternedString),
    /// A local library outside a configured package's public library directory.
    Project,
}

/// Owned syntax and declaration facts for one external file.
#[derive(Debug)]
pub struct ExternalFile {
    pub path: PathBuf,
    pub parsed: Arc<crate::ParsedFile>,
    /// Library documentation imports.
    pub doc_imports: Vec<String>,
    pub has_parse_errors: bool,
}

impl ExternalFile {
    pub fn program(&self) -> Program<'_> {
        self.parsed.program()
    }
}

/// An external library: its defining file first, then its parts.
#[derive(Debug)]
pub struct ExternalLibrary {
    pub origin: ExternalOrigin,
    pub files: Arc<Vec<ExternalFile>>,
    path_id: u32,
    pub context: Option<usize>,
    /// Smallest configured context in this instance's equivalence group.
    pub copy_context: Option<usize>,
    /// At least one declared part URI could not be resolved. Resolved paths
    /// that cannot be read contribute no declarations, as in the analyzer.
    pub has_unresolved_parts: bool,
}

/// Every external library transitively imported or exported by a set of
/// project files.
#[derive(Debug, Default)]
pub struct ExternalLibraries {
    libraries: Vec<ExternalLibrary>,
    paths: FxHashMap<PathBuf, u32>,
    canonical_paths: CanonicalPaths,
    states: Vec<PathState>,
    pending_contexts: FxHashSet<(u32, Option<usize>)>,
    resolved_dependencies: FxHashMap<u32, Vec<u32>>,
    aliases: FxHashMap<(u32, Option<usize>), usize>,
    grouped_library_count: usize,
    /// Allocated only after a non-SDK path is reached in multiple contexts.
    graph: Option<InstanceGraph>,
    /// Local defining targets, including unreadable paths that need file watches.
    attempted_project_paths: FxHashSet<PathBuf>,
    /// Files evicted for open project buffers; closing one requires retraversal.
    project_evictions: FxHashMap<PathBuf, PathBuf>,
    eviction_roots: FxHashMap<PathBuf, usize>,
    project_companions: std::collections::BTreeMap<PathBuf, Vec<PathBuf>>,
    companion_dependencies: FxHashMap<PathBuf, CompanionDependencies>,
    /// Defining files whose parser thread could not start; the next extend retries them.
    retry: Vec<(u32, PathBuf, Option<usize>)>,
}

/// Source-validated directives: unchanged snapshots need not reparse unopened parts.
#[derive(Debug)]
struct CompanionDependencies {
    source: String,
    context: Option<usize>,
    dependencies: Vec<PathBuf>,
    parts: Vec<PathBuf>,
}

#[derive(Debug)]
struct PathState {
    context: Option<usize>,
    sdk: Option<bool>,
    primary: Option<usize>,
    attempted: bool,
}

#[derive(Debug, Default)]
struct InstanceGraph {
    names: FxHashMap<String, u32>,
    indices: FxHashMap<(u32, Option<usize>), usize>,
    nodes: Vec<(u32, Option<usize>)>,
    facts: Vec<Option<NodeFacts>>,
    dirty: bool,
}

#[derive(Debug)]
struct NodeFacts {
    dependencies: Vec<u32>,
    names: Vec<u32>,
}

/// A loaded instance or an unresolved physical target.
#[derive(Debug)]
pub enum ExternalTarget {
    Instance(usize),
    Path(PathBuf),
}

impl ExternalLibraries {
    /// Load every library reachable from `project` (path, program, source)
    /// through imports, exports and documentation imports, plus implicit
    /// `dart:core`. Conditional directives use their default URI.
    /// Stop at canonical project paths.
    pub fn load(
        environment: &AnalysisEnvironment,
        project: &[(&Path, Program<'_>, &str)],
        is_project: impl Fn(&Path) -> bool,
    ) -> Self {
        let mut loaded = Self::default();
        loaded.extend(environment, project, is_project);
        loaded
    }

    /// Load the libraries reachable from `project` that are not loaded yet;
    /// libraries already loaded (or already found unloadable) are kept as is,
    /// except files now owned by the project, whose open buffers take priority.
    /// `environment` must be the one (possibly since extended) earlier loads used.
    pub fn extend(
        &mut self,
        environment: &AnalysisEnvironment,
        project: &[(&Path, Program<'_>, &str)],
        is_project: impl Fn(&Path) -> bool,
    ) {
        self.canonical_paths
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .clear();
        self.pending_contexts.clear();
        let mut evictions = self.project_evictions.clone();
        for library in &self.libraries {
            for file in library.files.iter().filter(|file| is_project(&file.path)) {
                evictions.insert(file.path.clone(), library.files[0].path.clone());
                self.project_companions.insert(
                    library.files[0].path.clone(),
                    library.files.iter().map(|file| file.path.clone()).collect(),
                );
            }
        }
        if evictions != self.project_evictions
            || self.project_evictions.keys().any(|path| !is_project(path))
        {
            // Rebuild the reachable graph, not just individual files: an evicted
            // part belongs to its defining library, and a closed transitive
            // dependency must be reached through previously attempted parents.
            evictions.retain(|path, _| is_project(path));
            let mut eviction_roots = FxHashMap::default();
            for root in evictions.values() {
                *eviction_roots.entry(root.clone()).or_default() += 1;
            }
            let mut companions = std::mem::take(&mut self.project_companions);
            companions.retain(|root, _| eviction_roots.contains_key(root));
            *self = Self {
                project_evictions: evictions,
                eviction_roots,
                project_companions: companions,
                companion_dependencies: std::mem::take(&mut self.companion_dependencies),
                ..Self::default()
            };
        }
        self.project_companions.retain(|root, companions| {
            companions.retain(|path| !is_project(path));
            !companions.is_empty() && (is_project(root) || self.eviction_roots.contains_key(root))
        });
        let mut wave = Vec::new();
        for &(path, program, source) in project {
            let context = environment.context_index(path);
            let mut dependencies =
                dependencies(environment, &self.canonical_paths, context, path, program);
            dependencies.extend(
                program
                    .part_of_directive()
                    .and_then(|part| part.uri())
                    .and_then(|uri| {
                        resolve(
                            environment,
                            &self.canonical_paths,
                            context,
                            path,
                            &uri.value(),
                        )
                    }),
            );
            dependencies.extend(
                crate::resolve::doc_imports(program, source).filter_map(|uri| {
                    resolve(environment, &self.canonical_paths, context, path, uri)
                }),
            );
            // Buffered defining directives own unopened parts, even at traversal boundaries.
            let root = canonical_or_lexical(path);
            self.project_companions.remove(&root);
            let mut seen = FxHashSet::from_iter([root.clone()]);
            let mut parts: Vec<_> = program
                .part_directives()
                .filter_map(|part| {
                    resolve(
                        environment,
                        &self.canonical_paths,
                        context,
                        path,
                        &part.uri().value(),
                    )
                })
                .collect();
            let mut companions = Vec::new();
            while let Some(part) = parts.pop() {
                if !seen.insert(part.clone()) {
                    continue;
                }
                if is_project(&part) {
                    continue;
                }
                companions.push(part.clone());
                let Some(cached) = self.companion_dependencies(environment, context, &part) else {
                    continue;
                };
                dependencies.extend(cached.dependencies.iter().cloned());
                parts.extend(cached.parts.iter().cloned());
            }
            if !companions.is_empty() {
                self.project_companions.insert(root, companions);
            }
            for dependency in dependencies {
                if is_project(&dependency) {
                    self.record_eviction(dependency.clone(), dependency);
                } else if !self.eviction_roots.contains_key(&dependency) {
                    self.enqueue(dependency, context, &mut wave);
                }
            }
        }
        wave.append(&mut self.retry);
        self.load_waves(environment, wave, &is_project);
        self.group_instances(environment);
        if !self.companion_dependencies.is_empty() {
            let referenced: FxHashSet<_> = self.project_companions.values().flatten().collect();
            self.companion_dependencies
                .retain(|path, _| referenced.contains(path));
        }
    }

    fn companion_dependencies(
        &mut self,
        environment: &AnalysisEnvironment,
        context: Option<usize>,
        path: &Path,
    ) -> Option<&CompanionDependencies> {
        let source = match crate::source::read_source(path) {
            Ok(source) => source,
            Err(_) => {
                self.companion_dependencies.remove(path);
                return None;
            }
        };
        let unchanged = self
            .companion_dependencies
            .get(path)
            .is_some_and(|cached| cached.source == source && cached.context == context);
        if !unchanged {
            let package_version = environment
                .package_config(path)
                .and_then(|config| config.language_version_canonical(path));
            let Ok(file) = read_source(path, &source, package_version) else {
                self.companion_dependencies.remove(path);
                return None;
            };
            let mut dependencies = dependencies(
                environment,
                &self.canonical_paths,
                context,
                path,
                file.program(),
            );
            dependencies.extend(
                file.doc_imports.iter().filter_map(|uri| {
                    resolve(environment, &self.canonical_paths, context, path, uri)
                }),
            );
            let parts = file
                .program()
                .part_directives()
                .filter_map(|part| {
                    resolve(
                        environment,
                        &self.canonical_paths,
                        context,
                        path,
                        &part.uri().value(),
                    )
                })
                .collect();
            self.companion_dependencies.insert(
                path.to_path_buf(),
                CompanionDependencies {
                    source,
                    context,
                    dependencies,
                    parts,
                },
            );
        }
        self.companion_dependencies.get(path)
    }

    fn record_eviction(&mut self, path: PathBuf, root: PathBuf) {
        if let Some(previous) = self.project_evictions.insert(path, root.clone()) {
            if previous == root {
                return;
            }
            let count = self.eviction_roots.get_mut(&previous).unwrap();
            *count -= 1;
            if *count == 0 {
                self.eviction_roots.remove(&previous);
            }
        }
        *self.eviction_roots.entry(root).or_default() += 1;
    }

    fn enqueue(
        &mut self,
        path: PathBuf,
        context: Option<usize>,
        wave: &mut Vec<(u32, PathBuf, Option<usize>)>,
    ) -> u32 {
        let next = self.states.len() as u32;
        let id = *self.paths.entry(path.clone()).or_insert_with(|| {
            self.states.push(PathState {
                context,
                sdk: None,
                primary: None,
                attempted: false,
            });
            next
        });
        let state = &self.states[id as usize];
        let context = if state.sdk == Some(true) {
            state.context
        } else {
            context
        };
        let fresh = if state.sdk.is_none() {
            id == next || (context != state.context && self.pending_contexts.insert((id, context)))
        } else {
            self.register_context(id, context) || id == next
        };
        if fresh {
            wave.push((id, path, context));
        }
        id
    }

    fn register_context(&mut self, id: u32, context: Option<usize>) -> bool {
        let state = &self.states[id as usize];
        if state.sdk != Some(false) {
            return false;
        }
        if context != state.context && self.graph.is_none() {
            let mut graph = InstanceGraph {
                indices: FxHashMap::with_capacity_and_hasher(self.states.len(), Default::default()),
                nodes: Vec::with_capacity(self.states.len()),
                facts: Vec::with_capacity(self.states.len()),
                ..InstanceGraph::default()
            };
            for (id, state) in self.states.iter().enumerate() {
                if state.sdk == Some(false) {
                    let node = (id as u32, state.context);
                    graph.indices.insert(node, graph.nodes.len());
                    graph.nodes.push(node);
                    graph.facts.push(None);
                }
            }
            self.graph = Some(graph);
        }
        if let Some(graph) = &mut self.graph {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                graph.indices.entry((id, context))
            {
                entry.insert(graph.nodes.len());
                graph.nodes.push((id, context));
                graph.facts.push(None);
                graph.dirty = true;
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    fn enqueue_dependencies(
        &mut self,
        dependencies: Vec<PathBuf>,
        context: Option<usize>,
        is_project: &impl Fn(&Path) -> bool,
        next: &mut Vec<(u32, PathBuf, Option<usize>)>,
        collect_targets: bool,
    ) -> Option<Vec<u32>> {
        let mut targets = collect_targets.then(|| Vec::with_capacity(dependencies.len()));
        for dependency in dependencies {
            if is_project(&dependency) {
                self.record_eviction(dependency.clone(), dependency);
            } else if !self.eviction_roots.contains_key(&dependency) {
                let id = self.enqueue(dependency, context, next);
                if let Some(targets) = &mut targets
                    && self.states[id as usize].sdk != Some(true)
                {
                    targets.push(id);
                }
            }
        }
        targets
    }

    fn load_waves(
        &mut self,
        environment: &AnalysisEnvironment,
        mut wave: Vec<(u32, PathBuf, Option<usize>)>,
        is_project: &impl Fn(&Path) -> bool,
    ) {
        let track_dependencies =
            environment.contexts.len() + usize::from(environment.default_sdk.is_some()) > 1;
        while !wave.is_empty() {
            let mut revisits = Vec::new();
            wave.retain(|(id, path, context)| {
                let state = &mut self.states[*id as usize];
                if state.attempted {
                    revisits.push((*id, path.clone(), *context));
                    false
                } else {
                    state.attempted = true;
                    state.context = *context;
                    true
                }
            });
            let loaded: Vec<_> = wave
                .into_par_iter()
                .map_init(StringInterner::default, |interner, (id, path, context)| {
                    let origin = environment.origin(context, &path, interner);
                    let local = matches!(origin, ExternalOrigin::Project);
                    let sdk = matches!(origin, ExternalOrigin::Sdk(_));
                    let loaded = load_library(
                        environment,
                        &self.canonical_paths,
                        id,
                        &path,
                        context,
                        origin,
                    );
                    (id, path, context, local, sdk, loaded)
                })
                .collect();
            let mut next = Vec::new();
            for (id, path, context, local, sdk, loaded) in loaded {
                self.states[id as usize].sdk = Some(sdk);
                self.register_context(id, context);
                if local {
                    self.attempted_project_paths.insert(path.clone());
                }
                let loaded = match loaded {
                    Ok(loaded) => loaded,
                    Err(error) => {
                        warn!(
                            "could not start parser thread for {}: {error}",
                            path.display()
                        );
                        self.states[id as usize].attempted = false;
                        self.retry.push((id, path, context));
                        continue;
                    }
                };
                let Some((library, dependencies)) = loaded else {
                    continue;
                };
                let mut project_owned = false;
                for file in library.files.iter().filter(|file| is_project(&file.path)) {
                    self.record_eviction(file.path.clone(), library.files[0].path.clone());
                    project_owned = true;
                }
                if project_owned {
                    self.project_companions.insert(
                        library.files[0].path.clone(),
                        library
                            .files
                            .iter()
                            .filter(|file| !is_project(&file.path))
                            .map(|file| file.path.clone())
                            .collect(),
                    );
                } else {
                    self.states[id as usize].primary = Some(self.libraries.len());
                    self.libraries.push(library);
                    let mut pending = Vec::new();
                    self.retry.retain(|(retry_id, retry_path, retry_context)| {
                        if *retry_id == id {
                            pending.push((*retry_id, retry_path.clone(), *retry_context));
                            false
                        } else {
                            true
                        }
                    });
                    revisits.extend(pending);
                }
                let collect_targets = !sdk
                    && self.states[id as usize].primary.is_some()
                    && (track_dependencies || self.graph.is_some());
                let targets = self.enqueue_dependencies(
                    dependencies,
                    context,
                    is_project,
                    &mut next,
                    collect_targets,
                );
                if let Some(targets) = targets {
                    if self.graph.is_some() {
                        self.cache_node(environment, id, context, targets);
                    } else {
                        self.resolved_dependencies.insert(id, targets);
                    }
                }
            }
            for (id, path, context) in revisits {
                let state = &self.states[id as usize];
                if state.sdk == Some(true) {
                    continue;
                }
                let Some(index) = state.primary else {
                    if !state.attempted {
                        self.retry.push((id, path, context));
                    }
                    continue;
                };
                if context == state.context {
                    continue;
                }
                self.register_context(id, context);
                if self
                    .graph
                    .as_ref()
                    .is_some_and(|graph| graph.facts[graph.indices[&(id, context)]].is_some())
                {
                    continue;
                }
                let library = &self.libraries[index];
                let dependencies =
                    library_dependencies(environment, &self.canonical_paths, context, library);
                let targets =
                    self.enqueue_dependencies(dependencies, context, is_project, &mut next, true);
                if let Some(targets) = targets {
                    self.cache_node(environment, id, context, targets);
                }
            }
            wave = next;
        }
    }

    /// Replace a node's dependency and package-name facts after traversal or activation backfill.
    fn cache_node(
        &mut self,
        environment: &AnalysisEnvironment,
        id: u32,
        context: Option<usize>,
        dependencies: Vec<u32>,
    ) {
        let graph = self.graph.as_mut().unwrap();
        let node = graph.indices[&(id, context)];
        let index = self.states[id as usize].primary.unwrap();
        let mut facts = NodeFacts {
            dependencies,
            names: Vec::new(),
        };
        {
            let library = &self.libraries[index];
            if let Some(package) = environment.config_in(context).and_then(|config| {
                config.package_library_containing_canonical(&library.files[0].path)
            }) {
                let name = if let Some(&id) = graph.names.get(&package.name) {
                    id
                } else {
                    let next = graph.names.len() as u32;
                    graph.names.insert(package.name.clone(), next);
                    next
                };
                facts.names.push(name);
            }
            for file in library.files.iter() {
                for uri in directive_uris(file).filter(|uri| !uri.starts_with("dart:")) {
                    if let Some(name) = uri
                        .strip_prefix("package:")
                        .map(|rest| rest.split('/').next().unwrap_or(rest))
                    {
                        let next = graph.names.len() as u32;
                        let name = if let Some(&id) = graph.names.get(name) {
                            id
                        } else {
                            graph.names.insert(name.to_owned(), next);
                            next
                        };
                        facts.names.push(name);
                    }
                }
            }
        }
        facts.dependencies.sort_unstable();
        facts.dependencies.dedup();
        facts.names.sort_unstable();
        facts.names.dedup();
        graph.facts[node] = Some(facts);
        graph.dirty = true;
    }

    fn group_instances(&mut self, environment: &AnalysisEnvironment) {
        if self.grouped_library_count == self.libraries.len()
            && self.graph.as_ref().is_none_or(|graph| !graph.dirty)
        {
            return;
        }
        if !self.aliases.is_empty() {
            let mut primary = vec![false; self.libraries.len()];
            for state in &self.states {
                if let Some(index) = state.primary {
                    primary[index] = true;
                }
            }
            let mut remap = vec![0; self.libraries.len()];
            let mut old = 0;
            let mut next = 0;
            self.libraries.retain(|_| {
                let keep = primary[old];
                if keep {
                    remap[old] = next;
                    next += 1;
                }
                old += 1;
                keep
            });
            for state in &mut self.states {
                state.primary = state.primary.map(|index| remap[index]);
            }
            self.aliases.clear();
        }
        if self.graph.is_some() {
            for (id, targets) in std::mem::take(&mut self.resolved_dependencies) {
                let context = self.states[id as usize].context;
                self.cache_node(environment, id, context, targets);
            }
        }
        if let Some(graph) = &self.graph {
            let missing: Vec<_> = graph
                .nodes
                .iter()
                .copied()
                .enumerate()
                .filter(|(index, (id, _))| {
                    graph.facts[*index].is_none() && self.states[*id as usize].primary.is_some()
                })
                .map(|(_, node)| node)
                .collect();
            let canonical_paths = &self.canonical_paths;
            let backfill: Vec<_> = missing
                .into_par_iter()
                .map(|(id, context)| {
                    let library = &self.libraries[self.states[id as usize].primary.unwrap()];
                    let dependencies =
                        library_dependencies(environment, canonical_paths, context, library)
                            .into_iter()
                            .filter_map(|path| self.paths.get(&path).copied())
                            .filter(|&target| self.states[target as usize].sdk != Some(true))
                            .collect();
                    (id, context, dependencies)
                })
                .collect();
            for (id, context, dependencies) in backfill {
                self.cache_node(environment, id, context, dependencies);
            }
            let graph = self.graph.as_ref().unwrap();
            let edges: Vec<Vec<usize>> = graph
                .nodes
                .iter()
                .zip(&graph.facts)
                .map(|(&(_, context), facts)| {
                    facts.as_ref().map_or_else(Vec::new, |facts| {
                        facts
                            .dependencies
                            .iter()
                            .filter_map(|&target| graph.indices.get(&(target, context)).copied())
                            .collect()
                    })
                })
                .collect();
            let names: Vec<&[u32]> = graph
                .facts
                .iter()
                .map(|facts| {
                    facts
                        .as_ref()
                        .map_or(&[][..], |facts| facts.names.as_slice())
                })
                .collect();
            let closures = dependency_closures(&edges, &names, graph.names.len());
            let mut mappings = FxHashMap::default();
            for &(_, context) in &graph.nodes {
                mappings.entry(context).or_insert_with(|| {
                    let mut roots = vec![None; graph.names.len()];
                    if let Some(config) = environment.config_in(context) {
                        for package in &config.packages {
                            if let Some(&id) = graph.names.get(&package.name) {
                                roots[id as usize] = Some(&package.lib_root);
                            }
                        }
                    }
                    roots
                });
            }
            for state in &self.states {
                if let Some(primary) = state.primary {
                    self.libraries[primary].copy_context = state.context;
                }
            }
            let mut groups: FxHashMap<u32, Vec<(usize, usize)>> = FxHashMap::default();
            for (node_index, &(id, context)) in graph.nodes.iter().enumerate() {
                let Some(primary) = self.states[id as usize].primary else {
                    continue;
                };
                let current_mapping = &mappings[&context];
                let equivalent = |representative: usize| {
                    let reference_mapping = &mappings[&graph.nodes[representative].1];
                    closures[representative]
                        .iter()
                        .zip(&closures[node_index])
                        .enumerate()
                        .all(|(word_index, (&reference, &current))| {
                            let mut remaining = reference | current;
                            while remaining != 0 {
                                let name = word_index * 64 + remaining.trailing_zeros() as usize;
                                if current_mapping[name] != reference_mapping[name] {
                                    return false;
                                }
                                remaining &= remaining - 1;
                            }
                            true
                        })
                };
                let instances = groups.entry(id).or_insert_with(|| {
                    vec![(
                        graph.indices[&(id, self.states[id as usize].context)],
                        primary,
                    )]
                });
                let instance = if let Some(&(_, instance)) = instances
                    .iter()
                    .find(|&&(representative, _)| equivalent(representative))
                {
                    instance
                } else {
                    let library = &self.libraries[primary];
                    let instance = self.libraries.len();
                    self.libraries.push(ExternalLibrary {
                        origin: library.origin.clone(),
                        files: Arc::clone(&library.files),
                        path_id: library.path_id,
                        context,
                        copy_context: context,
                        has_unresolved_parts: library.has_unresolved_parts,
                    });
                    instances.push((node_index, instance));
                    instance
                };
                let library = &mut self.libraries[instance];
                library.copy_context = library.copy_context.into_iter().chain(context).min();
                if instance != primary {
                    library.context = library.copy_context;
                    self.aliases.insert((id, context), instance);
                }
            }
            let mut interner = StringInterner::default();
            for instances in groups.values() {
                for &(_, instance) in instances.iter().skip(1) {
                    let library = &mut self.libraries[instance];
                    library.origin =
                        environment.origin(library.context, &library.files[0].path, &mut interner);
                }
            }
            self.graph.as_mut().unwrap().dirty = false;
        }
        if self.aliases.is_empty() {
            self.libraries.sort_unstable_by(|left, right| {
                (&left.files[0].path, left.copy_context)
                    .cmp(&(&right.files[0].path, right.copy_context))
            });
            for (index, library) in self.libraries.iter().enumerate() {
                self.states[library.path_id as usize].primary = Some(index);
            }
            self.grouped_library_count = self.libraries.len();
            return;
        }
        let mut ordered: Vec<_> = self.libraries.drain(..).enumerate().collect();
        ordered.sort_unstable_by(|(_, left), (_, right)| {
            (&left.files[0].path, left.copy_context)
                .cmp(&(&right.files[0].path, right.copy_context))
        });
        let mut remap = vec![0; ordered.len()];
        self.libraries.reserve(ordered.len());
        for (index, (old, library)) in ordered.into_iter().enumerate() {
            remap[old] = index;
            self.libraries.push(library);
        }
        for state in &mut self.states {
            state.primary = state.primary.map(|index| remap[index]);
        }
        for instance in self.aliases.values_mut() {
            *instance = remap[*instance];
        }
        self.grouped_library_count = self.libraries.len();
    }

    /// Disk companions of a library promoted to project ownership by an open
    /// defining file or part. Callers assemble these with their buffered files.
    pub fn project_companions(&self) -> impl Iterator<Item = &Path> {
        self.project_companions
            .values()
            .flatten()
            .map(PathBuf::as_path)
    }

    /// Local defining targets attempted during traversal, whether readable or not.
    /// Watch these paths so newly created imports can invalidate failed loads.
    pub fn attempted_project_paths(&self) -> impl Iterator<Item = &Path> {
        self.attempted_project_paths.iter().map(PathBuf::as_path)
    }

    pub fn libraries(&self) -> &[ExternalLibrary] {
        &self.libraries
    }

    pub fn is_empty(&self) -> bool {
        self.libraries.is_empty()
    }

    /// Find the configured package owning this instance's defining library.
    pub(crate) fn package_library<'a>(
        &self,
        environment: &'a AnalysisEnvironment,
        library: &ExternalLibrary,
    ) -> Option<&'a super::ConfiguredPackage> {
        environment
            .config_in(library.context)?
            .package_library_containing_canonical(&library.files[0].path)
    }

    /// Return this instance's package URI when its defining file belongs to a package.
    pub(crate) fn package_uri(
        &self,
        environment: &AnalysisEnvironment,
        library: &ExternalLibrary,
    ) -> Option<String> {
        let package = self.package_library(environment, library)?;
        let relative = library.files[0].path.strip_prefix(&package.lib_root).ok()?;
        let mut uri = format!("package:{}", package.name);
        for component in relative.components() {
            uri.push('/');
            super::percent_encode(component.as_os_str().to_str()?, &mut uri);
        }
        Some(uri)
    }

    /// Whether context-specific directive targets differ from their primary instances.
    pub(crate) fn has_aliases(&self) -> bool {
        !self.aliases.is_empty()
    }

    /// Whether this instance is the first-loaded instance of its physical path.
    pub(crate) fn is_primary(&self, instance: usize) -> bool {
        self.states[self.libraries[instance].path_id as usize].primary == Some(instance)
    }

    /// Select package context or the physical library's first-load SDK context for a directive.
    pub(crate) fn resolution_context(
        &self,
        environment: &AnalysisEnvironment,
        from: &Path,
        instance: Option<usize>,
        uri: &str,
    ) -> Option<usize> {
        instance.map_or_else(
            || environment.context_index(from),
            |index| {
                let library = &self.libraries[index];
                if uri.starts_with("dart:") && self.has_aliases() {
                    self.states[library.path_id as usize].context
                } else {
                    library.context
                }
            },
        )
    }

    /// Resolve a directive using its importing instance's package and first-load SDK context.
    pub fn resolve_target(
        &self,
        environment: &AnalysisEnvironment,
        from: &Path,
        instance: Option<usize>,
        uri: &str,
    ) -> Option<ExternalTarget> {
        let context = self.resolution_context(environment, from, instance, uri);
        let path = resolve(environment, &self.canonical_paths, context, from, uri)?;
        let target = self.paths.get(&path).and_then(|&id| {
            self.aliases
                .get(&(id, context))
                .copied()
                .or(self.states[id as usize].primary)
        });
        Some(target.map_or(ExternalTarget::Path(path), ExternalTarget::Instance))
    }
}

fn resolve(
    environment: &AnalysisEnvironment,
    canonical_paths: &CanonicalPaths,
    context: Option<usize>,
    from: &Path,
    uri: &str,
) -> Option<PathBuf> {
    let target = if uri.starts_with("dart:") {
        environment.sdk_in(context)?.library(uri)?.to_path_buf()
    } else if uri.starts_with("package:") {
        environment.config_in(context)?.resolve_package_uri(uri)?
    } else if uri.contains(':') {
        return None;
    } else {
        from.parent()?.join(uri)
    };
    if let Some(canonical) = canonical_paths
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&target)
    {
        return Some(canonical.clone().unwrap_or(target));
    }
    let path = canonical_or_lexical(&target);
    let changed = (path != target).then(|| path.clone());
    canonical_paths
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(target, changed);
    Some(path)
}

fn directive_uris(file: &ExternalFile) -> impl Iterator<Item = std::borrow::Cow<'_, str>> {
    program_uris(file.program()).chain(
        file.doc_imports
            .iter()
            .map(|uri| std::borrow::Cow::Borrowed(uri.as_str())),
    )
}

fn program_uris(program: Program<'_>) -> impl Iterator<Item = std::borrow::Cow<'_, str>> {
    program
        .imports()
        .map(|import| import.uri().value())
        .chain(program.exports().map(|export| export.uri().value()))
}

fn library_dependencies(
    environment: &AnalysisEnvironment,
    canonical_paths: &CanonicalPaths,
    context: Option<usize>,
    library: &ExternalLibrary,
) -> Vec<PathBuf> {
    library
        .files
        .iter()
        .flat_map(|file| {
            directive_uris(file)
                .filter(|uri| !uri.starts_with("dart:"))
                .filter_map(move |uri| {
                    resolve(environment, canonical_paths, context, &file.path, &uri)
                })
        })
        .collect()
}

fn dependency_closures(
    edges: &[Vec<usize>],
    names: &[impl AsRef<[u32]>],
    name_count: usize,
) -> Vec<Vec<u64>> {
    let node_count = edges.len();
    let mut closures: Vec<Vec<u64>> = vec![Vec::new(); node_count];
    if name_count == 0 {
        return closures;
    }
    let word_count = name_count.div_ceil(64);
    let mut indices = vec![usize::MAX; node_count];
    let mut lowlinks = vec![0; node_count];
    let mut components = vec![usize::MAX; node_count];
    let mut merged = vec![usize::MAX; node_count];
    let mut active = Vec::with_capacity(node_count);
    let mut frames = Vec::with_capacity(node_count);
    let mut next_index = 0;
    for root in 0..node_count {
        if indices[root] != usize::MAX {
            continue;
        }
        indices[root] = next_index;
        lowlinks[root] = next_index;
        next_index += 1;
        active.push(root);
        frames.push((root, 0));
        while let Some(&(node, next_edge)) = frames.last() {
            if let Some(&dependency) = edges[node].get(next_edge) {
                frames.last_mut().unwrap().1 += 1;
                if indices[dependency] == usize::MAX {
                    indices[dependency] = next_index;
                    lowlinks[dependency] = next_index;
                    next_index += 1;
                    active.push(dependency);
                    frames.push((dependency, 0));
                } else if components[dependency] == usize::MAX {
                    lowlinks[node] = lowlinks[node].min(indices[dependency]);
                }
                continue;
            }
            frames.pop();
            if let Some(&(parent, _)) = frames.last() {
                lowlinks[parent] = lowlinks[parent].min(lowlinks[node]);
            }
            if lowlinks[node] != indices[node] {
                continue;
            }
            let start = active.iter().rposition(|&member| member == node).unwrap();
            let members = &active[start..];
            for &member in members {
                components[member] = node;
            }
            let mut closure = vec![0u64; word_count];
            for &member in members {
                for &name in names[member].as_ref() {
                    let name = name as usize;
                    closure[name / 64] |= 1u64 << (name % 64);
                }
                for &dependency in &edges[member] {
                    let component = components[dependency];
                    if component == node || merged[component] == node {
                        continue;
                    }
                    merged[component] = node;
                    for (word, &dependency_word) in closure.iter_mut().zip(&closures[component]) {
                        *word |= dependency_word;
                    }
                }
            }
            for &member in members {
                if member != node {
                    closures[member] = closure.clone();
                }
            }
            closures[node] = closure;
            active.truncate(start);
        }
    }
    closures
}

/// The libraries `program` imports or exports, plus implicit `dart:core`.
fn dependencies(
    environment: &AnalysisEnvironment,
    canonical_paths: &CanonicalPaths,
    context: Option<usize>,
    path: &Path,
    program: Program<'_>,
) -> Vec<PathBuf> {
    program_uris(program)
        .chain([std::borrow::Cow::Borrowed("dart:core")])
        .filter_map(|uri| resolve(environment, canonical_paths, context, path, &uri))
        .collect()
}

/// Read and parse the library defined at `path` with its parts; `None` when
/// the file is unreadable, exceeds the source-size limit or is itself a part.
///
/// # Errors
///
/// Returns the spawn error when a parser thread cannot start, so the load is
/// retried instead of cached as unloadable or missing a part.
fn load_library(
    environment: &AnalysisEnvironment,
    canonical_paths: &CanonicalPaths,
    path_id: u32,
    path: &Path,
    context: Option<usize>,
    origin: ExternalOrigin,
) -> std::io::Result<Option<(ExternalLibrary, Vec<PathBuf>)>> {
    // Each file parses at its package's language version (SDK: current).
    let read = |path: &Path| {
        let package_version = context.and_then(|index| {
            environment.contexts[index]
                .config
                .language_version_canonical(path)
        });
        read(path, package_version)
    };
    let Some(defining) = read(path)? else {
        return Ok(None);
    };
    if defining.program().part_of_directive().is_some() {
        return Ok(None);
    }
    let mut files = vec![defining];
    let mut seen = FxHashSet::from_iter([path.to_path_buf()]);
    let mut index = 0;
    let mut has_unresolved_parts = false;
    while index < files.len() {
        let parts: Vec<_> = files[index]
            .program()
            .part_directives()
            .filter_map(|part| {
                let resolved = resolve(
                    environment,
                    canonical_paths,
                    context,
                    &files[index].path,
                    &part.uri().value(),
                );
                has_unresolved_parts |= resolved.is_none();
                resolved
            })
            .filter(|part| seen.insert(part.clone()))
            .collect();
        for part in parts {
            files.extend(read(&part)?);
        }
        index += 1;
    }
    let dependencies =
        files
            .iter()
            .flat_map(|file| {
                dependencies(
                    environment,
                    canonical_paths,
                    context,
                    &file.path,
                    file.program(),
                )
                .into_iter()
                .chain(file.doc_imports.iter().filter_map(|uri| {
                    resolve(environment, canonical_paths, context, &file.path, uri)
                }))
            })
            .collect();
    Ok(Some((
        ExternalLibrary {
            origin,
            files: Arc::new(files),
            path_id,
            context,
            copy_context: context,
            has_unresolved_parts,
        },
        dependencies,
    )))
}

/// `None` when the source is unreadable; `Err` when the parser thread cannot start.
fn read(
    path: &Path,
    package_version: Option<LanguageVersion>,
) -> std::io::Result<Option<ExternalFile>> {
    let Ok(source) = crate::source::read_source(path) else {
        return Ok(None);
    };
    read_source(path, &source, package_version).map(Some)
}

fn read_source(
    path: &Path,
    source: &str,
    package_version: Option<LanguageVersion>,
) -> std::io::Result<ExternalFile> {
    let falcon_dart_parser::ParseResult { tree, errors } =
        try_parse_for_package(source, package_version)?;
    let program = tree.program(source);
    let has_parse_errors = !declarations_intact(program, &errors);
    let doc_imports = crate::resolve::doc_imports(program, source)
        .map(str::to_owned)
        .collect();
    Ok(ExternalFile {
        path: path.to_path_buf(),
        parsed: Arc::new(crate::ParsedFile {
            source: source.into(),
            tree,
        }),
        doc_imports,
        has_parse_errors,
    })
}

/// Whether every parse error leaves the file's declarations intact: it lies
/// inside a function or member body (bodies are never indexed), or it is a
/// language-feature gate on otherwise well-formed syntax (external sources
/// are written for their own, possibly newer, language version).
fn declarations_intact(program: Program<'_>, errors: &[ParseError]) -> bool {
    if errors.is_empty() {
        return true;
    }
    #[derive(Default)]
    struct Integrity {
        bodies: Vec<Span>,
        has_error: bool,
    }
    impl<'a> Visitor<'a> for Integrity {
        fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) {
            if matches!(declaration, TopLevelDecl::Error(_)) {
                self.has_error = true;
            } else {
                walk_top_level_decl(self, declaration);
            }
        }

        fn visit_function_decl(&mut self, function: FunctionDecl<'a>) {
            self.bodies.extend(function.body().map(|body| body.span()));
        }

        fn visit_class_member(&mut self, member: ClassMember<'a>) {
            let body = match member {
                ClassMember::Constructor(member) => member.body(),
                ClassMember::Method(member) => member.body(),
                ClassMember::Getter(member) => member.body(),
                ClassMember::Setter(member) => member.body(),
                ClassMember::Operator(member) => member.body(),
                ClassMember::PrimaryConstructorBody(member) => member.body(),
                ClassMember::Error(_) => {
                    self.has_error = true;
                    None
                }
                ClassMember::Field(_) => None,
            };
            self.bodies.extend(body.map(|body| body.span()));
        }
    }
    let mut integrity = Integrity::default();
    integrity.visit_program(program);
    if integrity.has_error {
        return false;
    }
    errors.iter().all(|error| {
        error.message.starts_with(FEATURE_GATE)
            || integrity
                .bodies
                .iter()
                .any(|body| body.start <= error.offset && error.offset < body.end)
    })
}

/// The parser's message prefix for syntax gated behind a language feature.
const FEATURE_GATE: &str = "This requires the '";

impl AnalysisEnvironment {
    /// The origin of the library defined at canonical `path` in `context`.
    fn origin(
        &self,
        context: Option<usize>,
        path: &Path,
        interner: &mut StringInterner,
    ) -> ExternalOrigin {
        if let Some(uri) = self.sdk_in(context).and_then(|sdk| sdk.library_uri(path)) {
            return ExternalOrigin::Sdk(interner.intern(uri));
        }
        self.config_in(context)
            .and_then(|config| config.package_library_containing_canonical(path))
            .map_or(ExternalOrigin::Project, |package| {
                ExternalOrigin::Package(interner.intern(&package.name))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unclassified_dependencies_are_enqueued_once_per_context() {
        let mut libraries = ExternalLibraries::default();
        let mut wave = Vec::new();
        let path = PathBuf::from("shared.dart");
        for _ in 0..100 {
            for context in [Some(0), Some(1), Some(2)] {
                assert_eq!(libraries.enqueue(path.clone(), context, &mut wave), 0);
            }
        }
        assert_eq!(
            wave,
            [
                (0, path.clone(), Some(0)),
                (0, path.clone(), Some(1)),
                (0, path, Some(2)),
            ]
        );
    }

    #[test]
    fn eviction_roots_follow_reassigned_part_owners() {
        let mut libraries = ExternalLibraries::default();
        let first = PathBuf::from("first.dart");
        let second = PathBuf::from("second.dart");
        let owner = PathBuf::from("owner.dart");
        let replacement = PathBuf::from("replacement.dart");
        libraries.record_eviction(first.clone(), owner.clone());
        libraries.record_eviction(second.clone(), owner.clone());
        libraries.record_eviction(first, replacement.clone());
        assert!(libraries.eviction_roots.contains_key(&owner));
        libraries.record_eviction(second.clone(), replacement.clone());
        assert!(!libraries.eviction_roots.contains_key(&owner));
        libraries.record_eviction(second, replacement.clone());
        assert_eq!(libraries.eviction_roots[&replacement], 2);
    }

    #[test]
    fn dependency_name_closures_follow_cycles_and_transitive_edges() {
        let edges = vec![vec![1], vec![2], vec![1, 3], vec![], vec![]];
        let names = vec![vec![0], vec![1], vec![65], vec![2], vec![3]];
        let closures = dependency_closures(&edges, &names, 66);
        assert_eq!(closures[0], vec![0b111, 0b10]);
        assert_eq!(closures[1], vec![0b110, 0b10]);
        assert_eq!(closures[2], closures[1]);
        assert_eq!(closures[3], vec![0b100, 0]);
        assert_eq!(closures[4], vec![0b1000, 0]);
        assert_eq!(
            dependency_closures(&edges, &vec![vec![]; 5], 0),
            vec![Vec::<u64>::new(); 5]
        );
    }

    #[test]
    fn oversized_import_outside_explicit_root_is_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let directory_path = directory.path().canonicalize().unwrap();
        let root = directory_path.join("selected");
        std::fs::create_dir(&root).unwrap();
        let path = root.join("main.dart");
        let source = "import '../oversized.dart';";
        std::fs::write(&path, source).unwrap();
        let imported = directory_path.join("oversized.dart");
        std::fs::File::create(&imported)
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
        assert!(read(&imported, None).unwrap().is_none());
        let error = crate::source::read_source(&imported).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("maximum supported source size"));

        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
        let parsed = falcon_dart_parser::parse(source);
        let libraries = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), parsed.tree.program(source), source)],
            |candidate| candidate.starts_with(&root),
        );
        assert!(libraries.is_empty());
        assert!(libraries.paths.contains_key(&imported));
        assert!(
            libraries
                .attempted_project_paths()
                .any(|path| path == imported)
        );
        assert!(matches!(
            libraries.resolve_target(&environment, &path, None, "../oversized.dart"),
            Some(ExternalTarget::Path(target)) if target == imported
        ));
    }

    #[test]
    fn oversized_companion_evicts_cached_directives() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("part.dart");
        std::fs::write(&path, "import 'dependency.dart';").unwrap();
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
        let mut libraries = ExternalLibraries::default();
        assert!(
            libraries
                .companion_dependencies(&environment, None, &path)
                .is_some()
        );
        std::fs::File::create(&path)
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
        assert!(
            libraries
                .companion_dependencies(&environment, None, &path)
                .is_none()
        );
        assert!(!libraries.companion_dependencies.contains_key(&path));
    }

    #[test]
    fn companion_directives_cache_refreshes_content_changes_and_failed_reads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("part.dart");
        std::fs::write(&path, "import 'first.dart'; part 'nested.dart';").unwrap();
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
        let mut libraries = ExternalLibraries::default();
        let cached = libraries
            .companion_dependencies(&environment, None, &path)
            .unwrap();
        let dependencies = cached.dependencies.as_ptr();
        let parts = cached.parts.as_ptr();
        let cached = libraries
            .companion_dependencies(&environment, None, &path)
            .unwrap();
        assert_eq!(cached.dependencies.as_ptr(), dependencies);
        assert_eq!(cached.parts.as_ptr(), parts);
        assert_eq!(cached.dependencies, [directory.path().join("first.dart")]);
        assert_eq!(cached.parts, [directory.path().join("nested.dart")]);
        // Content, not size or mtime, determines whether the cache is reusable.
        let modified = std::fs::metadata(&path).unwrap().modified().unwrap();
        std::fs::write(&path, "import 'other.dart'; part 'second.dart';").unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let cached = libraries
            .companion_dependencies(&environment, None, &path)
            .unwrap();
        assert_eq!(cached.dependencies, [directory.path().join("other.dart")]);
        assert_eq!(cached.parts, [directory.path().join("second.dart")]);
        // Different-size changes must also refresh and remove old part edges.
        std::fs::write(&path, "import 'replacement.dart';").unwrap();
        let cached = libraries
            .companion_dependencies(&environment, None, &path)
            .unwrap();
        assert_eq!(
            cached.dependencies,
            [directory.path().join("replacement.dart")]
        );
        assert!(cached.parts.is_empty());
        std::fs::remove_file(&path).unwrap();
        assert!(
            libraries
                .companion_dependencies(&environment, None, &path)
                .is_none(),
        );
        assert!(!libraries.companion_dependencies.contains_key(&path));
        std::fs::write(&path, "part 'restored.dart';").unwrap();
        let cached = libraries
            .companion_dependencies(&environment, None, &path)
            .unwrap();
        assert!(cached.dependencies.is_empty());
        assert_eq!(cached.parts, [directory.path().join("restored.dart")]);
    }

    #[test]
    fn snapshot_rebuild_evicts_unreferenced_companion_sources() {
        let directory = tempfile::tempdir().unwrap();
        let directory_path = directory.path().canonicalize().unwrap();
        let owner = directory_path.join("owner.dart");
        let part = directory_path.join("part.dart");
        let source = "part 'part.dart';";
        std::fs::write(&owner, source).unwrap();
        std::fs::write(&part, "part of 'owner.dart';").unwrap();
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([owner.as_path()], |_| None);
        let parsed = falcon_dart_parser::parse(source);
        let mut libraries = ExternalLibraries::load(
            &environment,
            &[(owner.as_path(), parsed.tree.program(source), source)],
            |path| path == owner,
        );
        assert!(libraries.companion_dependencies.contains_key(&part));
        let cached = libraries.companion_dependencies[&part].source.as_ptr();
        libraries.extend(
            &environment,
            &[(owner.as_path(), parsed.tree.program(source), source)],
            |path| path == owner,
        );
        assert_eq!(
            libraries.companion_dependencies[&part].source.as_ptr(),
            cached
        );
        let parsed = falcon_dart_parser::parse("");
        libraries.extend(
            &environment,
            &[(owner.as_path(), parsed.tree.program(""), "")],
            |path| path == owner,
        );
        assert!(libraries.companion_dependencies.is_empty());
    }

    #[test]
    fn external_initializers_retain_const_execution_and_inference_inputs() {
        let source = "class const C(final int x) { final int y = 1; }\n\
                      enum E { first; final int y = 2; }\n\
                      class Ordinary { final int y = 3; }";
        let file = read_source(Path::new("external.dart"), source, None).unwrap();
        assert!(!file.has_parse_errors);
        for declaration in file.program().declarations() {
            let mut members = match declaration {
                TopLevelDecl::Class(class) => class.members(),
                TopLevelDecl::Enum(enumeration) => enumeration.members(),
                _ => panic!("expected class or enum"),
            };
            let field = members
                .find_map(|member| match member {
                    ClassMember::Field(field) => Some(field),
                    _ => None,
                })
                .unwrap();
            assert!(field.declarators().next().unwrap().initializer().is_some());
        }
    }

    #[test]
    fn external_operator_bodies_preserve_abstract_and_concrete_members() {
        let source = "abstract class A {\n\
                        int operator +(A other);\n\
                        int operator -(A other) => 1;\n\
                      }\n\
                      int topLevel() => 0;\n\
                      class B {\n\
                        final int value = 1;\n\
                        int operator *(B other) { return value; }\n\
                      }";
        let file = read_source(Path::new("external.dart"), source, None).unwrap();
        assert!(!file.has_parse_errors);
        let TopLevelDecl::Class(class) = file.program().declarations().next().unwrap() else {
            panic!("expected class");
        };
        let ClassMember::Operator(abstract_operator) = class.members().next().unwrap() else {
            panic!("expected abstract operator");
        };
        assert!(abstract_operator.body().is_none());
        let ClassMember::Operator(operator) = class.members().nth(1).unwrap() else {
            panic!("expected operator");
        };
        assert!(operator.body().is_some());
        let TopLevelDecl::Class(class) = file.program().declarations().nth(2).unwrap() else {
            panic!("expected second class");
        };
        let ClassMember::Operator(operator) = class.members().nth(1).unwrap() else {
            panic!("expected block-bodied operator");
        };
        assert!(operator.body().is_some());
    }
}
