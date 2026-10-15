//! Loading libraries omitted from the requested project files.
//!
//! SDK, dependency and unrequested local libraries are parsed and indexed so the
//! resolver sees real declarations, but are never linted, never produce
//! diagnostics and are never handed to cross-file rules.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use falcon_dart_parser::parse_for_package;
use falcon_dart_parser::parser::ParseError;
use falcon_syntax::ast::{ClassMember, FunctionDecl, Program, TopLevelDecl, VarKeyword};
use falcon_syntax::visitor::{Visitor, walk_top_level_decl};
use rayon::prelude::*;

use super::{AnalysisEnvironment, LanguageVersion};
use crate::intern::{InternedString, StringInterner};
use crate::resolve::canonical_or_lexical;

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

/// Compact, unresolved declaration facts for one external file.
/// Executable bodies are discarded; initializers survive only for missing
/// declaration types and constant evaluation.
#[derive(Debug)]
pub struct ExternalFile {
    pub path: PathBuf,
    pub facts: Program,
    /// Library documentation imports retained after the source text is dropped.
    pub doc_imports: Vec<String>,
    /// Declaration/member ordinals of concrete operators. Their body-presence
    /// bit is not explicit in the parser AST and must survive body removal.
    pub concrete_operators: Vec<(usize, usize)>,
    pub has_parse_errors: bool,
}

/// An external library: its defining file first, then its parts.
#[derive(Debug)]
pub struct ExternalLibrary {
    pub origin: ExternalOrigin,
    pub files: Vec<ExternalFile>,
    /// At least one declared part URI could not be resolved. Resolved paths
    /// that cannot be read contribute no declarations, as in the analyzer.
    pub has_unresolved_parts: bool,
}

/// Every external library transitively imported or exported by a set of
/// project files.
#[derive(Debug, Default)]
pub struct ExternalLibraries {
    libraries: Vec<ExternalLibrary>,
    /// The package-configuration context each external file resolves
    /// `package:` URIs in.
    contexts: HashMap<PathBuf, Option<usize>>,
    /// Every defining file a load was attempted for, loaded or not.
    attempted: HashSet<PathBuf>,
    /// Local defining targets, including unreadable paths that need file watches.
    attempted_project_paths: HashSet<PathBuf>,
    /// Files evicted for open project buffers; closing one requires retraversal.
    project_evictions: HashMap<PathBuf, PathBuf>,
    project_companions: std::collections::BTreeMap<PathBuf, Vec<PathBuf>>,
    companion_dependencies: HashMap<PathBuf, CompanionDependencies>,
}

/// Source-validated directives: unchanged snapshots need not reparse unopened parts.
#[derive(Debug)]
struct CompanionDependencies {
    source: String,
    context: Option<usize>,
    dependencies: Vec<PathBuf>,
    parts: Vec<PathBuf>,
}

impl ExternalLibraries {
    /// Load every library reachable from `project` (path, program, source)
    /// through imports, exports and documentation imports, plus implicit
    /// `dart:core`. Conditional directives use their default URI.
    /// Stop at canonical project paths; each parallel worker retains only one
    /// full AST/source.
    pub fn load(
        environment: &AnalysisEnvironment,
        project: &[(&Path, &Program, &str)],
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
        project: &[(&Path, &Program, &str)],
        is_project: impl Fn(&Path) -> bool,
    ) {
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
            let mut companions = std::mem::take(&mut self.project_companions);
            companions.retain(|root, _| evictions.values().any(|owner| owner == root));
            *self = Self {
                project_evictions: evictions,
                project_companions: companions,
                companion_dependencies: std::mem::take(&mut self.companion_dependencies),
                ..Self::default()
            };
        }
        self.project_companions.retain(|root, companions| {
            companions.retain(|path| !is_project(path));
            !companions.is_empty()
                && (is_project(root) || self.project_evictions.values().any(|owner| owner == root))
        });
        let mut wave = Vec::new();
        for (path, program, source) in project {
            let context = environment.context_index(path);
            let mut dependencies = dependencies(environment, context, path, program);
            dependencies.extend(
                program
                    .part_of_directive
                    .as_ref()
                    .and_then(|part| part.uri.as_ref())
                    .and_then(|uri| resolve(environment, context, path, &uri.value)),
            );
            dependencies.extend(
                crate::resolve::doc_imports(program, source)
                    .filter_map(|uri| resolve(environment, context, path, uri)),
            );
            // Buffered defining directives own unopened parts, even at traversal boundaries.
            let root = canonical_or_lexical(path);
            self.project_companions.remove(&root);
            let mut seen = HashSet::from([root.clone()]);
            let mut parts: Vec<_> = program
                .part_directives
                .iter()
                .filter_map(|part| resolve(environment, context, path, &part.uri.value))
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
                    self.project_evictions
                        .insert(dependency.clone(), dependency.clone());
                }
                if !is_project(&dependency)
                    && !self
                        .project_evictions
                        .values()
                        .any(|root| root == &dependency)
                    && self.attempted.insert(dependency.clone())
                {
                    wave.push((dependency, context));
                }
            }
        }
        self.load_waves(environment, wave, &is_project);
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
            let file = read_source(path, &source, environment.package_language_version(path));
            let mut dependencies = dependencies(environment, context, path, &file.facts);
            dependencies.extend(
                file.doc_imports
                    .iter()
                    .filter_map(|uri| resolve(environment, context, path, uri)),
            );
            let parts = file
                .facts
                .part_directives
                .iter()
                .filter_map(|part| resolve(environment, context, path, &part.uri.value))
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

    fn load_waves(
        &mut self,
        environment: &AnalysisEnvironment,
        mut wave: Vec<(PathBuf, Option<usize>)>,
        is_project: &impl Fn(&Path) -> bool,
    ) {
        while !wave.is_empty() {
            let mut next = Vec::new();
            let loaded: Vec<_> = wave
                .into_par_iter()
                .map_init(StringInterner::default, |interner, (path, context)| {
                    let origin = environment.origin(context, &path, interner);
                    let local = matches!(origin, ExternalOrigin::Project);
                    let loaded = load_library(environment, &path, context, origin);
                    (path, local, loaded)
                })
                .collect();
            for (path, local, loaded) in loaded {
                if local {
                    self.attempted_project_paths.insert(path);
                }
                let Some((library, context, dependencies)) = loaded else {
                    continue;
                };
                let mut project_owned = false;
                for file in library.files.iter().filter(|file| is_project(&file.path)) {
                    self.project_evictions
                        .insert(file.path.clone(), library.files[0].path.clone());
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
                }
                for file in &library.files {
                    if !is_project(&file.path) {
                        self.contexts.insert(file.path.clone(), context);
                    }
                }
                for dependency in dependencies {
                    if is_project(&dependency) {
                        self.project_evictions
                            .insert(dependency.clone(), dependency.clone());
                    }
                    if !is_project(&dependency)
                        && !self
                            .project_evictions
                            .values()
                            .any(|root| root == &dependency)
                        && self.attempted.insert(dependency.clone())
                    {
                        next.push((dependency, context));
                    }
                }
                if !project_owned {
                    self.libraries.push(library);
                }
            }
            wave = next;
        }
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

    /// The logical package URI of a loaded defining file in its loading context.
    pub(crate) fn package_uri(
        &self,
        environment: &AnalysisEnvironment,
        path: &Path,
    ) -> Option<String> {
        let context = *self.contexts.get(path)?;
        let package = environment
            .config_in(context)?
            .package_library_containing(path)?;
        let relative = path.strip_prefix(&package.lib_root).ok()?;
        let mut uri = format!("package:{}", package.name);
        for component in relative.components() {
            uri.push('/');
            super::percent_encode(component.as_os_str().to_str()?, &mut uri);
        }
        Some(uri)
    }

    /// The canonical file `uri` names when written in `from` (a project or an
    /// external file); `None` when unresolvable.
    pub fn resolve_uri(
        &self,
        environment: &AnalysisEnvironment,
        from: &Path,
        uri: &str,
    ) -> Option<PathBuf> {
        let context = match self.contexts.get(from) {
            Some(context) => *context,
            None => environment.context_index(from),
        };
        resolve(environment, context, from, uri)
    }
}

fn resolve(
    environment: &AnalysisEnvironment,
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
    Some(canonical_or_lexical(&target))
}

/// The libraries `program` imports or exports, plus implicit `dart:core`.
fn dependencies(
    environment: &AnalysisEnvironment,
    context: Option<usize>,
    path: &Path,
    program: &Program,
) -> Vec<PathBuf> {
    program
        .imports
        .iter()
        .map(|import| import.uri.value.as_str())
        .chain(
            program
                .exports
                .iter()
                .map(|export| export.uri.value.as_str()),
        )
        .chain(["dart:core"])
        .filter_map(|uri| resolve(environment, context, path, uri))
        .collect()
}

/// Read and parse the library defined at `path` with its parts; `None` when
/// the file is unreadable, exceeds the source-size limit or is itself a part.
fn load_library(
    environment: &AnalysisEnvironment,
    path: &Path,
    context: Option<usize>,
    origin: ExternalOrigin,
) -> Option<(ExternalLibrary, Option<usize>, Vec<PathBuf>)> {
    // Each file parses at its package's language version (SDK: current).
    let read = |path: &Path| {
        let package_version =
            context.and_then(|index| environment.contexts[index].config.language_version(path));
        read(path, package_version)
    };
    let defining = read(path).ok()?;
    if defining.facts.part_of_directive.is_some() {
        return None;
    }
    let mut files = vec![defining];
    let mut seen = HashSet::from([path.to_path_buf()]);
    let mut index = 0;
    let mut has_unresolved_parts = false;
    while index < files.len() {
        let parts: Vec<_> = files[index]
            .facts
            .part_directives
            .iter()
            .filter_map(|part| {
                let resolved = resolve(environment, context, &files[index].path, &part.uri.value);
                has_unresolved_parts |= resolved.is_none();
                resolved
            })
            .filter(|part| seen.insert(part.clone()))
            .collect();
        files.extend(parts.iter().filter_map(|part| read(part).ok()));
        index += 1;
    }
    let dependencies = files
        .iter()
        .flat_map(|file| {
            dependencies(environment, context, &file.path, &file.facts)
                .into_iter()
                .chain(
                    file.doc_imports
                        .iter()
                        .filter_map(|uri| resolve(environment, context, &file.path, uri)),
                )
        })
        .collect();
    Some((
        ExternalLibrary {
            origin,
            files,
            has_unresolved_parts,
        },
        context,
        dependencies,
    ))
}

fn read(path: &Path, package_version: Option<LanguageVersion>) -> std::io::Result<ExternalFile> {
    let source = crate::source::read_source(path)?;
    Ok(read_source(path, &source, package_version))
}

fn read_source(
    path: &Path,
    source: &str,
    package_version: Option<LanguageVersion>,
) -> ExternalFile {
    let falcon_dart_parser::ParseResult {
        mut program,
        errors,
        ..
    } = parse_for_package(source, package_version);
    let has_parse_errors = !declarations_intact(&program, &errors);
    let doc_imports = crate::resolve::doc_imports(&program, source)
        .map(str::to_owned)
        .collect();
    let concrete_operators = compact_declarations(&mut program);
    ExternalFile {
        path: path.to_path_buf(),
        facts: program,
        doc_imports,
        concrete_operators,
        has_parse_errors,
    }
}

/// Whether every parse error leaves the file's declarations intact: it lies
/// inside a function or member body (bodies are never indexed), or it is a
/// language-feature gate on otherwise well-formed syntax (external sources
/// are written for their own, possibly newer, language version).
fn declarations_intact(program: &Program, errors: &[ParseError]) -> bool {
    if errors.is_empty() {
        return true;
    }
    #[derive(Default)]
    struct Integrity {
        bodies: Vec<falcon_syntax::ast::Span>,
        has_error: bool,
    }
    impl Visitor for Integrity {
        fn visit_top_level_decl(&mut self, declaration: &TopLevelDecl) {
            if matches!(declaration, TopLevelDecl::Error(_)) {
                self.has_error = true;
            } else {
                walk_top_level_decl(self, declaration);
            }
        }

        fn visit_function_decl(&mut self, function: &FunctionDecl) {
            self.bodies
                .extend(function.body.as_ref().map(|body| *body.span()));
        }

        fn visit_class_member(&mut self, member: &ClassMember) {
            let body = match member {
                ClassMember::Constructor(member) => member.body.as_ref(),
                ClassMember::Method(member) => member.body.as_ref(),
                ClassMember::Getter(member) => member.body.as_ref(),
                ClassMember::Setter(member) => member.body.as_ref(),
                ClassMember::Operator(member) => member.body.as_ref(),
                ClassMember::PrimaryConstructorBody(member) => member.body.as_ref(),
                ClassMember::Error(_) => {
                    self.has_error = true;
                    None
                }
                ClassMember::Field(_) => None,
            };
            self.bodies.extend(body.map(|body| *body.span()));
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

/// Move the parsed declaration headers into retained facts without cloning the
/// full tree. Preserve defaults, annotations, redirects and super-constructor
/// names. Retain missing-type initializers for signature inference, plus
/// const-constructor execution facts.
fn compact_declarations(program: &mut Program) -> Vec<(usize, usize)> {
    let mut concrete_operators = Vec::new();
    for (declaration_index, declaration) in program.declarations.iter_mut().enumerate() {
        let implicit_const = match &*declaration {
            TopLevelDecl::Class(class) => class
                .primary_constructor
                .as_ref()
                .is_some_and(|constructor| constructor.is_const),
            TopLevelDecl::MixinClass(class) => class
                .primary_constructor
                .as_ref()
                .is_some_and(|constructor| constructor.is_const),
            TopLevelDecl::Enum(_) => true,
            _ => false,
        };
        let members = match declaration {
            TopLevelDecl::Function(function) => {
                function.body = None;
                continue;
            }
            TopLevelDecl::Variable(variable) => {
                if variable.keyword != VarKeyword::Const && variable.var_type.is_some() {
                    for declarator in &mut variable.declarators {
                        declarator.initializer = None;
                    }
                }
                continue;
            }
            TopLevelDecl::Class(class) => &mut class.members,
            TopLevelDecl::MixinClass(class) => &mut class.members,
            TopLevelDecl::Mixin(mixin) => &mut mixin.members,
            TopLevelDecl::Enum(enumeration) => &mut enumeration.members,
            TopLevelDecl::Extension(extension) => &mut extension.members,
            TopLevelDecl::ExtensionType(extension) => &mut extension.members,
            _ => continue,
        };
        let has_const_constructor = implicit_const
            || members.iter().any(|member| {
                matches!(member, ClassMember::Constructor(constructor)
                if constructor.is_const && !constructor.is_external)
            });
        for (member_index, member) in members.iter_mut().enumerate() {
            match member {
                ClassMember::Constructor(member) => {
                    member.body = None;
                    if member.is_const && !member.is_external {
                        continue;
                    }
                    let super_call = std::mem::take(&mut member.initializers)
                        .into_iter()
                        .find(|initializer| {
                            matches!(
                                initializer,
                                falcon_syntax::ast::ConstructorInitializer::SuperCall { .. }
                            )
                        })
                        .map(|mut initializer| {
                            if let falcon_syntax::ast::ConstructorInitializer::SuperCall {
                                args,
                                ..
                            } = &mut initializer
                            {
                                args.positional = Vec::new();
                                args.named = Vec::new();
                            }
                            initializer
                        });
                    member.initializers = super_call.into_iter().collect();
                }
                ClassMember::Method(member) => member.body = None,
                ClassMember::Getter(member) => member.body = None,
                ClassMember::Setter(member) => member.body = None,
                ClassMember::PrimaryConstructorBody(member) => member.body = None,
                ClassMember::Operator(member) => {
                    if member.body.is_some() {
                        concrete_operators.push((declaration_index, member_index));
                    }
                    member.body = None;
                }
                ClassMember::Field(field)
                    if field.keyword != VarKeyword::Const
                        && field.field_type.is_some()
                        && (field.is_static || !has_const_constructor) =>
                {
                    for declarator in &mut field.declarators {
                        declarator.initializer = None;
                    }
                }
                ClassMember::Field(_) | ClassMember::Error(_) => {}
            }
        }
    }
    concrete_operators
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
            .and_then(|config| config.package_library_containing(path))
            .map_or(ExternalOrigin::Project, |package| {
                ExternalOrigin::Package(interner.intern(&package.name))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_import_outside_explicit_root_is_unavailable() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().join("selected");
        std::fs::create_dir(&root).unwrap();
        let path = root.join("main.dart");
        let source = "import '../oversized.dart';";
        std::fs::write(&path, source).unwrap();
        let imported = directory.path().join("oversized.dart");
        std::fs::File::create(&imported)
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
        let error = read(&imported, None).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("maximum supported source size"));

        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
        let parsed = falcon_dart_parser::parse(source);
        let libraries = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), &parsed.program, source)],
            |candidate| candidate.starts_with(&root),
        );
        assert!(libraries.is_empty());
        assert!(libraries.attempted.contains(&imported));
        assert!(
            libraries
                .attempted_project_paths()
                .any(|path| path == imported)
        );
        assert_eq!(
            libraries.resolve_uri(&environment, &path, "../oversized.dart"),
            Some(imported),
        );
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
    fn const_primary_and_implicit_enum_fields_survive_compaction() {
        let falcon_dart_parser::ParseResult {
            mut program,
            errors,
            ..
        } = falcon_dart_parser::parse(
            "class const C(final int x) { final int y = 1; }\n\
             enum E { first; final int y = 2; }\n\
             class Ordinary { final int y = 3; }",
        );
        assert!(errors.is_empty(), "{errors:?}");
        compact_declarations(&mut program);
        for (index, declaration) in program.declarations.iter().enumerate() {
            let members = match declaration {
                TopLevelDecl::Class(class) => &class.members,
                TopLevelDecl::Enum(enumeration) => &enumeration.members,
                _ => panic!("expected class or enum"),
            };
            let field = members
                .iter()
                .find_map(|member| match member {
                    ClassMember::Field(field) => Some(field),
                    _ => None,
                })
                .unwrap();
            assert_eq!(field.declarators[0].initializer.is_some(), index < 2);
        }
    }
}
