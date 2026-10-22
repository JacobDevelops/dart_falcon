//! Conservative identity resolution for expression identifiers.
//!
//! This is intentionally smaller than a Dart element model. It answers only
//! whether a simple or import-prefixed name denotes a type, a value, or cannot
//! be resolved uniquely. Ambiguity and incomplete bindings become
//! [`NameIdentity::Unknown`] without suppressing unrelated external names.
//! Missing project part files preserve explicit, unambiguous types both inside
//! their declaring library and through imports unless they overlap `dart:core`;
//! unresolvable external part URIs remain uncertain.
//! A top-level getter and setter share one value identity, including across parts;
//! repeated accessors and collisions with other declarations remain ambiguous.

use crate::InternedString;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

// External IDs occupy a disjoint namespace, independent of project file count.
const EXTERNAL_ID: usize = 1 << (usize::BITS - 1);

use crate::{NodeRef, ParsedFile};
use falcon_syntax::cst::*;

use super::group_libraries;
use super::library::canonical_or_lexical;
use crate::environment::{AnalysisEnvironment, ExternalLibraries, ExternalOrigin, ExternalTarget};
use crate::intern::StringInterner;
use crate::memo::NameMemo;
use rustc_hash::{FxHashMap, FxHashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NameIdentity {
    Type,
    Value,
    Unknown,
}

/// What a simple or import-prefixed name denotes in a library's
/// documentation-comment scope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocName {
    /// A top-level type (`kind` [`NameIdentity::Type`]) or value declaration;
    /// `identity` is `None` when it is not unique (ambiguous or colliding
    /// declarations), `kind` unknown when ambiguous between namespaces.
    Declaration {
        identity: Option<DeclarationIdentity>,
        kind: NameIdentity,
    },
    /// A named extension, which has no identity.
    Extension,
    /// An import prefix (`@docImport` prefixes are not prefixes).
    Prefix,
    /// Provably nothing in scope.
    Absent,
    /// Some namespace in scope is not fully indexed.
    Unknown,
}

pub struct IdentitySource<'a> {
    pub path: &'a Path,
    pub parsed: &'a Arc<ParsedFile>,
    pub has_parse_errors: bool,
}

#[derive(Debug, Clone)]
pub struct PackageIdentity {
    pub name: String,
    pub lib_root: PathBuf,
}

/// A library (a defining file plus its parts) of an [`IdentityIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LibraryId(usize);

/// A source file of an [`IdentityIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(usize);

/// A simple name or an import-prefixed name. Longer dotted names are member
/// accesses and have no declaration identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QualifiedName<'a> {
    pub prefix: Option<&'a str>,
    pub name: &'a str,
}

impl<'a> QualifiedName<'a> {
    pub fn prefixed(prefix: &'a str, name: &'a str) -> Self {
        Self {
            prefix: Some(prefix),
            name,
        }
    }

    pub fn from_segments<S: AsRef<str>>(segments: &'a [S]) -> Option<Self> {
        match segments {
            [name] => Some(name.as_ref().into()),
            [prefix, name] => Some(Self::prefixed(prefix.as_ref(), name.as_ref())),
            _ => None,
        }
    }

    pub fn from_identifiers(segments: impl IntoIterator<Item = Identifier<'a>>) -> Option<Self> {
        let mut segments = segments.into_iter();
        let first = segments.next()?.name();
        let second = segments.next();
        if segments.next().is_some() {
            return None;
        }
        Some(match second {
            Some(name) => Self::prefixed(first, name.name()),
            None => first.into(),
        })
    }
}

impl<'a> From<&'a str> for QualifiedName<'a> {
    fn from(name: &'a str) -> Self {
        Self { prefix: None, name }
    }
}

impl<'a> From<&'a String> for QualifiedName<'a> {
    fn from(name: &'a String) -> Self {
        name.as_str().into()
    }
}

#[derive(Debug, Clone)]
struct Library {
    declarations: FxHashMap<InternedString, (NameIdentity, FileId)>,
    imports: Vec<Import>,
    exports: Vec<Export>,
    configurable: bool,
    incomplete: bool,
    missing_parts_only: bool,
    missing_generated_parts_only: bool,
    implicit_core: bool,
    /// `None` for project libraries.
    origin: Option<ExternalOrigin>,
    /// The targets of the `@docImport`s of the library documentation, in
    /// order: their export namespaces back documentation-comment lookups
    /// that the library scope misses.
    doc_imports: Vec<ImportTarget>,
    /// Names in the library's separate named-extension namespace.
    extensions: HashSet<InternedString>,
    /// `package:<name>/<path>` of the defining file when it lies inside a known
    /// package's `lib` directory.
    uri: Option<String>,
    /// The package copy; see [`DeclarationIdentity::Package`].
    copy: Option<u32>,
    named: bool,
}

#[derive(Debug, Clone)]
struct Import {
    target: ImportTarget,
    prefix: Option<String>,
    combinators: Combinators,
}

#[derive(Debug, Clone)]
struct Export {
    target: ImportTarget,
    combinators: Combinators,
}

/// The intersection of all show lists, minus hides; without a show, the union of hides.
#[derive(Debug, Clone)]
enum Combinators {
    Show(Vec<InternedString>),
    Hide(Vec<InternedString>),
}

/// SDK and package identities use static builtin or index-owned shared names.
/// Without loaded external sources, index resolution falls back to allowlists.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeclarationIdentity {
    Library {
        library: LibraryId,
        name: InternedString,
    },
    Sdk {
        library: InternedString,
        name: InternedString,
    },
    Package {
        package: InternedString,
        name: InternedString,
        /// Which library-root/context copy of the package this is, in sorted
        /// order; `None` when only one copy is loaded.
        copy: Option<u32>,
    },
}

impl DeclarationIdentity {
    pub const OBJECT: Self = Self::sdk("dart:core", "Object");

    pub const fn sdk(library: &'static str, name: &'static str) -> Self {
        Self::Sdk {
            library: InternedString::Static(library),
            name: InternedString::Static(name),
        }
    }

    pub const fn package(package: &'static str, name: &'static str) -> Self {
        Self::Package {
            package: InternedString::Static(package),
            name: InternedString::Static(name),
            copy: None,
        }
    }

    pub const fn flutter(name: &'static str) -> Self {
        Self::package("flutter", name)
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Library { name, .. } => name,
            Self::Sdk { name, .. } | Self::Package { name, .. } => name,
        }
    }

    pub fn is_sdk(&self, library: &str, name: &str) -> bool {
        matches!(self, Self::Sdk { library: found_library, name: found_name }
            if *found_library == library && *found_name == name)
    }

    pub fn is_package(&self, package: &str, name: &str) -> bool {
        matches!(self, Self::Package { package: found_package, name: found_name, .. }
            if *found_package == package && *found_name == name)
    }

    /// Whether `self` is `target`; a package `target` without a copy names
    /// the declaration in whichever copy of its package `self` is from.
    pub fn matches(&self, target: &Self) -> bool {
        match target {
            Self::Package {
                package,
                name,
                copy: None,
            } => self.is_package(package, name),
            _ => self == target,
        }
    }

    /// The analyzer's `ElementAnnotation.isImmutable`: meta's `immutable`
    /// getter or its `Immutable` constructor.
    pub fn is_meta_immutable(&self) -> bool {
        self.is_package("meta", "immutable") || self.is_package("meta", "Immutable")
    }
}

#[derive(Debug, Clone)]
enum ImportTarget {
    Library(LibraryId),
    Sdk(String),
    Package(String),
    Unknown,
}
/// The declaration category of a source annotation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MemberSelectorKind {
    /// The owner itself, including a top-level function or getter; name is empty.
    Declaration,
    /// A top-level variable; name is empty because the owner identifies it.
    Variable,
    /// An instance or static method/getter, with its unencoded source name.
    Read,
    /// A setter, with its source name (without `=`), or empty for a top-level setter.
    Write,
    /// An instance or static field, with its source name.
    Field,
    /// A constructor, named `new` for the unnamed constructor.
    Constructor,
    /// An operator's source token and explicit parameter count.
    Operator(usize),
}

/// An allocation-free source member identity, independent of encoded key spelling.
///
/// Top-level targets always use `is_static = false` and an empty name: their
/// owner already identifies the declaration. Member targets borrow the source
/// name and distinguish static access explicitly. Constructors and operators
/// also use `is_static = false`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemberSelector<'a> {
    pub kind: MemberSelectorKind,
    pub is_static: bool,
    pub name: &'a str,
}

impl<'a> MemberSelector<'a> {
    pub const fn new(kind: MemberSelectorKind, is_static: bool, name: &'a str) -> Self {
        Self {
            kind,
            is_static,
            name,
        }
    }

    pub const DECLARATION: Self = Self::new(MemberSelectorKind::Declaration, false, "");
}

/// A member or one of its formal parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSelector<'a> {
    pub member: MemberSelector<'a>,
    pub parameter: Option<&'a str>,
}

impl<'a> From<MemberSelector<'a>> for SourceSelector<'a> {
    fn from(member: MemberSelector<'a>) -> Self {
        Self {
            member,
            parameter: None,
        }
    }
}

type AnnotationData = (FileId, Vec<NodeRef>);

#[derive(Debug, Clone, Default)]
struct MemberAnnotations {
    annotations: Option<AnnotationData>,
    parameters: HashMap<String, (Option<usize>, AnnotationData)>,
}

type DeclarationAnnotations =
    HashMap<(MemberSelectorKind, bool), HashMap<String, MemberAnnotations>>;
type SourceAnnotations = HashMap<LibraryId, HashMap<String, DeclarationAnnotations>>;

/// Project-wide library/name identity index.
#[derive(Debug, Clone, Default)]
pub struct IdentityIndex {
    libraries: Vec<Library>,
    file_library: Vec<LibraryId>,
    by_path: HashMap<PathBuf, FileId>,
    /// File ranges for external instances, in loader order, including an end sentinel.
    external_file_offsets: Vec<usize>,
    /// Source paths indexed by [`FileId`], including non-primary instances.
    paths: Vec<Option<PathBuf>>,
    parsed: Vec<Arc<ParsedFile>>,
    source_annotations: SourceAnnotations,
    /// The loaded `dart:core`, when external libraries are indexed.
    core: Option<LibraryId>,
    /// Names declared by multiple libraries within one copy of a package
    /// origin; these require library-scoped identities instead of package-wide ones.
    colliding: FxHashMap<ExternalOrigin, FxHashMap<Option<u32>, FxHashSet<InternedString>>>,
    /// Every indexed `typedef`, by the identity of its name.
    aliases: HashMap<DeclarationIdentity, TypeAlias>,
    /// Reverse source context for uniquely identified external declarations.
    declaration_libraries: FxHashMap<DeclarationIdentity, LibraryId>,
    /// Memoized answers of the pure lookups below.
    memos: LookupMemos,
    /// The project packages directive URIs resolve against.
    packages: Vec<PackageIdentity>,
    external: Option<Arc<IdentityIndex>>,
}

/// Memoized lookups. The index is immutable once built, so answers never go
/// stale; clones start empty. Only complete traversals are cached, never
/// answers shortened by a caller's cycle guard.
#[derive(Debug, Default, Clone)]
struct LookupMemos {
    /// Top-level [`IdentityIndex::exported_declaration`] answers.
    exports: NameMemo<(LibraryId, NameIdentity), Option<DeclarationIdentity>>,
    /// [`IdentityIndex::target_may_hide_declaration`] for library targets.
    uncertainty: NameMemo<LibraryId, bool>,
    /// [`IdentityIndex::resolve_declaration`], keyed by prefix and name.
    declarations: NameMemo<LibraryId, Option<DeclarationIdentity>>,
    /// [`IdentityIndex::resolve_value_declaration`], keyed by prefix and name.
    values: NameMemo<LibraryId, Option<DeclarationIdentity>>,
    /// [`IdentityIndex::resolve_extension_declaration`], keyed by prefix and name.
    named_extensions: NameMemo<LibraryId, Option<DeclarationIdentity>>,
    /// Complete top-level [`IdentityIndex::exported_extension`] traversals.
    extension_exports: NameMemo<LibraryId, Result<Option<DeclarationIdentity>, ()>>,
    /// [`IdentityIndex::resolve`], keyed by prefix and name.
    identities: NameMemo<LibraryId, NameIdentity>,
    /// [`IdentityIndex::extension_visible`] by importing and declaring
    /// library, qualified by the extension name; the name part is empty.
    extensions: NameMemo<(LibraryId, LibraryId), bool>,
}

/// A `typedef Name<T> = Type;` declaration (function-type typedefs included).
#[derive(Debug, Clone)]
pub(crate) struct TypeAlias {
    pub(crate) file: FileId,
    pub(crate) library: LibraryId,
    node: NodeRef,
    pub(crate) templates: super::semantic::AliasTemplates,
}

impl TypeAlias {
    pub(crate) fn type_params(&self) -> AstChildren<'_, TypeParam<'_>> {
        self.node.cast::<TypeAliasDecl<'_>>().type_params()
    }

    pub(crate) fn aliased(&self) -> DartType<'_> {
        self.node.cast::<TypeAliasDecl<'_>>().aliased()
    }
}

/// Loaded SDK and dependency libraries to index next to the project files.
#[derive(Clone, Copy)]
pub struct ExternalSources<'a> {
    pub libraries: &'a ExternalLibraries,
    pub environment: &'a AnalysisEnvironment,
}

/// Translate source-graph targets into IDs without losing external instances.
struct LibraryResolver<'a> {
    by_path: &'a HashMap<PathBuf, FileId>,
    file_library: &'a [LibraryId],
    packages: &'a [PackageIdentity],
    external: Option<ExternalSources<'a>>,
    base: Option<&'a IdentityIndex>,
    external_file_offsets: &'a [usize],
    project_library_count: usize,
}

impl LibraryResolver<'_> {
    fn resolve(&self, from: &Path, instance: Option<usize>, uri: &str) -> ImportTarget {
        let fallback = if instance.is_none() {
            resolve_uri(from, uri, self.by_path, self.file_library, self.packages)
        } else if uri.starts_with("dart:") {
            ImportTarget::Sdk(uri.to_string())
        } else if let Some((package, _)) = uri
            .strip_prefix("package:")
            .and_then(|rest| rest.split_once('/'))
            .filter(|(package, _)| {
                !self
                    .packages
                    .iter()
                    .any(|candidate| candidate.name == *package)
            })
        {
            ImportTarget::Package(package.to_string())
        } else {
            ImportTarget::Unknown
        };
        if instance.is_none()
            && matches!(fallback, ImportTarget::Library(id) if id.0 < self.project_library_count
                || self.external.is_none_or(|external| !external.libraries.has_aliases()))
        {
            return fallback;
        }
        self.external
            .and_then(|external| {
                external
                    .libraries
                    .resolve_target(external.environment, from, instance, uri)
            })
            .and_then(|target| self.library(target))
            .map_or(fallback, ImportTarget::Library)
    }

    fn doc_uri_missing(&self, from: &Path, instance: usize, uri: &str) -> bool {
        let Some(external) = self.external else {
            return false;
        };
        if let Some(rest) = uri.strip_prefix("package:") {
            let context = external.libraries.libraries()[instance].context;
            let package = rest.split('/').next().unwrap_or(rest);
            return external
                .environment
                .config_in(context)
                .is_some_and(|config| config.package(package).is_none());
        }
        if uri.starts_with("dart:") {
            let context = external.libraries.resolution_context(
                external.environment,
                from,
                Some(instance),
                uri,
            );
            return external
                .environment
                .sdk_in(context)
                .is_some_and(|sdk| sdk.library(uri).is_none());
        }
        uri_missing(external.environment, from, uri)
    }

    fn library(&self, target: ExternalTarget) -> Option<LibraryId> {
        match target {
            ExternalTarget::Instance(instance) => {
                if let Some(base) = self.base {
                    base.external_library(instance)
                } else {
                    let file = *self.external_file_offsets.get(instance)?;
                    self.file_library.get(file).copied()
                }
            }
            ExternalTarget::Path(path) => self
                .by_path
                .get(&path)
                .map(|file| self.file_library[file.0])
                .or_else(|| {
                    let base = self.base?;
                    base.file_canonical(&path).map(|file| base.library(file))
                }),
        }
    }
}

struct PackageInstance<'a> {
    library: LibraryId,
    package: &'a str,
    root: &'a Path,
    path: &'a Path,
    context: Option<usize>,
}

/// A root needs context-specific copies only when one of its paths splits.
fn assign_package_copies(
    libraries: &mut [Library],
    instances: &mut [PackageInstance<'_>],
    has_aliases: bool,
) {
    let mut split_roots = FxHashSet::default();
    if has_aliases {
        let mut paths = FxHashMap::default();
        for instance in instances.iter() {
            if paths
                .insert((instance.path, instance.package, instance.root), ())
                .is_some()
            {
                split_roots.insert((instance.package, instance.root));
            }
        }
    }
    let mut package_roots: FxHashMap<&str, BTreeSet<(&Path, Option<usize>)>> = FxHashMap::default();
    for instance in instances.iter_mut() {
        if !split_roots.contains(&(instance.package, instance.root)) {
            instance.context = None;
        }
        package_roots
            .entry(instance.package)
            .or_default()
            .insert((instance.root, instance.context));
    }
    let copies: FxHashMap<(&str, &Path, Option<usize>), u32> = package_roots
        .iter()
        .filter(|(_, roots)| roots.len() > 1)
        .flat_map(|(&package, roots)| {
            (0..)
                .zip(roots)
                .map(move |(copy, &(root, context))| ((package, root, context), copy))
        })
        .collect();
    for instance in instances {
        libraries[instance.library.0].copy = copies
            .get(&(instance.package, instance.root, instance.context))
            .copied();
    }
}

impl IdentityIndex {
    pub fn node_ref(&self, file: FileId, id: NodeId) -> NodeRef {
        NodeRef::new(Arc::clone(self.parsed_file(file)), id)
    }

    pub(crate) fn parsed_file(&self, file: FileId) -> &Arc<ParsedFile> {
        if file.0 & EXTERNAL_ID != 0
            && let Some(external) = &self.external
        {
            return external.parsed_file(file);
        }
        &self.parsed[file.0 & !EXTERNAL_ID]
    }

    pub fn from_project_files(files: &[IdentitySource<'_>], packages: &[PackageIdentity]) -> Self {
        Self::build(files, &[], packages, None)
    }

    /// Index project `files` plus, when given, loaded external libraries.
    /// External declarations use canonical SDK identities and package identities
    /// when unique within their package, otherwise their defining library identity.
    ///
    /// `sources` holds the text of each of `files`, in order, so `@docImport`s
    /// in library documentation can be indexed; pass `&[]` to skip them.
    pub fn build(
        files: &[IdentitySource<'_>],
        sources: &[&str],
        packages: &[PackageIdentity],
        external: Option<ExternalSources<'_>>,
    ) -> Self {
        Self::build_layer(files, sources, packages, external, None, None)
    }

    /// Build the immutable external layer once, with edit-stable file/library IDs.
    pub fn build_external(external: ExternalSources<'_>) -> Self {
        let mut index = Self::build(&[], &[], &[], Some(external));
        for library in &mut index.file_library {
            library.0 |= EXTERNAL_ID;
        }
        for file in index.by_path.values_mut() {
            file.0 |= EXTERNAL_ID;
        }
        for offset in &mut index.external_file_offsets {
            *offset |= EXTERNAL_ID;
        }
        for library in &mut index.libraries {
            for (_, file) in library.declarations.values_mut() {
                file.0 |= EXTERNAL_ID;
            }
            for target in library
                .imports
                .iter_mut()
                .map(|import| &mut import.target)
                .chain(library.exports.iter_mut().map(|export| &mut export.target))
                .chain(library.doc_imports.iter_mut())
            {
                if let ImportTarget::Library(library) = target {
                    library.0 |= EXTERNAL_ID;
                }
            }
        }
        for alias in index.aliases.values_mut() {
            alias.file.0 |= EXTERNAL_ID;
            alias.library.0 |= EXTERNAL_ID;
        }
        let external_identity = |mut identity: DeclarationIdentity| {
            if let DeclarationIdentity::Library { library, .. } = &mut identity {
                library.0 |= EXTERNAL_ID;
            }
            identity
        };
        index.aliases = index
            .aliases
            .into_iter()
            .map(|(identity, alias)| (external_identity(identity), alias))
            .collect();
        for library in index.declaration_libraries.values_mut() {
            library.0 |= EXTERNAL_ID;
        }
        if let Some(core) = &mut index.core {
            core.0 |= EXTERNAL_ID;
        }
        index.memos = LookupMemos::default();
        index
    }

    /// Refresh project facts without copying or re-indexing the external layer.
    pub fn with_external(
        files: &[IdentitySource<'_>],
        sources: &[&str],
        packages: &[PackageIdentity],
        external: ExternalSources<'_>,
        base: Arc<Self>,
    ) -> Self {
        Self::build_layer(files, sources, packages, Some(external), Some(base), None)
    }

    /// Build project facts using an optional immutable external index and grouping.
    /// `base` must come from `build_external`; `grouping` must describe `files`
    /// in the same order, including any promoted companions.
    pub fn build_layer(
        files: &[IdentitySource<'_>],
        sources: &[&str],
        packages: &[PackageIdentity],
        external: Option<ExternalSources<'_>>,
        base: Option<Arc<Self>>,
        grouping: Option<&super::LibraryGrouping>,
    ) -> Self {
        let owned_grouping = grouping.is_none().then(|| {
            let files: Vec<_> = files
                .iter()
                .map(|file| (file.path.to_path_buf(), file.parsed.program()))
                .collect();
            group_libraries(&files)
        });
        let grouping = grouping.or(owned_grouping.as_ref()).unwrap();
        debug_assert_eq!(grouping.len(), files.len());

        let mut root_to_library = HashMap::new();
        let mut libraries = Vec::<Library>::new();
        let mut file_library: Vec<LibraryId> = (0..files.len())
            .map(|i| {
                let root = grouping.siblings(i).iter().copied().fold(i, usize::min);
                *root_to_library.entry(root).or_insert_with(|| {
                    libraries.push(Library {
                        declarations: FxHashMap::default(),
                        imports: Vec::new(),
                        exports: Vec::new(),
                        configurable: false,
                        incomplete: grouping.is_unresolved(i),
                        missing_parts_only: grouping.is_unresolved(i),
                        missing_generated_parts_only: grouping.missing_generated_parts_only(i),
                        implicit_core: true,
                        origin: None,
                        doc_imports: Vec::new(),
                        extensions: HashSet::new(),
                        uri: None,
                        copy: None,
                        named: false,
                    });
                    LibraryId(libraries.len() - 1)
                })
            })
            .collect();
        // Every indexed file: its path, program and parse-error flag.
        let mut all: Vec<(&Path, Program<'_>, bool)> = files
            .iter()
            .map(|file| (file.path, file.parsed.program(), file.has_parse_errors))
            .collect();
        let mut parsed: Vec<_> = files.iter().map(|file| Arc::clone(file.parsed)).collect();
        let mut core = base.as_ref().and_then(|index| index.core);
        let mut package_copies = Vec::new();
        let external_library_start = libraries.len();
        let external_count = external
            .filter(|_| base.is_none())
            .map_or(0, |external| external.libraries.libraries().len());
        let mut external_file_offsets =
            Vec::with_capacity(external_count + usize::from(external_count != 0));
        for library in external
            .iter()
            .filter(|_| base.is_none())
            .flat_map(|external| external.libraries.libraries())
        {
            let id = LibraryId(libraries.len());
            external_file_offsets.push(all.len());
            libraries.push(Library {
                declarations: FxHashMap::default(),
                imports: Vec::new(),
                exports: Vec::new(),
                configurable: false,
                incomplete: library.has_unresolved_parts,
                missing_parts_only: false,
                missing_generated_parts_only: false,
                implicit_core: true,
                origin: (!matches!(library.origin, ExternalOrigin::Project))
                    .then(|| library.origin.clone()),
                doc_imports: Vec::new(),
                extensions: HashSet::new(),
                uri: external.and_then(|external| {
                    external
                        .libraries
                        .package_uri(external.environment, library)
                }),
                copy: None,
                named: false,
            });
            if matches!(&library.origin, ExternalOrigin::Sdk(uri) if uri.as_str() == "dart:core") {
                core = Some(id);
            }
            if let ExternalOrigin::Package(package) = &library.origin
                && let Some(configured) = external.and_then(|external| {
                    external
                        .libraries
                        .package_library(external.environment, library)
                })
            {
                package_copies.push(PackageInstance {
                    library: id,
                    package: package.as_str(),
                    root: configured.lib_root.as_path(),
                    path: &library.files[0].path,
                    context: library.copy_context,
                });
            }
            for file in library.files.iter() {
                file_library.push(id);
                all.push((&file.path, file.program(), file.has_parse_errors));
                parsed.push(Arc::clone(&file.parsed));
            }
        }
        if !external_file_offsets.is_empty() {
            external_file_offsets.push(all.len());
        }
        assign_package_copies(
            &mut libraries,
            &mut package_copies,
            external.is_some_and(|external| external.libraries.has_aliases()),
        );

        let mut interner = StringInterner::default();
        let mut by_path = HashMap::new();
        let mut paths = Vec::with_capacity(all.len());
        // Accessors share one value declaration, unlike duplicate variables or
        // functions. Keep these flags only while collecting declarations.
        let mut accessors = FxHashMap::default();
        let mut previous_library = None;
        let mut primary = true;
        for (index, (&(path, program, has_parse_errors), &library)) in
            all.iter().zip(&file_library).enumerate()
        {
            let canonical = canonical_or_lexical(path);
            if previous_library != Some(library) {
                previous_library = Some(library);
                primary = library
                    .0
                    .checked_sub(external_library_start)
                    .is_none_or(|instance| {
                        external.is_some_and(|external| {
                            !external.libraries.has_aliases()
                                || external.libraries.is_primary(instance)
                        })
                    });
            }
            if primary {
                by_path.insert(canonical.clone(), FileId(index));
            }
            paths.push(Some(canonical));
            let library_id = library;
            let library = &mut libraries[library.0];
            library.incomplete |= has_parse_errors;
            library.configurable |= program
                .imports()
                .any(|node| node.configurable_uris().next().is_some())
                || program
                    .exports()
                    .any(|node| node.configurable_uris().next().is_some());
            library.missing_parts_only &= !has_parse_errors;
            library.missing_generated_parts_only &= !has_parse_errors;
            library.named |= program
                .library_directive()
                .is_some_and(|directive| directive.name().next().is_some());
            if library.origin.is_none() && program.part_of_directive().is_none() {
                library.uri = package_uri(path, packages);
            }
            for declaration in program.declarations() {
                if let TopLevelDecl::Extension(extension) = declaration
                    && let Some(name) = extension.name()
                {
                    library.extensions.insert(interner.intern(name.name()));
                }
                declaration_identities(declaration, |name, identity| {
                    let name = interner.intern(name);
                    let accessor = match declaration {
                        TopLevelDecl::Function(function) => match function.kind() {
                            FunctionKind::Getter => 1u8,
                            FunctionKind::Setter(_) => 2u8,
                            _ => 0,
                        },
                        _ => 0,
                    };
                    if accessor != 0 {
                        let flags = accessors.entry((library_id, name.clone())).or_insert(0);
                        let pair = *flags != 0 && *flags & accessor == 0;
                        *flags |= accessor;
                        if pair
                            && let Some((kind, file)) = library.declarations.get_mut(&name)
                            && *kind == NameIdentity::Value
                        {
                            if accessor == 1 {
                                *file = FileId(index);
                            }
                            return;
                        }
                    }
                    merge(&mut library.declarations, name, identity, FileId(index));
                });
            }
        }

        let resolver = LibraryResolver {
            by_path: &by_path,
            file_library: &file_library,
            packages,
            external,
            base: base.as_deref(),
            external_file_offsets: &external_file_offsets,
            project_library_count: external_library_start,
        };
        for (&(path, program, _), &library) in all.iter().zip(&file_library) {
            let external_instance = library.0.checked_sub(external_library_start);
            let mut imports = Vec::with_capacity(program.imports().len());
            let mut implicit_core = true;
            for import in program.imports() {
                let uri = import.uri().value();
                if uri == "dart:core" {
                    implicit_core = false;
                }
                imports.push(Import {
                    target: resolver.resolve(path, external_instance, &uri),
                    prefix: import.as_name().map(|id| id.name().to_owned()),
                    combinators: Combinators::collect(import.combinators(), &mut interner),
                });
            }
            let exports = program.exports().map(|export| Export {
                target: resolver.resolve(path, external_instance, &export.uri().value()),
                combinators: Combinators::collect(export.combinators(), &mut interner),
            });
            let library = &mut libraries[library.0];
            library.implicit_core &= implicit_core;
            library.imports.extend(imports);
            library.exports.extend(exports);
        }
        drop(interner);
        for ((file, source), &library) in files.iter().zip(sources).zip(&file_library) {
            // A doc import of a library that does not exist contributes nothing.
            let targets = doc_imports(file.parsed.program(), source)
                .filter(|uri| {
                    !external
                        .is_some_and(|external| uri_missing(external.environment, file.path, uri))
                })
                .map(|uri| resolver.resolve(file.path, None, uri));
            libraries[library.0].doc_imports.extend(targets);
        }
        if base.is_none()
            && let Some(external) = external
        {
            for (instance, library) in external.libraries.libraries().iter().enumerate() {
                let id = file_library[external_file_offsets[instance]];
                for file in library.files.iter() {
                    let targets = file
                        .doc_imports
                        .iter()
                        .filter(|uri| !resolver.doc_uri_missing(&file.path, instance, uri))
                        .map(|uri| resolver.resolve(&file.path, Some(instance), uri));
                    libraries[id.0].doc_imports.extend(targets);
                }
            }
        }
        let mut declared_by = FxHashMap::default();
        for instance in &package_copies {
            let library = &libraries[instance.library.0];
            let Some(origin) = &library.origin else {
                continue;
            };
            for name in library.declarations.keys().chain(library.extensions.iter()) {
                declared_by
                    .entry((origin, instance.root, name))
                    .and_modify(|(first, colliding)| {
                        *colliding |= *first != instance.path;
                    })
                    .or_insert((instance.path, false));
            }
        }
        let mut colliding: FxHashMap<ExternalOrigin, FxHashMap<Option<u32>, FxHashSet<_>>> =
            FxHashMap::default();
        let declaration_capacity = declared_by.len();
        for instance in &package_copies {
            let library = &libraries[instance.library.0];
            let Some(origin) = &library.origin else {
                continue;
            };
            for name in library.declarations.keys().chain(library.extensions.iter()) {
                if declared_by[&(origin, instance.root, name)].1 {
                    colliding
                        .entry(origin.clone())
                        .or_default()
                        .entry(library.copy)
                        .or_default()
                        .insert(name.clone());
                }
            }
        }

        let mut index = Self {
            libraries,
            file_library,
            by_path,
            external_file_offsets,
            paths,
            parsed,
            source_annotations: HashMap::new(),
            core,
            colliding,
            aliases: HashMap::new(),
            declaration_libraries: FxHashMap::with_capacity_and_hasher(
                declaration_capacity,
                Default::default(),
            ),
            memos: LookupMemos::default(),
            external: base,
            packages: packages
                .iter()
                .map(|package| PackageIdentity {
                    name: package.name.clone(),
                    lib_root: canonical_or_lexical(&package.lib_root),
                })
                .collect(),
        };
        for (ordinal, library) in index.libraries.iter().enumerate() {
            if library.origin.is_none() {
                continue;
            }
            let library_id = LibraryId(ordinal);
            for (name, (kind, _)) in &library.declarations {
                if *kind != NameIdentity::Unknown
                    && let Some(identity) = index.declared(library_id, name)
                    && !matches!(identity, DeclarationIdentity::Library { .. })
                {
                    index.declaration_libraries.insert(identity, library_id);
                }
            }
            for name in &library.extensions {
                if let Some(identity) = index.declared(library_id, name)
                    && !matches!(identity, DeclarationIdentity::Library { .. })
                {
                    index.declaration_libraries.insert(identity, library_id);
                }
            }
        }
        for (file, (_, program, _)) in all.iter().enumerate() {
            let library = index.file_library[file];
            for declaration in program.declarations() {
                let TopLevelDecl::TypeAlias(alias) = declaration else {
                    continue;
                };
                // A name declared twice in one library has no unique alias.
                let unique = matches!(
                    index.get(library).declarations.get(alias.name().name()),
                    Some((NameIdentity::Type, _))
                );
                if unique && let Some(identity) = index.declared(library, alias.name().name()) {
                    index.aliases.insert(
                        identity,
                        TypeAlias {
                            file: FileId(file),
                            library,
                            node: index.node_ref(FileId(file), alias.syntax().id()),
                            templates: Default::default(),
                        },
                    );
                }
            }
        }
        // Resolve metadata only after aliases and declaration identities exist.
        let mut source_annotations = HashMap::new();
        for (file, (_, program, _)) in all.iter().enumerate() {
            let library = index.file_library[file];
            if matches!(
                index.libraries[library.0].origin,
                Some(ExternalOrigin::Sdk(_))
            ) {
                continue;
            }
            let model = super::SemanticModel::from_file(
                Some(FileId(file)),
                &index.parsed[file],
                &index,
                None,
            );
            collect_source_annotations(
                *program,
                library,
                FileId(file),
                &mut source_annotations,
                &index.parsed[file],
                model,
            );
        }
        index.source_annotations = source_annotations;
        index
    }

    /// The type alias (`typedef`) `identity` names, if it is one.
    pub(crate) fn type_alias(&self, identity: &DeclarationIdentity) -> Option<&TypeAlias> {
        self.aliases
            .get(identity)
            .or_else(|| self.external.as_ref()?.type_alias(identity))
    }

    /// The identity of a declaration, retaining its library when its name
    /// is not unique within the external package.
    fn declared(&self, library: LibraryId, name: &str) -> Option<DeclarationIdentity> {
        let name = self
            .get(library)
            .declarations
            .get_key_value(name)
            .map(|(name, _)| name)
            .or_else(|| self.get(library).extensions.get(name))?;
        match &self.get(library).origin {
            None => Some(DeclarationIdentity::Library {
                library,
                name: name.clone(),
            }),
            Some(origin) => {
                let copy = self.get(library).copy;
                let collides = |index: &Self| {
                    index
                        .colliding
                        .get(origin)
                        .and_then(|copies| copies.get(&copy))
                        .is_some_and(|names| names.contains(name))
                };
                if collides(self) || self.external.as_deref().is_some_and(collides) {
                    return Some(DeclarationIdentity::Library {
                        library,
                        name: name.clone(),
                    });
                }
                Some(match origin {
                    ExternalOrigin::Sdk(library) => DeclarationIdentity::Sdk {
                        library: library.clone(),
                        name: name.clone(),
                    },
                    ExternalOrigin::Package(package) => DeclarationIdentity::Package {
                        package: package.clone(),
                        name: name.clone(),
                        copy: self.get(library).copy,
                    },
                    ExternalOrigin::Project => DeclarationIdentity::Library {
                        library,
                        name: name.clone(),
                    },
                })
            }
        }
    }

    /// Whether a library-scoped declaration was loaded from user project code.
    /// SDK and package-wide identities do not carry project provenance.
    pub fn is_project(&self, identity: &DeclarationIdentity) -> bool {
        match identity {
            DeclarationIdentity::Library { library, .. } => {
                matches!(
                    self.get(*library).origin,
                    None | Some(ExternalOrigin::Project)
                )
            }
            DeclarationIdentity::Package { .. } | DeclarationIdentity::Sdk { .. } => false,
        }
    }

    /// Whether a declaration belongs to the named package, including packages
    /// whose sources are part of the analyzed project.
    pub fn is_package_declaration(
        &self,
        identity: &DeclarationIdentity,
        package: &str,
        name: &str,
    ) -> bool {
        if identity.is_package(package, name) {
            return true;
        }
        let DeclarationIdentity::Library {
            library,
            name: found_name,
        } = identity
        else {
            return false;
        };
        if found_name != name {
            return false;
        }
        if let Some(ExternalOrigin::Package(origin)) = &self.get(*library).origin {
            return origin.as_str() == package;
        }
        self.get(*library)
            .declarations
            .get(name)
            .and_then(|(_, file)| self.source_path(*file))
            .and_then(|path| {
                self.packages
                    .iter()
                    .filter(|package| path.starts_with(&package.lib_root))
                    .max_by_key(|package| package.lib_root.components().count())
            })
            .is_some_and(|origin| origin.name == package)
    }
    /// Whether real `dart:core` sources are indexed (so curated SDK fallback
    /// tables must not be consulted).
    pub fn has_sdk_sources(&self) -> bool {
        self.core.is_some()
    }

    /// The indexed file at `path`, normalizing its spelling.
    pub fn file(&self, path: &Path) -> Option<FileId> {
        self.file_canonical(path)
            .or_else(|| self.file_canonical(&canonical_or_lexical(path)))
    }

    fn file_canonical(&self, path: &Path) -> Option<FileId> {
        self.by_path
            .get(path)
            .copied()
            .or_else(|| self.external.as_ref()?.file_canonical(path))
    }

    /// The library of a loaded external instance, in loader order.
    pub(crate) fn external_library(&self, instance: usize) -> Option<LibraryId> {
        self.external_file(instance, 0)
            .map(|file| self.library(file))
    }

    /// A file within a loaded external instance, in that instance's file order.
    pub(crate) fn external_file(&self, instance: usize, ordinal: usize) -> Option<FileId> {
        if let Some(base) = &self.external {
            return base.external_file(instance, ordinal);
        }
        let start = *self.external_file_offsets.get(instance)?;
        let end = *self.external_file_offsets.get(instance + 1)?;
        (ordinal < end - start).then(|| FileId(start + ordinal))
    }

    pub fn library(&self, file: FileId) -> LibraryId {
        if file.0 & EXTERNAL_ID != 0
            && let Some(external) = &self.external
        {
            return external.library(file);
        }
        self.file_library[file.0 & !EXTERNAL_ID]
    }
    /// Possible deprecation metadata, including constructor annotations whose
    /// types may alias `dart:core`'s `Deprecated`, for a proven declaration.
    pub fn source_annotations(
        &self,
        identity: &DeclarationIdentity,
        selector: SourceSelector<'_>,
    ) -> Option<(&Path, &[NodeRef])> {
        if self.declaration_library(identity)?.0 & EXTERNAL_ID != 0
            && let Some(external) = &self.external
        {
            return external.source_annotations(identity, selector);
        }
        let entry = self.member_annotations(identity, selector.member)?;
        let (file, annotations) = match selector.parameter {
            Some(name) => &entry.parameters.get(name)?.1,
            None => entry.annotations.as_ref()?,
        };
        Some((self.source_path(*file)?, annotations))
    }

    fn member_annotations(
        &self,
        identity: &DeclarationIdentity,
        selector: MemberSelector<'_>,
    ) -> Option<&MemberAnnotations> {
        let library = self.declaration_library(identity)?;
        if library.0 & EXTERNAL_ID != 0
            && let Some(external) = &self.external
        {
            return external.member_annotations(identity, selector);
        }
        self.source_annotations
            .get(&LibraryId(library.0 & !EXTERNAL_ID))?
            .get(identity.name())?
            .get(&(selector.kind, selector.is_static))?
            .get(selector.name)
    }

    /// A static member with deprecation metadata, including parameter-only metadata.
    pub fn static_member_selector<'a>(
        &self,
        identity: &DeclarationIdentity,
        name: &'a str,
        write: bool,
    ) -> Option<MemberSelector<'a>> {
        let access = if write {
            MemberSelectorKind::Write
        } else {
            MemberSelectorKind::Read
        };
        [access, MemberSelectorKind::Field]
            .into_iter()
            .find_map(|kind| {
                let selector = MemberSelector::new(kind, true, name);
                self.member_annotations(identity, selector)
                    .map(|_| selector)
            })
    }

    /// A project library's metadata, addressed through its defining source.
    pub fn library_source_annotations(&self, path: &Path) -> Option<(&Path, &[NodeRef])> {
        let library = self.library(self.file(path)?);
        self.source_annotations(
            &DeclarationIdentity::Library {
                library,
                name: "".into(),
            },
            MemberSelector::DECLARATION.into(),
        )
    }

    /// The source name of a deprecated optional positional formal.
    pub fn deprecated_positional_parameter(
        &self,
        identity: &DeclarationIdentity,
        selector: MemberSelector<'_>,
        position: usize,
    ) -> Option<&str> {
        self.member_annotations(identity, selector)?
            .parameters
            .iter()
            .find_map(|(name, (ordinal, _))| (*ordinal == Some(position)).then_some(name.as_str()))
    }

    pub(crate) fn declaration_file(&self, library: LibraryId, name: &str) -> Option<FileId> {
        self.get(library)
            .declarations
            .get(name)
            .filter(|(identity, _)| *identity != NameIdentity::Unknown)
            .map(|(_, file)| *file)
    }

    /// A canonical external identity has one defining library even when several
    /// package contexts load different physical copies of its logical library.
    pub(crate) fn is_authoritative_declaration(&self, library: LibraryId, name: &str) -> bool {
        if self.get(library).origin.is_none() {
            return true;
        }
        self.declared(library, name)
            .is_some_and(|identity| self.declaration_library(&identity) == Some(library))
    }

    pub fn declaration_library(&self, identity: &DeclarationIdentity) -> Option<LibraryId> {
        match identity {
            DeclarationIdentity::Library { library, .. } => Some(*library),
            _ => self
                .declaration_libraries
                .get(identity)
                .copied()
                .or_else(|| self.external.as_ref()?.declaration_library(identity)),
        }
    }

    /// Whether a declaration's loaded library has no explicit library name.
    /// Missing source context is not evidence of an unnamed library.
    pub fn identity_is_in_unnamed_library(&self, identity: &DeclarationIdentity) -> Option<bool> {
        self.declaration_library(identity)
            .map(|library| !self.get(library).named)
    }

    pub(crate) fn source_path(&self, file: FileId) -> Option<&Path> {
        if file.0 & EXTERNAL_ID != 0
            && let Some(external) = &self.external
        {
            return external.source_path(file);
        }
        self.paths.get(file.0 & !EXTERNAL_ID)?.as_deref()
    }

    /// The `package:` URI of a project library's defining file, when it lies
    /// inside a known package's `lib` directory.
    pub fn library_uri(&self, library: LibraryId) -> Option<&str> {
        self.get(library).uri.as_deref()
    }

    fn get(&self, library: LibraryId) -> &Library {
        if library.0 & EXTERNAL_ID != 0
            && let Some(external) = &self.external
        {
            return external.get(library);
        }
        &self.libraries[library.0 & !EXTERNAL_ID]
    }

    pub(crate) fn has_configurable_directives(&self, library: LibraryId) -> bool {
        self.get(library).configurable
    }

    /// Conditional selection along direct imports and their re-export paths.
    pub(crate) fn configurable_constant_path(&self, source: LibraryId, target: LibraryId) -> bool {
        fn visit(
            index: &IdentityIndex,
            library: LibraryId,
            target: LibraryId,
            conditional: bool,
            visited: &mut FxHashSet<(LibraryId, bool)>,
        ) -> bool {
            let unit = index.get(library);
            let conditional = conditional || unit.configurable;
            if !visited.insert((library, conditional)) {
                return false;
            }
            if library == target {
                return conditional;
            }
            unit.exports.iter().any(|export| {
                matches!(export.target, ImportTarget::Library(next)
                    if visit(index, next, target, conditional, visited))
            })
        }
        if source == target {
            return self.has_configurable_directives(source);
        }
        let source = self.get(source);
        if source.configurable {
            return true;
        }
        let mut visited = FxHashSet::default();
        source
            .imports
            .iter()
            .map(|import| &import.target)
            .chain(source.exports.iter().map(|export| &export.target))
            .any(|edge| {
                matches!(edge, ImportTarget::Library(library)
                if visit(self, *library, target, false, &mut visited))
            })
    }

    fn complete(&self, library: LibraryId) -> Option<&Library> {
        Some(self.get(library)).filter(|library| !library.incomplete)
    }

    fn missing_part_type(&self, library: LibraryId, name: &str) -> Option<DeclarationIdentity> {
        let unit = self.get(library);
        (unit.missing_parts_only
            && self.implicit_core(unit, name, NameIdentity::Type).is_none()
            && unit
                .declarations
                .get(name)
                .is_some_and(|(identity, _)| *identity == NameIdentity::Type))
        .then(|| self.declared(library, name))
        .flatten()
    }

    /// Classify `name` as it is visible from `library`.
    pub fn resolve(&self, library: LibraryId, name: QualifiedName<'_>) -> NameIdentity {
        if name.prefix.is_none() && self.missing_part_type(library, name.name).is_some() {
            return NameIdentity::Type;
        }
        if self.complete(library).is_none() {
            return NameIdentity::Unknown;
        }
        self.memos
            .identities
            .get_or_compute(library, name.prefix, name.name, || match name.prefix {
                None => self.resolve_unprefixed(library, name.name),
                Some(prefix) => self.resolve_prefixed(library, prefix, name.name),
            })
    }

    /// Resolve the executable library scope, excluding documentation imports.
    /// Ambiguous declarations and prefixes remain distinct from absent names.
    pub fn library_name(&self, library: LibraryId, name: &str) -> DocName {
        let unit = self.get(library);
        if let Some(declared) = self.declared_doc_name(library, name) {
            return declared;
        }
        if unit
            .imports
            .iter()
            .any(|import| import.prefix.as_deref() == Some(name))
        {
            return DocName::Prefix;
        }
        let mut scope = self.imported_doc_name(unit, None, name);
        if scope == DocName::Absent && unit.implicit_core {
            scope = match self.core {
                Some(core) => {
                    let found = self.target_doc_name(
                        &ImportTarget::Library(core),
                        name,
                        &mut HashSet::new(),
                    );
                    let builtin = CORE_BUILTIN_TYPES.iter().find(|builtin| **builtin == name);
                    match (found, builtin) {
                        (DocName::Absent, Some(&builtin)) => DocName::Declaration {
                            identity: Some(DeclarationIdentity::sdk("dart:core", builtin)),
                            kind: NameIdentity::Type,
                        },
                        (found, _) => found,
                    }
                }
                None => {
                    curated_doc_name(sdk_type("dart:core", name), sdk_member("dart:core", name))
                        .unwrap_or(DocName::Absent)
                }
            };
        }
        scope
    }

    /// What the simple `name` denotes in the documentation-comment scope of
    /// `library`, as the analyzer's `DocumentationCommentScope` resolves it:
    /// the library scope (declarations, import prefixes, imports, implicit
    /// `dart:core`) first, then the export namespaces of the `@docImport`s,
    /// the first doc import supplying a name winning. Doc imports contribute
    /// no prefixes and their
    /// combinators are ignored, as by the analyzer (which also reports both as
    /// invalid). Without loaded SDK sources the implicit `dart:core` supplies
    /// only its curated members.
    pub fn doc_name(&self, library: LibraryId, name: &str) -> DocName {
        let scope = self.library_name(library, name);
        if !matches!(scope, DocName::Absent | DocName::Unknown) {
            return scope;
        }
        let unit = self.get(library);
        let mut uncertain = scope == DocName::Unknown;
        for target in &unit.doc_imports {
            match self.target_doc_name(target, name, &mut HashSet::new()) {
                DocName::Declaration { identity, kind } => {
                    return DocName::Declaration {
                        identity: identity.filter(|_| !uncertain),
                        kind: if uncertain {
                            NameIdentity::Unknown
                        } else {
                            kind
                        },
                    };
                }
                DocName::Extension if !uncertain => return DocName::Extension,
                DocName::Extension => {
                    return DocName::Declaration {
                        identity: None,
                        kind: NameIdentity::Unknown,
                    };
                }
                DocName::Unknown => uncertain = true,
                DocName::Prefix | DocName::Absent => {}
            }
        }
        if uncertain {
            DocName::Unknown
        } else {
            DocName::Absent
        }
    }

    /// What `prefix.name` denotes through the imports of `library` with that
    /// prefix.
    pub fn doc_prefixed_name(&self, library: LibraryId, prefix: &str, name: &str) -> DocName {
        self.imported_doc_name(self.get(library), Some(prefix), name)
    }

    /// `name` as the imports of `unit` with `prefix` supply it; an incomplete
    /// library may hide imports, so an absent name is unknown there.
    fn imported_doc_name(&self, unit: &Library, prefix: Option<&str>, name: &str) -> DocName {
        let found = unit
            .imports
            .iter()
            .filter(|import| {
                import.prefix.as_deref() == prefix && allows(&import.combinators, name)
            })
            .fold(DocName::Absent, |found, import| {
                join_doc_names(
                    found,
                    self.target_doc_name(&import.target, name, &mut HashSet::new()),
                )
            });
        if found == DocName::Absent && unit.incomplete {
            DocName::Unknown
        } else {
            found
        }
    }

    /// `name` in the export namespace of `target`.
    fn target_doc_name(
        &self,
        target: &ImportTarget,
        name: &str,
        visited: &mut HashSet<LibraryId>,
    ) -> DocName {
        match target {
            ImportTarget::Library(library) => {
                if name.starts_with('_') || !visited.insert(*library) {
                    return DocName::Absent;
                }
                if let Some(declared) = self.declared_doc_name(*library, name) {
                    return declared;
                }
                let unit = self.get(*library);
                let exported = unit
                    .exports
                    .iter()
                    .filter(|export| allows(&export.combinators, name))
                    .fold(DocName::Absent, |found, export| {
                        join_doc_names(found, self.target_doc_name(&export.target, name, visited))
                    });
                if exported == DocName::Absent && unit.incomplete {
                    DocName::Unknown
                } else {
                    exported
                }
            }
            ImportTarget::Sdk(uri) => curated_doc_name(sdk_type(uri, name), sdk_member(uri, name))
                .unwrap_or(DocName::Unknown),
            ImportTarget::Package(package) => {
                curated_doc_name(package_type(package, name), package_member(package, name))
                    .unwrap_or(DocName::Unknown)
            }
            ImportTarget::Unknown => DocName::Unknown,
        }
    }

    /// `name` as `library` itself declares it.
    fn declared_doc_name(&self, library: LibraryId, name: &str) -> Option<DocName> {
        let unit = self.get(library);
        if let Some((kind, _)) = unit.declarations.get(name) {
            let identity = (*kind != NameIdentity::Unknown)
                .then(|| self.declared(library, name))
                .flatten();
            return Some(DocName::Declaration {
                identity,
                kind: *kind,
            });
        }
        unit.extensions.contains(name).then_some(DocName::Extension)
    }

    pub fn extension_visible(
        &self,
        from: LibraryId,
        declaring: LibraryId,
        extension_name: Option<&str>,
    ) -> bool {
        // Anonymous extensions are lexical to their library, including parts.
        if from == declaring || extension_name.is_none() {
            return from == declaring;
        }
        self.memos
            .extensions
            .get_or_compute((from, declaring), extension_name, "", || {
                let unit = self.get(from);
                unit.imports.iter().any(|import| {
                    extension_name.is_none_or(|name| allows(&import.combinators, name))
                        && matches!(import.target, ImportTarget::Library(library) if self.library_exports(library, declaring, extension_name, &mut HashSet::new()))
                }) || self.core.is_some_and(|core| {
                    unit.implicit_core
                        && self.library_exports(core, declaring, extension_name, &mut HashSet::new())
                })
            })
    }

    fn library_exports(
        &self,
        library: LibraryId,
        target: LibraryId,
        extension_name: Option<&str>,
        visited: &mut HashSet<LibraryId>,
    ) -> bool {
        if library == target {
            return true;
        }
        if !visited.insert(library) {
            return false;
        }
        self.get(library).exports.iter().any(|export| {
            extension_name.is_none_or(|name| allows(&export.combinators, name))
                && matches!(export.target, ImportTarget::Library(next) if self.library_exports(next, target, extension_name, visited))
        })
    }

    /// Resolve a named extension in the ordinary import/export namespace.
    ///
    /// Answers are memoized by library, prefix and name. Incomplete external
    /// branches invalidate their matching bindings, not unrelated names;
    /// ambiguous exports and potentially hidden project bindings stay unresolved.
    pub fn resolve_extension_declaration(
        &self,
        library: LibraryId,
        QualifiedName { prefix, name }: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.complete(library)?;
        self.memos
            .named_extensions
            .get_or_compute(library, prefix, name, || {
                self.resolve_extension_declaration_uncached(library, unit, prefix, name)
            })
    }

    fn resolve_extension_declaration_uncached(
        &self,
        library: LibraryId,
        unit: &Library,
        prefix: Option<&str>,
        name: &str,
    ) -> Option<DeclarationIdentity> {
        if let Some(prefix) = prefix {
            if unit.declarations.contains_key(prefix) {
                return None;
            }
        } else {
            if unit.declarations.contains_key(name) {
                return None;
            }
            if unit.extensions.contains(name) {
                return self.declared(library, name);
            }
        }
        let mut candidate = None;
        for import in self.visible_imports(unit, prefix, name) {
            if self.target_may_hide_declaration(&import.target, name) {
                return None;
            }
            if let ImportTarget::Library(target) = import.target
                && let Some(found) = self
                    .exported_extension(target, name, &mut HashSet::new())
                    .ok()?
            {
                if candidate
                    .as_ref()
                    .is_some_and(|previous| previous != &found)
                {
                    return None;
                }
                candidate = Some(found);
            }
        }
        if prefix.is_none() && candidate.is_none() && unit.implicit_core {
            candidate = self
                .core
                .and_then(|core| {
                    self.exported_extension(core, name, &mut HashSet::new())
                        .ok()
                })
                .flatten();
        }
        candidate
    }

    fn exported_extension(
        &self,
        library: LibraryId,
        name: &str,
        visited: &mut HashSet<LibraryId>,
    ) -> Result<Option<DeclarationIdentity>, ()> {
        if !visited.is_empty() {
            return self.walk_exported_extension(library, name, visited);
        }
        self.memos
            .extension_exports
            .get_or_compute(library, None, name, || {
                self.walk_exported_extension(library, name, visited)
            })
    }

    fn walk_exported_extension(
        &self,
        library: LibraryId,
        name: &str,
        visited: &mut HashSet<LibraryId>,
    ) -> Result<Option<DeclarationIdentity>, ()> {
        if name.starts_with('_') || !visited.insert(library) {
            return Ok(None);
        }
        let unit = self.get(library);
        let declares_name = unit.declarations.contains_key(name) || unit.extensions.contains(name);
        // Incomplete external libraries invalidate matching bindings, but their
        // unrelated re-exports still participate in this namespace.
        if unit.incomplete && (unit.origin.is_none() || declares_name) {
            return Ok(None);
        }
        if unit.extensions.contains(name) {
            return Ok(self.declared(library, name));
        }
        if unit.declarations.contains_key(name) {
            return Ok(None);
        }
        let mut candidate = None;
        for export in &unit.exports {
            if allows(&export.combinators, name)
                && let ImportTarget::Library(target) = export.target
                && let Some(found) = self.exported_extension(target, name, visited)?
            {
                if candidate
                    .as_ref()
                    .is_some_and(|previous| previous != &found)
                {
                    return Err(());
                }
                candidate = Some(found);
            }
        }
        Ok(candidate)
    }

    /// Resolve a written type name to its canonical declaring library identity.
    /// Ambiguous imports, values shadowing types, and potentially hidden bindings
    /// return `None` rather than guessing from the final name segment.
    pub fn resolve_declaration(
        &self,
        library: LibraryId,
        name: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        if name.prefix.is_none()
            && let Some(identity) = self.missing_part_type(library, name.name)
        {
            return Some(identity);
        }
        self.complete(library)?;
        self.memos
            .declarations
            .get_or_compute(library, name.prefix, name.name, || match name.prefix {
                None => self.resolve_unprefixed_declaration(library, name.name),
                Some(prefix) => self.resolve_prefixed_declaration(library, prefix, name.name),
            })
    }

    /// Resolve a visible top-level value to its canonical declaring library.
    pub fn resolve_value_declaration(
        &self,
        library: LibraryId,
        QualifiedName { prefix, name }: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.complete(library)?;
        self.memos.values.get_or_compute(library, prefix, name, || {
            self.resolve_value_declaration_uncached(library, unit, prefix, name)
        })
    }

    fn resolve_value_declaration_uncached(
        &self,
        library: LibraryId,
        unit: &Library,
        prefix: Option<&str>,
        name: &str,
    ) -> Option<DeclarationIdentity> {
        if prefix.is_none() {
            match unit.declarations.get(name) {
                Some((NameIdentity::Value, _)) => return self.declared(library, name),
                Some(_) => return None,
                None => {}
            }
        }
        if self
            .visible_imports(unit, prefix, name)
            .any(|import| self.target_may_hide_declaration(&import.target, name))
        {
            return None;
        }
        let mut candidates = self
            .visible_imports(unit, prefix, name)
            .filter_map(|import| match import.target {
                ImportTarget::Library(target) => self.exported_declaration(
                    target,
                    name,
                    NameIdentity::Value,
                    &mut HashSet::new(),
                ),
                _ => None,
            })
            .peekable();
        if prefix.is_none() && candidates.peek().is_none() {
            return self.implicit_core(unit, name, NameIdentity::Value);
        }
        unique_declaration(candidates)
    }

    /// Resolve an imported SDK or package member without guessing from its name.
    ///
    /// Unlike [`Self::resolve_declaration`], this also supports annotations and
    /// other top-level values from external libraries, including ones a
    /// project library re-exports (`export 'package:meta/meta.dart'`). Indexed
    /// package sources retain their project identity elsewhere, but this lookup
    /// exposes their package provenance after resolving a unique declaration.
    pub fn resolve_imported_member(
        &self,
        library: LibraryId,
        name: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        self.imported_member(self.complete(library)?, name)
    }

    /// Resolve imported annotations before generation when only generated parts are missing.
    pub fn resolve_annotation_before_generation(
        &self,
        library: LibraryId,
        name: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.get(library);
        if unit.incomplete && !unit.missing_generated_parts_only {
            return None;
        }
        self.imported_member(unit, name)
    }

    fn imported_member(
        &self,
        unit: &Library,
        QualifiedName { prefix, name }: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        if prefix.is_none() && unit.declarations.contains_key(name) {
            return None;
        }
        if self
            .visible_imports(unit, prefix, name)
            .any(|import| self.target_may_hide_declaration(&import.target, name))
        {
            return None;
        }
        let candidates =
            self.visible_imports(unit, prefix, name)
                .filter_map(|import| match &import.target {
                    ImportTarget::Sdk(uri) => sdk_type(uri, name).or_else(|| sdk_member(uri, name)),
                    ImportTarget::Package(package) => package_member(package, name),
                    ImportTarget::Library(target) => self.exported_any(*target, name),
                    ImportTarget::Unknown => None,
                });
        match unique_declaration(candidates)? {
            DeclarationIdentity::Library { library, name } => {
                let unit = self.get(library);
                if unit.origin.is_some() {
                    return Some(DeclarationIdentity::Library { library, name });
                }
                let (package, _) = unit
                    .uri
                    .as_ref()?
                    .strip_prefix("package:")?
                    .split_once('/')?;
                let (name, _) = unit.declarations.get_key_value(name.as_str())?;
                Some(DeclarationIdentity::Package {
                    package: InternedString::Shared(package.into()),
                    name: name.clone(),
                    copy: None,
                })
            }
            identity => Some(identity),
        }
    }

    /// Resolve an imported SDK top-level member without guessing from its name.
    pub fn resolve_sdk_member(
        &self,
        library: LibraryId,
        QualifiedName { prefix, name }: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.complete(library)?;
        if prefix.is_none() && unit.declarations.contains_key(name) {
            return None;
        }
        if self
            .visible_imports(unit, prefix, name)
            .any(|import| self.target_may_hide_declaration(&import.target, name))
        {
            return None;
        }
        let candidates =
            self.visible_imports(unit, prefix, name)
                .filter_map(|import| match &import.target {
                    ImportTarget::Sdk(uri) => sdk_member(uri, name),
                    ImportTarget::Library(target) => self
                        .exported_any(*target, name)
                        .filter(|identity| matches!(identity, DeclarationIdentity::Sdk { .. })),
                    _ => None,
                });
        let core = (prefix.is_none() && unit.implicit_core)
            .then(|| match self.core {
                Some(core) => self.exported_any(core, name),
                None => sdk_member("dart:core", name),
            })
            .flatten();
        unique_declaration(candidates.chain(core))
    }

    /// `name` as exported by `library` in either namespace.
    pub fn exported_any(&self, library: LibraryId, name: &str) -> Option<DeclarationIdentity> {
        [NameIdentity::Type, NameIdentity::Value]
            .into_iter()
            .find_map(|namespace| {
                self.exported_declaration(library, name, namespace, &mut HashSet::new())
            })
    }

    /// `name` from the implicitly imported `dart:core` of `unit`: the loaded
    /// library when SDK sources are indexed, else the curated type allowlist.
    fn implicit_core(
        &self,
        unit: &Library,
        name: &str,
        namespace: NameIdentity,
    ) -> Option<DeclarationIdentity> {
        if !unit.implicit_core {
            return None;
        }
        match self.core {
            Some(core) => self
                .exported_declaration(core, name, namespace, &mut HashSet::new())
                .or_else(|| {
                    let builtin = CORE_BUILTIN_TYPES.iter().find(|builtin| **builtin == name);
                    builtin
                        .filter(|_| namespace == NameIdentity::Type)
                        .map(|&name| DeclarationIdentity::sdk("dart:core", name))
                }),
            None if namespace == NameIdentity::Type => sdk_type("dart:core", name),
            None => None,
        }
    }

    fn visible_imports<'s>(
        &self,
        unit: &'s Library,
        prefix: Option<&'s str>,
        name: &'s str,
    ) -> impl Iterator<Item = &'s Import> {
        unit.imports.iter().filter(move |import| {
            import.prefix.as_deref() == prefix && allows(&import.combinators, name)
        })
    }

    /// Missing project input can hide any name. Incomplete external input
    /// invalidates its indexed bindings, not unrelated imports or re-exports.
    fn target_may_hide_declaration(&self, target: &ImportTarget, name: &str) -> bool {
        match target {
            ImportTarget::Unknown => true,
            ImportTarget::Library(library) => {
                self.memos
                    .uncertainty
                    .get_or_compute(*library, None, name, || {
                        self.walk_lookup_uncertainty(*library, name, &mut HashSet::new())
                    })
            }
            ImportTarget::Sdk(_) | ImportTarget::Package(_) => false,
        }
    }

    fn walk_lookup_uncertainty(
        &self,
        library: LibraryId,
        name: &str,
        visited: &mut HashSet<LibraryId>,
    ) -> bool {
        let unit = self.get(library);
        if name.starts_with('_') {
            return false;
        }
        if self.missing_part_type(library, name).is_some() {
            return false;
        }
        let declares_name = unit.declarations.contains_key(name) || unit.extensions.contains(name);
        if unit.incomplete && (unit.origin.is_none() || unit.missing_parts_only || declares_name) {
            return true;
        }
        if declares_name || !visited.insert(library) {
            return false;
        }
        unit.exports.iter().any(|export| {
            allows(&export.combinators, name)
                && match &export.target {
                    ImportTarget::Library(target) => {
                        self.walk_lookup_uncertainty(*target, name, visited)
                    }
                    ImportTarget::Unknown => true,
                    ImportTarget::Sdk(_) | ImportTarget::Package(_) => false,
                }
        })
    }

    fn resolve_unprefixed_declaration(
        &self,
        library: LibraryId,
        name: &str,
    ) -> Option<DeclarationIdentity> {
        let unit = self.get(library);
        if let Some((identity, _)) = unit.declarations.get(name) {
            return (*identity == NameIdentity::Type)
                .then(|| self.declared(library, name))
                .flatten();
        }
        if self
            .visible_imports(unit, None, name)
            .any(|import| self.target_may_hide_declaration(&import.target, name))
        {
            return None;
        }
        let mut candidates = self
            .visible_imports(unit, None, name)
            .filter_map(|import| self.target_declaration(&import.target, name))
            .peekable();
        if candidates.peek().is_none() {
            return self.implicit_core(unit, name, NameIdentity::Type);
        }
        unique_declaration(candidates)
    }

    fn resolve_prefixed_declaration(
        &self,
        library: LibraryId,
        prefix: &str,
        name: &str,
    ) -> Option<DeclarationIdentity> {
        let unit = self.get(library);
        if unit.declarations.contains_key(prefix) {
            return None;
        }
        if self
            .visible_imports(unit, Some(prefix), name)
            .any(|import| self.target_may_hide_declaration(&import.target, name))
        {
            return None;
        }
        unique_declaration(
            self.visible_imports(unit, Some(prefix), name)
                .filter_map(|import| self.target_declaration(&import.target, name)),
        )
    }

    fn target_declaration(&self, target: &ImportTarget, name: &str) -> Option<DeclarationIdentity> {
        match target {
            ImportTarget::Library(library) => {
                self.exported_declaration(*library, name, NameIdentity::Type, &mut HashSet::new())
            }
            ImportTarget::Sdk(uri) => sdk_type(uri, name),
            ImportTarget::Package(package) => package_type(package, name),
            ImportTarget::Unknown => None,
        }
    }

    fn resolve_unprefixed(&self, library: LibraryId, name: &str) -> NameIdentity {
        let unit = self.get(library);
        if let Some((identity, _)) = unit.declarations.get(name) {
            return *identity;
        }
        if self.resolve_unprefixed_declaration(library, name).is_some() {
            return NameIdentity::Type;
        }
        if self
            .resolve_value_declaration(library, name.into())
            .is_some()
        {
            return NameIdentity::Value;
        }
        let mut result = None;
        for import in self.visible_imports(unit, None, name) {
            combine(&mut result, self.target_identity(&import.target, name));
        }
        result.unwrap_or(NameIdentity::Unknown)
    }

    fn resolve_prefixed(&self, library: LibraryId, prefix: &str, name: &str) -> NameIdentity {
        let unit = self.get(library);
        if unit.declarations.contains_key(prefix) {
            return NameIdentity::Unknown;
        }
        if self
            .resolve_prefixed_declaration(library, prefix, name)
            .is_some()
        {
            return NameIdentity::Type;
        }
        if self
            .resolve_value_declaration(library, QualifiedName::prefixed(prefix, name))
            .is_some()
        {
            return NameIdentity::Value;
        }
        let mut result = None;
        for import in self.visible_imports(unit, Some(prefix), name) {
            combine(&mut result, self.target_identity(&import.target, name));
        }
        result.unwrap_or(NameIdentity::Unknown)
    }

    /// `name` in `namespace` as exported by `library`. Top-level lookups
    /// (`visiting` empty) are memoized: re-export chains through large
    /// external libraries are otherwise walked for every reference.
    fn exported_declaration(
        &self,
        library: LibraryId,
        name: &str,
        namespace: NameIdentity,
        visiting: &mut HashSet<LibraryId>,
    ) -> Option<DeclarationIdentity> {
        if !visiting.is_empty() {
            return self.walk_exported_declaration(library, name, namespace, visiting);
        }
        self.memos
            .exports
            .get_or_compute((library, namespace), None, name, || {
                self.walk_exported_declaration(library, name, namespace, visiting)
            })
    }

    fn walk_exported_declaration(
        &self,
        library: LibraryId,
        name: &str,
        namespace: NameIdentity,
        visiting: &mut HashSet<LibraryId>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.get(library);
        let declares_name = unit.declarations.contains_key(name) || unit.extensions.contains(name);
        if !name.starts_with('_')
            && namespace == NameIdentity::Type
            && let Some(identity) = self.missing_part_type(library, name)
        {
            return Some(identity);
        }
        if name.starts_with('_')
            || (unit.incomplete
                && (unit.origin.is_none() || unit.missing_parts_only || declares_name))
            || !visiting.insert(library)
        {
            return None;
        }
        if unit
            .declarations
            .get(name)
            .is_some_and(|(identity, _)| *identity == namespace)
        {
            visiting.remove(&library);
            return self.declared(library, name);
        }
        if unit.exports.iter().any(|export| {
            allows(&export.combinators, name)
                && self.target_may_hide_declaration(&export.target, name)
        }) {
            visiting.remove(&library);
            return None;
        }
        let candidate = unique_declaration(
            unit.exports
                .iter()
                .filter(|export| allows(&export.combinators, name))
                .filter_map(|export| match &export.target {
                    ImportTarget::Library(target) => {
                        self.exported_declaration(*target, name, namespace, visiting)
                    }
                    ImportTarget::Sdk(uri) if namespace == NameIdentity::Type => {
                        sdk_type(uri, name)
                    }
                    ImportTarget::Package(package) if namespace == NameIdentity::Type => {
                        package_type(package, name)
                    }
                    ImportTarget::Sdk(uri) if namespace == NameIdentity::Value => {
                        sdk_member(uri, name)
                    }
                    ImportTarget::Package(package) if namespace == NameIdentity::Value => {
                        package_member(package, name)
                            .filter(|_| package_type(package, name).is_none())
                    }
                    _ => None,
                }),
        );
        visiting.remove(&library);
        candidate
    }

    fn exported(
        &self,
        library: LibraryId,
        name: &str,
        visiting: &mut HashSet<LibraryId>,
    ) -> NameIdentity {
        let unit = self.get(library);
        if name.starts_with('_') || unit.incomplete || !visiting.insert(library) {
            return NameIdentity::Unknown;
        }
        if let Some((identity, _)) = unit.declarations.get(name) {
            visiting.remove(&library);
            return *identity;
        }
        let mut result = None;
        for export in &unit.exports {
            if !allows(&export.combinators, name) {
                continue;
            }
            let identity = match &export.target {
                ImportTarget::Library(target) => self.exported(*target, name, visiting),
                target => self.target_identity(target, name),
            };
            combine(&mut result, identity);
        }
        visiting.remove(&library);
        result.unwrap_or(NameIdentity::Unknown)
    }

    fn target_identity(&self, target: &ImportTarget, name: &str) -> NameIdentity {
        let known = match target {
            ImportTarget::Library(library) => {
                return self.exported(*library, name, &mut HashSet::new());
            }
            ImportTarget::Sdk(uri) => sdk_type(uri, name).is_some(),
            ImportTarget::Package(package) => package_type(package, name).is_some(),
            ImportTarget::Unknown => false,
        };
        if known {
            NameIdentity::Type
        } else {
            NameIdentity::Unknown
        }
    }
}

fn collect_source_annotations(
    program: Program<'_>,
    library: LibraryId,
    file: FileId,
    index: &mut SourceAnnotations,
    parsed: &Arc<ParsedFile>,
    model: super::SemanticModel<'_>,
) {
    use MemberSelectorKind as Kind;
    let mut add = |owner: &str,
                   selector: MemberSelector<'_>,
                   annotations: AstChildren<'_, Annotation<'_>>,
                   params: Option<FormalParamList<'_>>| {
        let annotations = deprecation_annotations(annotations, parsed, model);
        let mut parameters = HashMap::new();
        if let Some(params) = params {
            let mut positional = 0;
            for param in params.params() {
                let position = if matches!(param.kind(), ParamKind::Named { .. }) {
                    None
                } else {
                    let position = positional;
                    positional += 1;
                    Some(position)
                };
                if let Some(annotations) = parameter_deprecation(param, parsed, model) {
                    parameters.insert(
                        param.name().name().to_owned(),
                        (position, (file, annotations)),
                    );
                }
            }
        }
        if annotations.is_none() && parameters.is_empty() {
            return;
        }
        let owners = index.entry(library).or_default();
        if !owners.contains_key(owner) {
            owners.insert(owner.to_owned(), DeclarationAnnotations::default());
        }
        let names = owners
            .get_mut(owner)
            .unwrap()
            .entry((selector.kind, selector.is_static))
            .or_default();
        if !names.contains_key(selector.name) {
            names.insert(selector.name.to_owned(), MemberAnnotations::default());
        }
        let entry = names.get_mut(selector.name).unwrap();
        if let Some(annotations) = annotations {
            entry.annotations = Some((file, annotations));
        }
        entry.parameters.extend(parameters);
    };
    if let Some(directive) = program.library_directive() {
        add(
            "",
            MemberSelector::DECLARATION,
            directive.annotations(),
            None,
        );
    }
    for declaration in program.declarations() {
        let (names, annotations, members, params) = match declaration {
            TopLevelDecl::Class(n) => (
                vec![n.name().name()],
                n.annotations(),
                Some(n.element_members()),
                None,
            ),
            TopLevelDecl::Mixin(n) => (
                vec![n.name().name()],
                n.annotations(),
                Some(EffectiveMembers::declared(n.members())),
                None,
            ),
            TopLevelDecl::MixinClass(n) => (
                vec![n.name().name()],
                n.annotations(),
                Some(n.element_members()),
                None,
            ),
            TopLevelDecl::Enum(n) => (
                vec![n.name().name()],
                n.annotations(),
                Some(n.element_members()),
                None,
            ),
            TopLevelDecl::ExtensionType(n) => (
                vec![n.name().name()],
                n.annotations(),
                Some(n.element_members()),
                None,
            ),
            TopLevelDecl::ClassTypeAlias(n) => (vec![n.name().name()], n.annotations(), None, None),
            TopLevelDecl::TypeAlias(n) => (vec![n.name().name()], n.annotations(), None, None),
            TopLevelDecl::Function(n) => (vec![n.name().name()], n.annotations(), None, n.params()),
            TopLevelDecl::Variable(n) => (
                n.declarators().map(|d| d.name().name()).collect(),
                n.annotations(),
                None,
                None,
            ),
            TopLevelDecl::Extension(n) => (
                n.name().into_iter().map(|name| name.name()).collect(),
                n.annotations(),
                Some(EffectiveMembers::declared(n.members())),
                None,
            ),
            TopLevelDecl::Error(_) => continue,
        };
        for name in names {
            let selector = match declaration {
                TopLevelDecl::Variable(_) => MemberSelector::new(Kind::Variable, false, ""),
                TopLevelDecl::Function(n) if matches!(n.kind(), FunctionKind::Setter(_)) => {
                    MemberSelector::new(Kind::Write, false, "")
                }
                _ => MemberSelector::DECLARATION,
            };
            add(name, selector, annotations.clone(), params);
            let Some(members) = &members else {
                continue;
            };
            if let TopLevelDecl::Enum(enumeration) = declaration {
                for variant in enumeration.variants() {
                    add(
                        name,
                        MemberSelector::new(Kind::Field, true, variant.name().name()),
                        variant.annotations(),
                        None,
                    );
                }
            }
            for member in members.declared.clone() {
                let (selector, annotations, params) = match member {
                    ClassMember::Field(field) => {
                        for variable in field.declarators() {
                            add(
                                name,
                                MemberSelector::new(
                                    Kind::Field,
                                    field.is_static(),
                                    variable.name().name(),
                                ),
                                field.annotations(),
                                None,
                            );
                        }
                        continue;
                    }
                    ClassMember::Method(n) => (
                        MemberSelector::new(Kind::Read, n.is_static(), n.name().name()),
                        n.annotations(),
                        Some(n.params()),
                    ),
                    ClassMember::Getter(n) => (
                        MemberSelector::new(Kind::Read, n.is_static(), n.name().name()),
                        n.annotations(),
                        None,
                    ),
                    ClassMember::Setter(n) => (
                        MemberSelector::new(Kind::Write, n.is_static(), n.name().name()),
                        n.annotations(),
                        None,
                    ),
                    ClassMember::Operator(n) => (
                        MemberSelector::new(
                            Kind::Operator(n.params().params().len()),
                            false,
                            n.op(),
                        ),
                        n.annotations(),
                        None,
                    ),
                    ClassMember::Constructor(_)
                    | ClassMember::PrimaryConstructorBody(_)
                    | ClassMember::Error(_) => continue,
                };
                add(name, selector, annotations, params);
            }
            for param in members.declaring_params() {
                add(
                    name,
                    MemberSelector::new(Kind::Field, false, param.name().name()),
                    param.annotations(),
                    None,
                );
            }
            for constructor in members.constructors() {
                add(
                    name,
                    MemberSelector::new(
                        Kind::Constructor,
                        false,
                        constructor
                            .constructor_name
                            .map_or("new", |name| name.name()),
                    ),
                    constructor.annotations,
                    Some(constructor.params),
                );
            }
        }
    }
}

/// Deprecation does not apply to required parameters (the official
/// `deprecated_member_use` verifier skips them), so they are never indexed.
fn parameter_deprecation(
    param: FormalParam<'_>,
    parsed: &Arc<ParsedFile>,
    model: super::SemanticModel<'_>,
) -> Option<Vec<NodeRef>> {
    if is_required_parameter(param) {
        return None;
    }
    deprecation_annotations(param.annotations(), parsed, model)
}

fn is_required_parameter(param: FormalParam<'_>) -> bool {
    matches!(
        param.kind(),
        ParamKind::Positional | ParamKind::Named { required: true }
    )
}

/// SDK-spelled candidates (even without SDK sources) and resolved SDK aliases.
/// Consumers must resolve identity before treating retained metadata as deprecated.
fn deprecation_annotations(
    annotations: AstChildren<'_, Annotation<'_>>,
    parsed: &Arc<ParsedFile>,
    model: super::SemanticModel<'_>,
) -> Option<Vec<NodeRef>> {
    let annotations: Vec<_> = annotations
        .filter(|annotation| {
            let sdk_spelling = annotation.constructor_name().is_none()
                && match annotation.name().last().map(|name| name.name()) {
                    Some("deprecated") => annotation.args().is_none(),
                    Some("Deprecated") => annotation.args().is_some(),
                    _ => false,
                };
            sdk_spelling || model.is_deprecation(*annotation)
        })
        .map(|annotation| NodeRef::new(Arc::clone(parsed), annotation.syntax().id()))
        .collect();
    (!annotations.is_empty()).then_some(annotations)
}

fn declaration_identities(
    declaration: TopLevelDecl<'_>,
    mut visit: impl FnMut(&str, NameIdentity),
) {
    match declaration {
        TopLevelDecl::Class(x) => visit(x.name().name(), NameIdentity::Type),
        TopLevelDecl::ClassTypeAlias(x) => visit(x.name().name(), NameIdentity::Type),
        TopLevelDecl::Mixin(x) => visit(x.name().name(), NameIdentity::Type),
        TopLevelDecl::MixinClass(x) => visit(x.name().name(), NameIdentity::Type),
        TopLevelDecl::Enum(x) => visit(x.name().name(), NameIdentity::Type),
        TopLevelDecl::ExtensionType(x) => visit(x.name().name(), NameIdentity::Type),
        TopLevelDecl::TypeAlias(x) => visit(x.name().name(), NameIdentity::Type),
        TopLevelDecl::Function(x) => visit(x.name().name(), NameIdentity::Value),
        TopLevelDecl::Variable(x) => {
            for declarator in x.declarators() {
                visit(declarator.name().name(), NameIdentity::Value);
            }
        }
        TopLevelDecl::Extension(_) | TopLevelDecl::Error(_) => {}
    }
}

fn merge(
    map: &mut FxHashMap<InternedString, (NameIdentity, FileId)>,
    name: InternedString,
    identity: NameIdentity,
    file: FileId,
) {
    map.entry(name)
        .and_modify(|(existing, _)| *existing = NameIdentity::Unknown)
        .or_insert((identity, file));
}

/// The unique candidate; when non-SDK candidates are present, `dart:`
/// declarations are dropped first (a name imported from both a `dart:`
/// library and another library denotes the other library's declaration).
fn unique_declaration(
    candidates: impl IntoIterator<Item = DeclarationIdentity>,
) -> Option<DeclarationIdentity> {
    let mut sdk = None;
    let mut other = None;
    for candidate in candidates {
        let slot = if matches!(candidate, DeclarationIdentity::Sdk { .. }) {
            &mut sdk
        } else {
            &mut other
        };
        match slot {
            None => *slot = Some(Some(candidate)),
            Some(Some(first)) if *first != candidate => *slot = Some(None),
            _ => {}
        }
    }
    other.or(sdk).flatten()
}

/// Types the `dart:core` namespace provides without a source declaration.
const CORE_BUILTIN_TYPES: &[&str] = &["dynamic", "Never"];

fn combine(result: &mut Option<NameIdentity>, identity: NameIdentity) {
    match result {
        None => *result = Some(identity),
        // A name two imports can both supply is ambiguous, even when both
        // supply the same kind of thing.
        Some(existing) => *existing = NameIdentity::Unknown,
    }
}

/// The borrowed URI bodies of simple `/// @docImport` header lines. This
/// resolver contract is intentionally separate from the parsed ordering view.
/// Prefixes and combinators do not affect documentation lookups.
pub fn doc_imports<'s>(program: Program<'_>, source: &'s str) -> impl Iterator<Item = &'s str> {
    let header = program
        .library_directive()
        .and_then(|directive| source.get(..directive.span().start))
        .unwrap_or_default();
    header.lines().filter_map(|line| {
        let rest = line.trim_start().strip_prefix("///")?.trim_start();
        let rest = rest.strip_prefix("@docImport")?.trim_start();
        let quote = rest.chars().next().filter(|c| matches!(c, '\'' | '"'))?;
        rest[1..].split_once(quote).map(|(uri, _)| uri)
    })
}

/// A documentation import and its source location.
#[derive(Debug, Clone)]
pub struct DocImport<'s> {
    pub uri: &'s str,
    pub literal: NodeRef,
    pub span: Span,
}

/// Documentation imports attached to the library directive, in source order.
pub fn library_doc_imports<'s>(
    program: Program<'_>,
    source: &'s str,
    syntax: &'s SyntaxTree,
) -> impl Iterator<Item = DocImport<'s>> {
    let end = program
        .library_directive()
        .map_or(0, |directive| directive.span().start);
    let start = syntax
        .documentation_start(source, Span::new(0, end))
        .unwrap_or(end);
    let mut import_source = String::new();
    syntax
        .tokens_in(source, Span::new(start, end))
        .filter(|token| token.kind() == falcon_syntax::TokenKind::DocComment)
        .flat_map(move |token| {
            token
                .text()
                .split_inclusive('\n')
                .scan(token.offset(), move |offset, line| {
                    let start = *offset;
                    *offset += line.len();
                    Some((start, line))
                })
        })
        .filter_map(move |(start, line)| {
            let text = line.trim_start();
            let text = text
                .strip_prefix("///")
                .or_else(|| text.strip_prefix("/**"))
                .or_else(|| text.strip_prefix('*'))
                .unwrap_or(text)
                .trim_start();
            let rest = text.strip_prefix("@docImport")?;
            if !rest.starts_with(' ') {
                return None;
            }
            let rest = rest.trim_start();
            let directive_start = start + line.len() - rest.len() - "import ".len();
            // Let the Dart lexer/parser handle quoting, escaping and adjacent
            // literals rather than interpreting comment text as a string grammar.
            import_source.clear();
            import_source.push_str("import ");
            import_source.push_str(rest);
            let parsed = falcon_dart_parser::parse(&import_source);
            let parsed = Arc::new(ParsedFile {
                source: import_source.clone().into_boxed_str(),
                tree: parsed.tree,
            });
            let import = parsed.program().imports().next()?;
            let (first, first_span) = import.uri().segments().next()?;
            let content_start =
                first_span.start.checked_sub("import ".len())? + first.content_offset;
            let uri = rest.get(content_start..content_start + first.content.len())?;
            Some(DocImport {
                uri,
                span: Span::new(directive_start, directive_start + import.span().end),
                literal: NodeRef::new(Arc::clone(&parsed), import.uri().syntax().id()),
            })
        })
}

/// Two namespaces' answers for one name: a name both supply is still found,
/// but without a unique identity unless both agree on it.
fn join_doc_names(left: DocName, right: DocName) -> DocName {
    match (left, right) {
        (DocName::Absent, other) | (other, DocName::Absent) => other,
        (left, right) if left == right => left,
        // Found in one namespace and possibly or differently in another.
        _ => DocName::Declaration {
            identity: None,
            kind: NameIdentity::Unknown,
        },
    }
}

/// A curated SDK or package type or member (types first).
fn curated_doc_name(
    ty: Option<DeclarationIdentity>,
    member: Option<DeclarationIdentity>,
) -> Option<DocName> {
    ty.map(|identity| (identity, NameIdentity::Type))
        .or_else(|| member.map(|identity| (identity, NameIdentity::Value)))
        .map(|(identity, kind)| DocName::Declaration {
            identity: Some(identity),
            kind,
        })
}

/// Whether `uri`, written in `from`, provably names no library: a package
/// its package configuration lacks, an SDK library its SDK lacks, or a
/// relative file that does not exist.
fn uri_missing(environment: &AnalysisEnvironment, from: &Path, uri: &str) -> bool {
    if let Some(rest) = uri.strip_prefix("package:") {
        let package = rest.split('/').next().unwrap_or(rest);
        return environment
            .package_config(from)
            .is_some_and(|config| config.package(package).is_none());
    }
    if uri.starts_with("dart:") {
        return environment
            .sdk(from)
            .is_some_and(|sdk| sdk.library(uri).is_none());
    }
    !uri.contains(':') && from.parent().is_some_and(|dir| !dir.join(uri).exists())
}

impl Combinators {
    fn collect<'a>(
        combinators: impl Iterator<Item = ImportCombinator<'a>>,
        interner: &mut StringInterner,
    ) -> Self {
        let mut result = Self::Hide(Vec::new());
        for combinator in combinators {
            match (&mut result, combinator) {
                (Self::Hide(hidden), ImportCombinator::Hide(names)) => {
                    hidden.extend(names.names().map(|id| interner.intern(id.name())));
                }
                (Self::Hide(hidden), ImportCombinator::Show(names)) => {
                    result = Self::Show(
                        names
                            .names()
                            .filter(|id| !hidden.iter().any(|name| name == id.name()))
                            .map(|id| interner.intern(id.name()))
                            .collect(),
                    );
                }
                (Self::Show(shown), ImportCombinator::Show(names)) => {
                    shown.retain(|name| names.names().any(|id| name == id.name()));
                }
                (Self::Show(shown), ImportCombinator::Hide(names)) => {
                    shown.retain(|name| names.names().all(|id| name != id.name()));
                }
            }
        }
        let (Self::Show(names) | Self::Hide(names)) = &mut result;
        names.sort_unstable();
        names.dedup();
        result
    }
}

fn allows(combinators: &Combinators, name: &str) -> bool {
    let (Combinators::Show(names) | Combinators::Hide(names)) = combinators;
    let contains = names
        .binary_search_by(|candidate| candidate.as_str().cmp(name))
        .is_ok();
    contains == matches!(combinators, Combinators::Show(_))
}

fn resolve_uri(
    from: &Path,
    uri: &str,
    by_path: &HashMap<PathBuf, FileId>,
    file_library: &[LibraryId],
    packages: &[PackageIdentity],
) -> ImportTarget {
    if uri.starts_with("dart:") {
        return ImportTarget::Sdk(uri.to_string());
    }
    if let Some((package, _)) = uri
        .strip_prefix("package:")
        .and_then(|rest| rest.split_once('/'))
        && !packages.iter().any(|candidate| candidate.name == package)
    {
        return ImportTarget::Package(package.to_string());
    }
    import_path(from, uri, packages)
        .and_then(|target| by_path.get(&target))
        .map(|file| ImportTarget::Library(file_library[file.0]))
        .unwrap_or(ImportTarget::Unknown)
}

/// Normalized path of the project library `uri` names when imported from
/// `from`; `None` for SDK, unknown, ambiguous or escaping package URIs.
pub fn import_path(from: &Path, uri: &str, packages: &[PackageIdentity]) -> Option<PathBuf> {
    if uri.starts_with("dart:") {
        return None;
    }
    let Some(rest) = uri.strip_prefix("package:") else {
        let parent = from.parent().unwrap_or_else(|| Path::new(""));
        return Some(canonical_or_lexical(&parent.join(uri)));
    };
    let (package, subpath) = rest.split_once('/')?;
    let mut matches = packages
        .iter()
        .filter(|candidate| candidate.name == package);
    let matched = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let lib_root = canonical_or_lexical(&matched.lib_root);
    let target = canonical_or_lexical(&lib_root.join(subpath));
    target.starts_with(&lib_root).then_some(target)
}

/// The `package:` URI naming `path` through the unique package whose `lib`
/// directory contains it, percent-encoding each UTF-8 path segment.
fn package_uri(path: &Path, packages: &[PackageIdentity]) -> Option<String> {
    let path = canonical_or_lexical(path);
    let mut matches = packages.iter().filter_map(|package| {
        let relative = path
            .strip_prefix(canonical_or_lexical(&package.lib_root))
            .ok()?;
        let mut uri = format!("package:{}", package.name);
        for component in relative.components() {
            uri.push('/');
            crate::environment::percent_encode(component.as_os_str().to_str()?, &mut uri);
        }
        Some(uri)
    });
    let uri = matches.next()?;
    matches.next().is_none().then_some(uri)
}

type Allowlist = &'static [(&'static str, &'static [&'static str])];

const SDK_TYPES: Allowlist = &[
    (
        "dart:core",
        &[
            "Object",
            "Enum",
            "Function",
            "Record",
            "Type",
            "Symbol",
            "Null",
            "Never",
            "bool",
            "num",
            "int",
            "double",
            "String",
            "Pattern",
            "Comparable",
            "RegExp",
            "Iterable",
            "Iterator",
            "List",
            "Set",
            "Map",
            "MapEntry",
            "Duration",
            "DateTime",
            "Uri",
            "BigInt",
            "Stopwatch",
            "StackTrace",
            "Expando",
            "WeakReference",
            "Finalizer",
            "dynamic",
            "Exception",
            "FormatException",
            "IntegerDivisionByZeroException",
            "Error",
            "AssertionError",
            "TypeError",
            "ArgumentError",
            "RangeError",
            "IndexError",
            "StateError",
            "UnsupportedError",
            "UnimplementedError",
            "ConcurrentModificationError",
            "OutOfMemoryError",
            "StackOverflowError",
            "NoSuchMethodError",
            "Sink",
            "Deprecated",
            "StringSink",
        ],
    ),
    (
        "dart:async",
        &[
            "Future",
            "FutureOr",
            "Timer",
            "Completer",
            "StreamController",
            "Stream",
            "StreamSubscription",
            "StreamSink",
            "StreamConsumer",
            "EventSink",
            "TimeoutException",
            "AsyncError",
        ],
    ),
    ("dart:math", &["Random"]),
    ("dart:convert", &["JsonDecoder"]),
    (
        "dart:io",
        &[
            "FileSystemEntity",
            "File",
            "Directory",
            "Link",
            "IOSink",
            "Socket",
            "IOException",
            "OSError",
            "FileSystemException",
            "HttpException",
            "SocketException",
        ],
    ),
    ("dart:ui", &["Color"]),
    (
        "dart:collection",
        &[
            "Queue",
            "ListQueue",
            "DoubleLinkedQueue",
            "HashMap",
            "HashSet",
            "LinkedHashMap",
            "LinkedHashSet",
            "SplayTreeMap",
            "SplayTreeSet",
        ],
    ),
    (
        "dart:js_interop",
        &[
            "JSAny",
            "JSObject",
            "JSFunction",
            "JSExportedDartFunction",
            "JSBoxedDartObject",
            "JSArray",
            "JSPromise",
            "JSNumber",
            "JSBoolean",
            "JSString",
            "JSSymbol",
            "JSBigInt",
            "JSArrayBuffer",
            "JSDataView",
            "JSTypedArray",
            "JSInt8Array",
            "JSUint8Array",
            "JSUint8ClampedArray",
            "JSInt16Array",
            "JSUint16Array",
            "JSInt32Array",
            "JSUint32Array",
            "JSFloat32Array",
            "JSFloat64Array",
            "JSBigInt64Array",
            "JSBigUint64Array",
        ],
    ),
    (
        "dart:html",
        &[
            "Element",
            "HtmlElement",
            "DocumentFragment",
            "AnchorElement",
            "EmbedElement",
            "IFrameElement",
            "ScriptElement",
            "DivElement",
            "SpanElement",
            "ParagraphElement",
            "BodyElement",
            "TemplateElement",
            "Window",
        ],
    ),
];

const SDK_MEMBERS: Allowlist = &[
    ("dart:core", &["print", "pragma", "deprecated", "override"]),
    ("dart:async", &["scheduleMicrotask"]),
    ("dart:convert", &["jsonDecode", "json"]),
    ("dart:js_interop", &["JS", "staticInterop"]),
    ("dart:html", &["window"]),
];

const PACKAGE_TYPES: Allowlist = &[
    ("fixnum", &["Int32", "Int64"]),
    (
        "flutter",
        &[
            "Widget",
            "StatelessWidget",
            "StatefulWidget",
            "InheritedWidget",
            "ProxyWidget",
            "RenderObjectWidget",
            "Key",
            "BuildContext",
            "State",
            "TextEditingController",
            "ScrollController",
            "PageController",
            "TabController",
            "AnimationController",
            "FocusNode",
            "Listenable",
            "ValueListenable",
            "Animation",
            "ChangeNotifier",
            "ValueNotifier",
            "Color",
            "Text",
            "Container",
            "Align",
            "Alignment",
            "AlignmentGeometry",
            "Padding",
            "EdgeInsets",
            "EdgeInsetsGeometry",
            "Matrix4",
            "ConstrainedBox",
            "BoxConstraints",
            "Transform",
            "ColoredBox",
            "ListView",
            "GridView",
            "Row",
            "Column",
            "IconButton",
            "FloatingActionButton",
            "Builder",
            "ElevatedButton",
            "GestureDetector",
            "MaterialApp",
            "SizedBox",
            "Colors",
            "Diagnosticable",
            "DiagnosticableTree",
            "DiagnosticableTreeMixin",
            "FlutterError",
        ],
    ),
    ("provider", &["Provider"]),
    (
        "nested",
        &["SingleChildStatelessWidget", "SingleChildState"],
    ),
];

/// Curated fallback identities; arities already supplied by the SDK hierarchy are omitted here.
pub(super) fn fallback_types() -> impl Iterator<Item = (DeclarationIdentity, &'static str, usize)> {
    SDK_TYPES
        .iter()
        .flat_map(|(library, names)| {
            names.iter().map(move |name| {
                let arity = match (*library, *name) {
                    ("dart:collection", "HashMap" | "LinkedHashMap" | "SplayTreeMap") => 2,
                    ("dart:core", "Expando" | "WeakReference" | "Finalizer")
                    | ("dart:async", "FutureOr")
                    | (
                        "dart:collection",
                        "ListQueue" | "DoubleLinkedQueue" | "HashSet" | "LinkedHashSet"
                        | "SplayTreeSet",
                    )
                    | ("dart:js_interop", "JSArray" | "JSPromise") => 1,
                    _ => 0,
                };
                (DeclarationIdentity::sdk(library, name), *name, arity)
            })
        })
        .chain(PACKAGE_TYPES.iter().flat_map(|(package, names)| {
            names.iter().map(move |name| {
                let arity = usize::from(*package == "flutter" && *name == "State");
                (DeclarationIdentity::package(package, name), *name, arity)
            })
        }))
}

const PACKAGE_MEMBERS: Allowlist = &[
    (
        "meta",
        &[
            "visibleForTesting",
            "protected",
            "immutable",
            "Immutable",
            "Target",
            "TargetKind",
            "awaitNotRequired",
            "optionalTypeArgs",
            "protected",
        ],
    ),
    (
        "flutter",
        &[
            "runApp",
            "debugPrint",
            "kDebugMode",
            "kReleaseMode",
            "kProfileMode",
            "kIsWeb",
            "kIsWasm",
        ],
    ),
    ("test", &["TestOn", "Timeout", "Tags", "OnPlatform", "fail"]),
    ("test_api", &["fail"]),
    ("flutter_test", &["fail"]),
];

fn allowed(list: Allowlist, library: &str, name: &str) -> Option<(&'static str, &'static str)> {
    let (library, names) = list.iter().find(|(candidate, _)| *candidate == library)?;
    let name = names.iter().find(|candidate| **candidate == name)?;
    Some((library, name))
}

fn sdk_type(library: &str, name: &str) -> Option<DeclarationIdentity> {
    // `dart:core` re-exports `Future` and `Stream` from `dart:async`.
    let library = match (library, name) {
        ("dart:core", "Future" | "Stream") => "dart:async",
        _ => library,
    };
    allowed(SDK_TYPES, library, name).map(|(library, name)| DeclarationIdentity::sdk(library, name))
}

fn sdk_member(library: &str, name: &str) -> Option<DeclarationIdentity> {
    allowed(SDK_MEMBERS, library, name)
        .map(|(library, name)| DeclarationIdentity::sdk(library, name))
}

fn package_type(package: &str, name: &str) -> Option<DeclarationIdentity> {
    allowed(PACKAGE_TYPES, package, name)
        .map(|(package, name)| DeclarationIdentity::package(package, name))
}

fn package_member(package: &str, name: &str) -> Option<DeclarationIdentity> {
    package_type(package, name).or_else(|| {
        allowed(PACKAGE_MEMBERS, package, name)
            .map(|(package, name)| DeclarationIdentity::package(package, name))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ParsedFile;
    use falcon_dart_parser::parse;

    #[test]
    fn unique_candidates_preserve_ambiguity_and_non_sdk_precedence() {
        let sdk = DeclarationIdentity::sdk("dart:core", "Object");
        let other_sdk = DeclarationIdentity::sdk("dart:async", "Future");
        let package = DeclarationIdentity::package("example", "Object");
        let other_package = DeclarationIdentity::package("other", "Object");
        assert_eq!(unique_declaration([]), None);
        assert_eq!(
            unique_declaration([sdk.clone(), sdk.clone()]),
            Some(sdk.clone())
        );
        assert_eq!(unique_declaration([sdk.clone(), other_sdk.clone()]), None);
        for candidates in [
            [sdk.clone(), other_sdk.clone(), package.clone()],
            [package.clone(), sdk.clone(), other_sdk.clone()],
        ] {
            assert_eq!(unique_declaration(candidates), Some(package.clone()));
        }
        assert_eq!(unique_declaration([package, other_package, sdk]), None);
    }

    #[test]
    fn split_package_libraries_keep_collisions_with_unsplit_siblings() {
        let root = tempfile::tempdir().unwrap();
        let shared = root.path().join("meta/lib");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            shared.join("meta.dart"),
            "import 'package:foo/foo.dart'; class Marker {} const annotation = 0;",
        )
        .unwrap();
        std::fs::write(
            shared.join("other.dart"),
            "class Marker {} const annotation = 0;",
        )
        .unwrap();
        let source = "import 'package:meta/meta.dart' as split; \
                      import 'package:meta/other.dart' as sibling;";
        let paths = ["a", "b"].map(|context| {
            let directory = root.path().join(context);
            std::fs::create_dir_all(directory.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(directory.join("foo/lib")).unwrap();
            std::fs::write(directory.join("foo/lib/foo.dart"), "class Foo {}").unwrap();
            std::fs::write(
                directory.join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[
                    {"name":"meta","rootUri":"../../meta","packageUri":"lib/"},
                    {"name":"foo","rootUri":"../foo","packageUri":"lib/"}
                ]}"#,
            )
            .unwrap();
            let path = directory.join("main.dart");
            std::fs::write(&path, source).unwrap();
            path
        });
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            paths.iter().map(PathBuf::as_path),
            |_| None,
        );
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let project = paths
            .each_ref()
            .map(|path| (path.as_path(), parsed.program(), source));
        let libraries = ExternalLibraries::load(&environment, &project, |_| false);
        let sources = paths.each_ref().map(|path| IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        });
        let external = ExternalSources {
            libraries: &libraries,
            environment: &environment,
        };
        let monolithic = IdentityIndex::build(&sources, &[source; 2], &[], Some(external));
        let overlay = IdentityIndex::with_external(
            &sources,
            &[source; 2],
            &[],
            external,
            Arc::new(IdentityIndex::build_external(external)),
        );
        for index in [&monolithic, &overlay] {
            for path in &paths {
                let library = index.library(index.file(path).unwrap());
                for name in ["Marker", "annotation"] {
                    let resolve = |prefix| {
                        let qualified = QualifiedName::prefixed(prefix, name);
                        if name == "Marker" {
                            index.resolve_declaration(library, qualified)
                        } else {
                            index.resolve_imported_member(library, qualified)
                        }
                        .unwrap()
                    };
                    let split = resolve("split");
                    let sibling = resolve("sibling");
                    assert!(matches!(split, DeclarationIdentity::Library { .. }));
                    assert!(matches!(sibling, DeclarationIdentity::Library { .. }));
                    assert_ne!(split, sibling);
                }
            }
        }
    }

    #[test]
    fn shared_root_package_copies_survive_interleaved_package_names() {
        let root = tempfile::tempdir().unwrap();
        let shared = root.path().join("shared/lib");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            shared.join("shared.dart"),
            "import 'package:foo/foo.dart'; class Marker {}",
        )
        .unwrap();
        let packages = ["bar", "baz", "bar"];
        let sources = packages.map(|name| format!("import 'package:{name}/shared.dart';"));
        let paths = std::array::from_fn::<_, 3, _>(|context| {
            let directory = root.path().join(context.to_string());
            std::fs::create_dir_all(directory.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(directory.join("foo/lib")).unwrap();
            std::fs::write(directory.join("foo/lib/foo.dart"), "class Foo {}").unwrap();
            std::fs::write(
                directory.join(".dart_tool/package_config.json"),
                format!(
                    r#"{{"configVersion":2,"packages":[
                        {{"name":"{}","rootUri":"../../shared","packageUri":"lib/"}},
                        {{"name":"foo","rootUri":"../foo","packageUri":"lib/"}}
                    ]}}"#,
                    packages[context],
                ),
            )
            .unwrap();
            let path = directory.join("main.dart");
            std::fs::write(&path, &sources[context]).unwrap();
            path
        });
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            paths.iter().map(PathBuf::as_path),
            |_| None,
        );
        let parsed = sources.each_ref().map(|source| {
            let parsed = parse(source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            Arc::new(ParsedFile {
                source: source.as_str().into(),
                tree: parsed.tree,
            })
        });
        let project = std::array::from_fn::<_, 3, _>(|index| {
            (
                paths[index].as_path(),
                parsed[index].program(),
                sources[index].as_str(),
            )
        });
        let libraries = ExternalLibraries::load(&environment, &project, |_| false);
        let inputs = std::array::from_fn::<_, 3, _>(|index| IdentitySource {
            path: &paths[index],
            parsed: &parsed[index],
            has_parse_errors: false,
        });
        let external = ExternalSources {
            libraries: &libraries,
            environment: &environment,
        };
        let texts = sources.each_ref().map(String::as_str);
        let monolithic = IdentityIndex::build(&inputs, &texts, &[], Some(external));
        let overlay = IdentityIndex::with_external(
            &inputs,
            &texts,
            &[],
            external,
            Arc::new(IdentityIndex::build_external(external)),
        );
        for index in [&monolithic, &overlay] {
            let identities = paths.each_ref().map(|path| {
                index
                    .resolve_declaration(index.library(index.file(path).unwrap()), "Marker".into())
                    .unwrap()
            });
            assert!(matches!(
                identities[0],
                DeclarationIdentity::Package { copy: Some(_), .. }
            ));
            assert!(matches!(
                identities[2],
                DeclarationIdentity::Package { copy: Some(_), .. }
            ));
            assert_ne!(identities[0], identities[2]);
        }
    }

    #[test]
    fn package_copies_share_identity_but_distinct_libraries_still_collide() {
        use crate::{AnalysisEnvironment, ExternalLibraries};

        let root = tempfile::tempdir().unwrap();
        let source = "import 'package:meta/meta.dart'; import 'package:meta/other.dart' as other;";
        let mut paths = Vec::with_capacity(2);
        for context in ["a", "b"] {
            let directory = root.path().join(context);
            std::fs::create_dir_all(directory.join(".dart_tool")).unwrap();
            std::fs::create_dir_all(directory.join("meta/lib")).unwrap();
            std::fs::write(directory.join("main.dart"), source).unwrap();
            std::fs::write(
                directory.join("meta/lib/meta.dart"),
                "class immutable {} class Marker {} const annotation = 0;",
            )
            .unwrap();
            std::fs::write(
                directory.join("meta/lib/other.dart"),
                "class Marker {} const annotation = 0;",
            )
            .unwrap();
            std::fs::write(directory.join(".dart_tool/package_config.json"),
                r#"{"configVersion":2,"packages":[{"name":"meta","rootUri":"../meta","packageUri":"lib/"}]}"#).unwrap();
            paths.push(directory.join("main.dart"));
        }
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            paths.iter().map(PathBuf::as_path),
            |_| None,
        );
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let project: Vec<_> = paths
            .iter()
            .map(|path| (path.as_path(), parsed.program(), source))
            .collect();
        let external = ExternalLibraries::load(&environment, &project, |_| false);
        assert_eq!(external.libraries().len(), 4);
        let sources: Vec<_> = paths
            .iter()
            .map(|path| IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            })
            .collect();
        let external = ExternalSources {
            libraries: &external,
            environment: &environment,
        };
        let monolithic = IdentityIndex::build(&sources, &[source, source], &[], Some(external));
        let overlay = IdentityIndex::with_external(
            &sources,
            &[source, source],
            &[],
            external,
            Arc::new(IdentityIndex::build_external(external)),
        );
        for index in [&monolithic, &overlay] {
            for path in &paths {
                let library = index.library(index.file(path).unwrap());
                let identity = index
                    .resolve_declaration(library, "immutable".into())
                    .unwrap();
                assert!(identity.is_package("meta", "immutable"), "{identity:?}");
                let first = index.resolve_declaration(library, "Marker".into()).unwrap();
                let second = index
                    .resolve_declaration(library, QualifiedName::prefixed("other", "Marker"))
                    .unwrap();
                assert!(matches!(first, DeclarationIdentity::Library { .. }));
                assert!(matches!(second, DeclarationIdentity::Library { .. }));
                assert!(!index.is_project(&first));
                assert!(!index.is_project(&second));
                assert_ne!(first, second);
                let first = index
                    .resolve_imported_member(library, "annotation".into())
                    .unwrap();
                let second = index
                    .resolve_imported_member(
                        library,
                        QualifiedName::prefixed("other", "annotation"),
                    )
                    .unwrap();
                assert!(matches!(first, DeclarationIdentity::Library { .. }));
                assert!(matches!(second, DeclarationIdentity::Library { .. }));
                assert!(!index.is_project(&first));
                assert!(!index.is_project(&second));
                assert_ne!(first, second);
            }
        }
    }

    fn index(inputs: &[(&str, &str)], packages: &[PackageIdentity]) -> IdentityIndex {
        let parsed: Vec<_> = inputs
            .iter()
            .map(|(_, source)| {
                let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
                assert!(errors.is_empty(), "{errors:?}");
                Arc::new(ParsedFile {
                    source: (*source).into(),
                    tree,
                })
            })
            .collect();
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), parsed)| IdentitySource {
                path: Path::new(path),
                parsed,
                has_parse_errors: false,
            })
            .collect();
        IdentityIndex::from_project_files(&sources, packages)
    }

    fn library(index: &IdentityIndex, file: &str) -> LibraryId {
        index.library(index.file(Path::new(file)).expect("indexed file"))
    }

    fn resolve<'n>(
        index: &IdentityIndex,
        file: &str,
        name: impl Into<QualifiedName<'n>>,
    ) -> NameIdentity {
        index.resolve(library(index, file), name.into())
    }

    fn declaration<'n>(
        index: &IdentityIndex,
        file: &str,
        name: impl Into<QualifiedName<'n>>,
    ) -> Option<DeclarationIdentity> {
        index.resolve_declaration(library(index, file), name.into())
    }

    fn package(name: &str, lib_root: &str) -> PackageIdentity {
        PackageIdentity {
            name: name.to_string(),
            lib_root: PathBuf::from(lib_root),
        }
    }

    #[cfg(unix)]
    #[test]
    fn file_lookup_normalizes_symlinks_on_a_direct_lookup_miss() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("main.dart");
        let alias = directory.path().join("alias.dart");
        std::fs::write(&path, "class Main {}").unwrap();
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        let path = path.canonicalize().unwrap();
        let index = index(&[(path.to_str().unwrap(), "class Main {}")], &[]);
        let canonical = index.file(&path);
        assert!(canonical.is_some());
        assert_eq!(index.file(&alias), canonical);
        assert_eq!(index.file(&directory.path().join("missing.dart")), None);
    }

    #[test]
    fn missing_parts_preserve_explicit_generic_type_identity() {
        let mut index = index(
            &[
                (
                    "/project/lib/box.dart",
                    "part 'missing.g.dart'; abstract class Box<T> { factory Box(T value) = Missing<T>; }",
                ),
                ("/project/lib/barrel.dart", "export 'box.dart';"),
                (
                    "/project/lib/main.dart",
                    "import 'box.dart'; import 'box.dart' as direct; import 'box.dart' deferred as later; import 'barrel.dart' as barrel; final box = Box(1);",
                ),
            ],
            &[],
        );
        let box_library = library(&index, "/project/lib/box.dart");
        assert!(index.get(box_library).incomplete);
        let expected = index.declared(box_library, "Box").unwrap();
        assert_eq!(
            index.resolve_declaration(box_library, "Box".into()),
            Some(expected.clone())
        );
        assert_eq!(index.resolve(box_library, "Box".into()), NameIdentity::Type);
        let main = library(&index, "/project/lib/main.dart");
        for prefix in [None, Some("direct"), Some("later"), Some("barrel")] {
            let name = QualifiedName {
                prefix,
                name: "Box",
            };
            assert_eq!(
                index.resolve_declaration(main, name),
                Some(expected.clone())
            );
            assert_eq!(index.resolve(main, name), NameIdentity::Type);
            for name in ["Missing", "Absent"] {
                let name = QualifiedName { prefix, name };
                assert_eq!(index.resolve_declaration(main, name), None);
                assert_eq!(index.resolve(main, name), NameIdentity::Unknown);
            }
        }
        assert!(index.target_may_hide_declaration(&ImportTarget::Library(box_library), "Missing"));
        index.libraries[box_library.0].origin =
            Some(ExternalOrigin::Package(InternedString::Static("example")));
        index.memos = LookupMemos::default();
        assert_eq!(
            index.resolve_declaration(main, "Box".into()),
            index.declared(box_library, "Box")
        );
        assert!(index.target_may_hide_declaration(&ImportTarget::Library(box_library), "Missing"));
    }

    #[test]
    fn missing_parts_do_not_disambiguate_or_invent_type_declarations() {
        let index = index(
            &[
                (
                    "/project/lib/box.dart",
                    "part 'missing.g.dart'; class Box<T> {} class Duration {}",
                ),
                ("/project/lib/other.dart", "class Box<T> {} class Absent {}"),
                (
                    "/project/lib/duplicate.dart",
                    "part 'missing.g.dart'; class Duplicate {} class Duplicate {}",
                ),
                (
                    "/project/lib/barrel.dart",
                    "export 'box.dart'; export 'other.dart';",
                ),
                (
                    "/project/lib/main.dart",
                    "import 'box.dart'; import 'box.dart' as core; import 'other.dart'; import 'barrel.dart' as barrel; import 'duplicate.dart' as duplicate;",
                ),
            ],
            &[],
        );
        let main = library(&index, "/project/lib/main.dart");
        for name in [
            QualifiedName::from("Box"),
            QualifiedName::from("Absent"),
            QualifiedName::prefixed("barrel", "Box"),
            QualifiedName::prefixed("barrel", "Absent"),
            QualifiedName::from("Duration"),
            QualifiedName::prefixed("core", "Duration"),
            QualifiedName::prefixed("duplicate", "Duplicate"),
        ] {
            assert_eq!(index.resolve_declaration(main, name), None);
            assert_eq!(index.resolve(main, name), NameIdentity::Unknown);
        }
    }

    #[test]
    fn incomplete_external_bindings_do_not_suppress_unrelated_imports() {
        let mut index = index(
            &[
                ("/project/lib/incomplete.dart", "class Broken {}"),
                ("/project/lib/barrel.dart", "export 'incomplete.dart';"),
                ("/project/lib/known.dart", "class Known {}"),
                (
                    "/project/lib/main.dart",
                    "import 'barrel.dart'; import 'known.dart';",
                ),
            ],
            &[],
        );
        let incomplete = library(&index, "/project/lib/incomplete.dart");
        index.libraries[incomplete.0].incomplete = true;
        index.libraries[incomplete.0].origin =
            Some(ExternalOrigin::Sdk(InternedString::Static("dart:test")));
        let file = "/project/lib/main.dart";
        for name in ["Known", "int", "dynamic", "Never"] {
            assert_eq!(resolve(&index, file, name), NameIdentity::Type);
            assert!(declaration(&index, file, name).is_some());
        }
        assert_eq!(declaration(&index, file, "Broken"), None);
        assert_eq!(resolve(&index, file, "Broken"), NameIdentity::Unknown);
    }

    #[test]
    fn incomplete_core_keeps_intrinsic_type_fallback() {
        let mut index = index(
            &[
                ("/project/lib/core.dart", "class Object {}"),
                ("/project/lib/main.dart", ""),
            ],
            &[],
        );
        let core = library(&index, "/project/lib/core.dart");
        index.libraries[core.0].incomplete = true;
        index.libraries[core.0].origin =
            Some(ExternalOrigin::Sdk(InternedString::Static("dart:core")));
        index.core = Some(core);
        for name in ["dynamic", "Never"] {
            assert_eq!(
                resolve(&index, "/project/lib/main.dart", name),
                NameIdentity::Type
            );
            assert!(declaration(&index, "/project/lib/main.dart", name).is_some());
        }
        assert_eq!(
            declaration(&index, "/project/lib/main.dart", "Object"),
            None
        );
    }

    #[test]
    fn uncertainty_cache_preserves_cycles_and_import_combinators() {
        let index = index(
            &[
                (
                    "/project/lib/a.dart",
                    "export 'b.dart'; export 'missing.dart';",
                ),
                ("/project/lib/b.dart", "export 'a.dart';"),
                ("/project/lib/known.dart", "class Known {}"),
                (
                    "/project/lib/main.dart",
                    "import 'a.dart' show Broken; import 'known.dart';",
                ),
            ],
            &[],
        );
        let a = library(&index, "/project/lib/a.dart");
        let b = library(&index, "/project/lib/b.dart");
        for target in [a, b, a, b] {
            assert!(index.target_may_hide_declaration(&ImportTarget::Library(target), "Broken"));
            assert_eq!(
                index.memos.uncertainty.get(target, None, "Broken"),
                Some(true)
            );
        }
        assert_eq!(index.memos.uncertainty.len(), 2);
        assert_eq!(
            resolve(&index, "/project/lib/main.dart", "Known"),
            NameIdentity::Type
        );
        assert_eq!(
            resolve(&index, "/project/lib/main.dart", "int"),
            NameIdentity::Type
        );
        assert!(
            index
                .clone()
                .memos
                .uncertainty
                .get(a, None, "Broken")
                .is_none()
        );
    }

    #[test]
    fn complete_export_cycles_cache_absent_name_uncertainty() {
        let index = index(
            &[
                ("/project/lib/a.dart", "export 'b.dart';"),
                ("/project/lib/b.dart", "export 'a.dart'; class Known {}"),
                ("/project/lib/main.dart", "import 'a.dart';"),
            ],
            &[],
        );
        let a = library(&index, "/project/lib/a.dart");
        for _ in 0..2 {
            assert!(!index.target_may_hide_declaration(&ImportTarget::Library(a), "Absent"));
            assert_eq!(index.memos.uncertainty.get(a, None, "Absent"), Some(false));
            assert_eq!(
                resolve(&index, "/project/lib/main.dart", "Known"),
                NameIdentity::Type
            );
            assert_eq!(
                resolve(&index, "/project/lib/main.dart", "int"),
                NameIdentity::Type
            );
        }
    }

    #[test]
    fn recognizes_indexed_package_declarations_without_name_guessing() {
        let index = index(
            &[
                (
                    "/flutter/lib/src/widgets/container.dart",
                    "class Container {}",
                ),
                ("/app/lib/main.dart", "class Container {}"),
            ],
            &[
                package("flutter", "/flutter/lib"),
                package("app", "/app/lib"),
            ],
        );
        let flutter = declaration(
            &index,
            "/flutter/lib/src/widgets/container.dart",
            "Container",
        )
        .unwrap();
        let local = declaration(&index, "/app/lib/main.dart", "Container").unwrap();
        assert!(index.is_package_declaration(&flutter, "flutter", "Container"));
        assert!(!index.is_package_declaration(&local, "flutter", "Container"));
        assert!(!index.is_package_declaration(&flutter, "flutter", "ColoredBox"));
        assert!(index.is_package_declaration(
            &DeclarationIdentity::flutter("Container"),
            "flutter",
            "Container"
        ));
    }

    #[test]
    fn imported_source_package_members_require_unique_declarations() {
        let index = index(
            &[
                ("/meta/lib/first.dart", "const immutable = 0;"),
                ("/meta/lib/second.dart", "const immutable = 1;"),
                (
                    "/app/lib/facade.dart",
                    "export 'package:meta/first.dart'; export 'package:meta/second.dart';",
                ),
                (
                    "/app/lib/main.dart",
                    "import 'facade.dart' as ambiguous; import 'package:meta/first.dart' as direct;",
                ),
            ],
            &[package("meta", "/meta/lib")],
        );
        let main = library(&index, "/app/lib/main.dart");
        assert_eq!(
            index.resolve_imported_member(main, QualifiedName::prefixed("direct", "immutable")),
            Some(DeclarationIdentity::package("meta", "immutable"))
        );
        assert!(!index.is_project(&DeclarationIdentity::package("meta", "immutable")));
        assert_eq!(
            index.declaration_library(&DeclarationIdentity::package("meta", "immutable")),
            None
        );
        assert_eq!(
            index.resolve_imported_member(main, QualifiedName::prefixed("ambiguous", "immutable")),
            None
        );
    }

    #[test]
    fn annotation_resolution_only_tolerates_missing_generated_parts() {
        for (body, expected) in [
            ("part 'main.g.dart';", true),
            ("part 'main.freezed.dart';", true),
            ("part 'main.gr.dart';", true),
            ("part 'main.config.dart';", true),
            ("part 'main.mocks.dart';", true),
            ("part 'main.chopper.dart';", true),
            ("part 'main.mapper.dart';", true),
            ("part 'main.helpers.dart';", false),
            ("part './main.g.dart';", true),
            ("part 'other.shared.dart';", false),
            ("part 'sub/main.g.dart';", false),
            ("part 'main.extra.g.dart';", false),
            ("part 'main..dart';", false),
            ("part 'manual.dart';", false),
            ("part 'main.g.dart'; part 'manual.dart';", false),
            ("part 'main.g.txt';", false),
            ("part 'main.g.dart'; const immutable = 0;", false),
            ("import 'other.dart'; part 'main.g.dart';", false),
        ] {
            let source = format!("import 'package:meta/meta.dart'; {body}");
            let index = index(
                &[
                    ("/app/lib/main.dart", &source),
                    ("/app/lib/other.dart", "const immutable = 0;"),
                ],
                &[],
            );
            let main = library(&index, "/app/lib/main.dart");
            assert_eq!(
                index.resolve_imported_member(main, "immutable".into()),
                None
            );
            assert_eq!(
                index.resolve_annotation_before_generation(main, "immutable".into()),
                expected.then(|| DeclarationIdentity::package("meta", "immutable")),
                "{source}",
            );
        }
    }

    #[test]
    fn annotation_resolution_rejects_missing_owners_and_parse_errors() {
        let index = index(
            &[
                (
                    "/app/lib/main.dart",
                    "import 'package:meta/meta.dart'; part 'main.g.dart'; part 'other.dart';",
                ),
                ("/app/lib/other.dart", "part of 'missing.dart';"),
            ],
            &[],
        );
        let main = library(&index, "/app/lib/main.dart");
        assert_eq!(
            index.resolve_annotation_before_generation(main, "immutable".into()),
            None,
        );

        let source = "import 'package:meta/meta.dart'; part 'main.g.dart'; class";
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(!errors.is_empty());
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let index = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: Path::new("/app/lib/main.dart"),
                parsed: &parsed,
                has_parse_errors: true,
            }],
            &[],
        );
        let main = library(&index, "/app/lib/main.dart");
        assert_eq!(
            index.resolve_annotation_before_generation(main, "immutable".into()),
            None,
        );
    }

    #[test]
    fn cached_combinators_match_ast_semantics_including_recovery() {
        let mut interner = StringInterner::default();
        for suffix in [
            "",
            "show Foo",
            "hide Foo",
            "show Foo, Bar, Foo show Bar, Baz",
            "hide Foo hide Bar, Foo",
            "hide Foo show Foo, Bar",
            "show Foo, Bar hide Foo",
            "show Foo hide Foo show Foo",
            "show",
            "hide",
            "show Foo,",
            "hide Foo,",
            "show show Foo",
            "hide show Foo",
        ] {
            for directive in ["import", "export"] {
                let source = format!("{directive} 'a.dart' {suffix};");
                let parsed = parse(&source);
                let parsed = ParsedFile {
                    source: source.clone().into_boxed_str(),
                    tree: parsed.tree,
                };
                let program = parsed.program();
                let combinators: Vec<_> = if directive == "import" {
                    program.imports().next().unwrap().combinators().collect()
                } else {
                    program.exports().next().unwrap().combinators().collect()
                };
                let cached = Combinators::collect(combinators.iter().copied(), &mut interner);
                for name in ["", "Foo", "Bar", "Baz", "Unknown"] {
                    let expected = combinators.iter().all(|combinator| match combinator {
                        ImportCombinator::Show(names) => names.names().any(|id| id.name() == name),
                        ImportCombinator::Hide(names) => names.names().all(|id| id.name() != name),
                    });
                    assert_eq!(allows(&cached, name), expected, "{source}: {name:?}");
                }
            }
        }
    }

    #[test]
    fn intersects_import_and_export_show_lists_and_unions_hides() {
        let index = index(
            &[
                (
                    "/project/lib/a.dart",
                    "class Foo {} class Bar {} class Baz {}",
                ),
                (
                    "/project/lib/b.dart",
                    "export 'a.dart' show Foo, Bar, Baz show Bar, Baz hide Baz;",
                ),
                (
                    "/project/lib/c.dart",
                    "import 'b.dart' as p show Foo, Bar hide Foo; \
                     import 'a.dart' as q hide Foo hide Baz;",
                ),
            ],
            &[],
        );
        for prefix in ["p", "q"] {
            for (name, expected) in [
                ("Foo", NameIdentity::Unknown),
                ("Bar", NameIdentity::Type),
                ("Baz", NameIdentity::Unknown),
            ] {
                assert_eq!(
                    resolve(
                        &index,
                        "/project/lib/c.dart",
                        QualifiedName::prefixed(prefix, name),
                    ),
                    expected
                );
            }
        }
    }

    #[test]
    fn resolves_imports_prefixes_combinators_and_ambiguity() {
        let index = index(
            &[
                (
                    "/project/lib/a.dart",
                    "class Foo {} typedef Alias = Foo; class Dup {} final value = 0;",
                ),
                ("/project/lib/c.dart", "class Dup {}"),
                (
                    "/project/lib/b.dart",
                    "import 'a.dart' as p show Foo, Alias; import 'a.dart' show Dup; import 'c.dart' show Dup;",
                ),
            ],
            &[],
        );
        let file = "/project/lib/b.dart";

        assert_eq!(
            resolve(&index, file, QualifiedName::prefixed("p", "Foo")),
            NameIdentity::Type
        );
        assert_eq!(
            resolve(&index, file, QualifiedName::prefixed("p", "Alias")),
            NameIdentity::Type
        );
        assert_eq!(
            resolve(&index, file, QualifiedName::prefixed("p", "value")),
            NameIdentity::Unknown
        );
        assert_eq!(resolve(&index, file, "Dup"), NameIdentity::Unknown);
        assert_eq!(resolve(&index, file, "int"), NameIdentity::Type);
    }

    #[test]
    fn accessor_pairs_share_identity_without_hiding_duplicate_declarations() {
        let index = index(
            &[
                (
                    "/project/lib/a.dart",
                    "dynamic get data => null; set data(dynamic value) {} \
                     set reversed(dynamic value) {} dynamic get reversed => null; \
                     dynamic get duplicate => null; dynamic get duplicate => null; \
                     set duplicate(dynamic value) {} \
                     dynamic get mixed => null; void mixed() {} set mixed(dynamic value) {}",
                ),
                ("/project/lib/b.dart", "import 'a.dart';"),
            ],
            &[],
        );
        let file = "/project/lib/b.dart";
        for name in ["data", "reversed"] {
            assert_eq!(resolve(&index, file, name), NameIdentity::Value);
            assert!(
                index
                    .resolve_value_declaration(library(&index, file), name.into())
                    .is_some()
            );
        }
        for name in ["duplicate", "mixed"] {
            assert_eq!(resolve(&index, file, name), NameIdentity::Unknown);
            assert_eq!(
                index.resolve_value_declaration(library(&index, file), name.into()),
                None
            );
        }
    }

    #[test]
    fn setter_first_accessor_pair_uses_getter_part_as_source_owner() {
        let index = index(
            &[
                (
                    "/project/lib/root.dart",
                    "library example; part 'getter.dart'; set data(dynamic value) {}",
                ),
                (
                    "/project/lib/getter.dart",
                    "part of example; dynamic get data => null;",
                ),
                ("/project/lib/consumer.dart", "import 'root.dart';"),
            ],
            &[],
        );
        assert_eq!(
            resolve(&index, "/project/lib/consumer.dart", "data"),
            NameIdentity::Value
        );
        assert!(
            index
                .resolve_value_declaration(
                    library(&index, "/project/lib/consumer.dart"),
                    "data".into()
                )
                .is_some()
        );
        assert_eq!(
            index.declaration_file(library(&index, "/project/lib/root.dart"), "data"),
            Some(FileId(1))
        );
    }

    #[test]
    fn library_lookup_normalizes_path_spellings() {
        let index = index(&[("/project/lib/a.dart", "class Foo {}")], &[]);
        assert_eq!(
            index.file(Path::new("/project/lib/../lib/./a.dart")),
            index.file(Path::new("/project/lib/a.dart"))
        );
        assert!(index.file(Path::new("/project/lib/a.dart")).is_some());
    }

    #[test]
    fn explicit_core_imports_replace_the_implicit_namespace() {
        for source in [
            "import 'dart:core' hide Duration, print;",
            "import 'dart:core' show int;",
            "import 'dart:core' as core;",
        ] {
            let file = "/project/lib/core.dart";
            let index = index(&[(file, source)], &[]);
            assert_eq!(resolve(&index, file, "Duration"), NameIdentity::Unknown);
            assert_eq!(declaration(&index, file, "Duration"), None);
            assert_eq!(
                index.resolve_sdk_member(library(&index, file), "print".into()),
                None
            );
            if source.contains("as core") {
                let duration = QualifiedName::prefixed("core", "Duration");
                assert_eq!(resolve(&index, file, duration), NameIdentity::Type);
                assert!(declaration(&index, file, duration).is_some());
            } else {
                assert_eq!(resolve(&index, file, "int"), NameIdentity::Type);
                assert!(declaration(&index, file, "int").is_some());
            }
        }
    }

    #[test]
    fn reexports_preserve_original_declaration_identity() {
        let index = index(
            &[
                (
                    "/project/lib/origin.dart",
                    "class Box<T> {} const answer = 42;",
                ),
                ("/project/lib/barrel.dart", "export 'origin.dart';"),
                ("/project/lib/chain.dart", "export 'barrel.dart';"),
                (
                    "/project/lib/main.dart",
                    "import 'chain.dart'; Box<int>? value;",
                ),
            ],
            &[],
        );
        let origin = library(&index, "/project/lib/origin.dart");
        let main = library(&index, "/project/lib/main.dart");
        assert_eq!(
            index.resolve_declaration(main, "Box".into()),
            index.resolve_declaration(origin, "Box".into())
        );
        let origin_value = index.resolve_value_declaration(origin, "answer".into());
        assert!(origin_value.is_some());
        assert_eq!(
            index.resolve_value_declaration(main, "answer".into()),
            origin_value
        );
    }

    #[test]
    fn named_extensions_resolve_through_prefixes_and_reexports() {
        let index = index(
            &[
                (
                    "/project/lib/origin.dart",
                    "extension Ex on int { int value() => this; }",
                ),
                ("/project/lib/barrel.dart", "export 'origin.dart' show Ex;"),
                (
                    "/project/lib/main.dart",
                    "import 'barrel.dart'; import 'barrel.dart' as p show Ex;",
                ),
                (
                    "/project/lib/hidden.dart",
                    "import 'barrel.dart' hide Ex; import 'barrel.dart' as p hide Ex;",
                ),
                (
                    "/project/lib/shadow.dart",
                    "import 'barrel.dart'; int Ex = 0;",
                ),
            ],
            &[],
        );
        let origin = library(&index, "/project/lib/origin.dart");
        let extension = index.resolve_extension_declaration(origin, "Ex".into());
        assert!(extension.is_some());
        let main = library(&index, "/project/lib/main.dart");
        assert_eq!(
            index.resolve_extension_declaration(main, "Ex".into()),
            extension
        );
        assert_eq!(
            index.resolve_extension_declaration(main, QualifiedName::prefixed("p", "Ex")),
            extension
        );
        let hidden = library(&index, "/project/lib/hidden.dart");
        assert_eq!(
            index.resolve_extension_declaration(hidden, "Ex".into()),
            None
        );
        assert_eq!(
            index.resolve_extension_declaration(hidden, QualifiedName::prefixed("p", "Ex")),
            None
        );
        let shadow = library(&index, "/project/lib/shadow.dart");
        assert_eq!(
            index.resolve_extension_declaration(shadow, "Ex".into()),
            None
        );
    }

    #[test]
    fn named_extension_lookups_cache_hits_misses_and_prefixes() {
        let index = index(
            &[
                ("/project/lib/ext.dart", "extension Ex on int {}"),
                ("/project/lib/barrel.dart", "export 'ext.dart';"),
                (
                    "/project/lib/main.dart",
                    "import 'barrel.dart'; import 'barrel.dart' as p;",
                ),
            ],
            &[],
        );
        let main = library(&index, "/project/lib/main.dart");
        let expected = index.declared(library(&index, "/project/lib/ext.dart"), "Ex");
        for _ in 0..2 {
            for prefix in [None, Some("p")] {
                for (name, answer) in [("Ex", expected.clone()), ("Absent", None)] {
                    assert_eq!(
                        index.resolve_extension_declaration(main, QualifiedName { prefix, name }),
                        answer
                    );
                    assert_eq!(
                        index.memos.named_extensions.get(main, prefix, name),
                        Some(answer)
                    );
                }
            }
        }
        assert_eq!(index.memos.named_extensions.len(), 4);
        assert_eq!(index.clone().memos.named_extensions.len(), 0);
    }

    #[test]
    fn named_extension_export_cache_is_shared_without_caching_partial_cycles() {
        let index = index(
            &[
                (
                    "/project/lib/a.dart",
                    "export 'b.dart'; extension Ex on int {}",
                ),
                ("/project/lib/b.dart", "export 'a.dart';"),
                ("/project/lib/first.dart", "import 'b.dart';"),
                ("/project/lib/second.dart", "import 'b.dart';"),
                ("/project/lib/third.dart", "import 'a.dart';"),
            ],
            &[],
        );
        let a = library(&index, "/project/lib/a.dart");
        let b = library(&index, "/project/lib/b.dart");
        let expected = index.declared(a, "Ex");
        for path in ["/project/lib/first.dart", "/project/lib/second.dart"] {
            let importer = library(&index, path);
            assert_eq!(
                index.resolve_extension_declaration(importer, "Ex".into()),
                expected
            );
            assert_eq!(
                index.resolve_extension_declaration(importer, "Absent".into()),
                None
            );
            assert_eq!(index.memos.extension_exports.len(), 2);
        }
        assert_eq!(
            index.memos.extension_exports.get(b, None, "Ex"),
            Some(Ok(expected.clone()))
        );
        assert_eq!(index.memos.extension_exports.get(a, None, "Absent"), None);
        assert_eq!(
            index.resolve_extension_declaration(
                library(&index, "/project/lib/third.dart"),
                "Absent".into()
            ),
            None
        );
        assert_eq!(
            index.memos.extension_exports.get(a, None, "Absent"),
            Some(Ok(None))
        );
        assert_eq!(index.clone().memos.extension_exports.len(), 0);
    }

    #[test]
    fn incomplete_extension_exports_only_block_names_they_may_hide() {
        let mut index = index(
            &[
                ("/project/lib/broken.dart", "extension Broken on int {}"),
                (
                    "/project/lib/barrel.dart",
                    "export 'broken.dart'; export 'known.dart';",
                ),
                (
                    "/project/lib/known.dart",
                    "extension Known on int {} extension Broken on String {}",
                ),
                (
                    "/project/lib/main.dart",
                    "import 'barrel.dart'; import 'known.dart' as p;",
                ),
            ],
            &[],
        );
        let broken = library(&index, "/project/lib/broken.dart");
        index.libraries[broken.0].incomplete = true;
        index.libraries[broken.0].origin =
            Some(ExternalOrigin::Package(InternedString::Static("example")));
        let main = library(&index, "/project/lib/main.dart");
        let known = library(&index, "/project/lib/known.dart");
        assert_eq!(
            index.resolve_extension_declaration(main, "Known".into()),
            index.declared(known, "Known")
        );
        // An incomplete external binding of this exact name may conflict.
        assert_eq!(
            index.resolve_extension_declaration(main, "Broken".into()),
            None
        );
        assert_eq!(
            index.resolve_extension_declaration(main, QualifiedName::prefixed("p", "Broken")),
            index.declared(known, "Broken")
        );
        // Missing project input can hide any exported name.
        let mut project_incomplete = index.clone();
        project_incomplete.libraries[broken.0].origin = None;
        assert_eq!(
            project_incomplete.resolve_extension_declaration(main, "Known".into()),
            None
        );
    }

    #[test]
    fn incomplete_external_libraries_preserve_unrelated_reexports() {
        let mut index = index(
            &[
                (
                    "/project/lib/broken.dart",
                    "export 'barrel.dart'; export 'known.dart' hide Hidden; \
                     extension Broken on int {} class BrokenType {} final brokenValue = 0;",
                ),
                ("/project/lib/barrel.dart", "export 'broken.dart';"),
                (
                    "/project/lib/known.dart",
                    "extension Known on int {} extension Broken on String {} \
                     extension Hidden on int {} class KnownType {} class BrokenType {} \
                     final knownValue = 0; final brokenValue = 0;",
                ),
                (
                    "/project/lib/main.dart",
                    "import 'barrel.dart'; import 'barrel.dart' as p;",
                ),
            ],
            &[],
        );
        let broken = library(&index, "/project/lib/broken.dart");
        index.libraries[broken.0].incomplete = true;
        index.libraries[broken.0].origin =
            Some(ExternalOrigin::Package(InternedString::Static("example")));
        let main = library(&index, "/project/lib/main.dart");
        let known = library(&index, "/project/lib/known.dart");
        for _ in 0..2 {
            for prefix in [None, Some("p")] {
                assert_eq!(
                    index.resolve_extension_declaration(
                        main,
                        QualifiedName {
                            prefix,
                            name: "Known"
                        }
                    ),
                    index.declared(known, "Known")
                );
                assert_eq!(
                    index.resolve_declaration(
                        main,
                        QualifiedName {
                            prefix,
                            name: "KnownType"
                        }
                    ),
                    index.declared(known, "KnownType")
                );
                assert_eq!(
                    index.resolve_value_declaration(
                        main,
                        QualifiedName {
                            prefix,
                            name: "knownValue"
                        }
                    ),
                    index.declared(known, "knownValue")
                );
                for name in ["Broken", "Hidden"] {
                    assert_eq!(
                        index.resolve_extension_declaration(main, QualifiedName { prefix, name }),
                        None
                    );
                }
                assert_eq!(
                    index.resolve_declaration(
                        main,
                        QualifiedName {
                            prefix,
                            name: "BrokenType"
                        }
                    ),
                    None
                );
                assert_eq!(
                    index.resolve_value_declaration(
                        main,
                        QualifiedName {
                            prefix,
                            name: "brokenValue"
                        }
                    ),
                    None
                );
            }
        }
        let mut project_incomplete = index.clone();
        project_incomplete.libraries[broken.0].origin = None;
        assert_eq!(
            project_incomplete.resolve_extension_declaration(main, "Known".into()),
            None
        );
        assert_eq!(
            project_incomplete.resolve_declaration(main, "KnownType".into()),
            None
        );
        assert_eq!(
            project_incomplete.resolve_value_declaration(main, "knownValue".into()),
            None
        );
    }

    #[test]
    fn external_named_extension_identity_keeps_declaring_library() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let app = root.join("app");
        let dep = root.join("dep");
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(app.join("lib")).unwrap();
        std::fs::create_dir_all(dep.join("lib")).unwrap();
        std::fs::write(
            app.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[
                {"name":"app","rootUri":"../","packageUri":"lib/"},
                {"name":"dep","rootUri":"../../dep","packageUri":"lib/"}
            ]}"#,
        )
        .unwrap();
        let extension_path = dep.join("lib/extension.dart");
        std::fs::write(
            &extension_path,
            "const _limit = 5; extension Ex on int { void take([int n = _limit]) {} }",
        )
        .unwrap();
        std::fs::write(dep.join("lib/api.dart"), "export 'extension.dart' show Ex;").unwrap();
        let path = app.join("lib/main.dart");
        let source = "import 'package:dep/api.dart' as p; const _limit = 9;";
        std::fs::write(&path, source).unwrap();
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
        let external = ExternalLibraries::load(
            &environment,
            &[(path.as_path(), parsed.program(), source)],
            |candidate| candidate == path,
        );
        let index = IdentityIndex::build(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[source],
            &[],
            Some(ExternalSources {
                libraries: &external,
                environment: &environment,
            }),
        );
        let identity = index
            .resolve_extension_declaration(
                library(&index, path.to_str().unwrap()),
                QualifiedName::prefixed("p", "Ex"),
            )
            .expect("loaded dependency extension");
        assert!(identity.is_package("dep", "Ex"));
        assert_eq!(
            index.declaration_library(&identity),
            Some(library(&index, extension_path.to_str().unwrap()))
        );
        let sources = ExternalSources {
            libraries: &external,
            environment: &environment,
        };
        let base = Arc::new(IdentityIndex::build_external(sources));
        let external_file = base.file(&extension_path).unwrap();
        let external_library = base.library(external_file);
        let project_paths = [
            path.clone(),
            app.join("lib/one.dart"),
            app.join("lib/two.dart"),
        ];
        let project: Vec<_> = project_paths
            .iter()
            .map(|path| IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            })
            .collect();
        for project_count in [1, 3] {
            let overlay = IdentityIndex::with_external(
                &project[..project_count],
                &[source; 3][..project_count],
                &[PackageIdentity {
                    name: "dep".to_string(),
                    lib_root: dep.join("lib"),
                }],
                sources,
                Arc::clone(&base),
            );
            assert_eq!(overlay.file(&extension_path), Some(external_file));
            assert_eq!(overlay.library(external_file), external_library);
            assert_eq!(
                overlay.source_path(external_file),
                Some(extension_path.as_path())
            );
            assert_eq!(
                overlay.resolve_extension_declaration(
                    library(&overlay, path.to_str().unwrap()),
                    QualifiedName::prefixed("p", "Ex"),
                ),
                Some(identity.clone())
            );
            assert_eq!(
                overlay.declaration_library(&identity),
                Some(external_library)
            );
            assert!(overlay.is_package_declaration(
                &DeclarationIdentity::Library {
                    library: external_library,
                    name: "_limit".into()
                },
                "dep",
                "_limit",
            ));
        }
    }

    #[test]
    fn ambiguous_named_extension_exports_do_not_resolve() {
        let index = index(
            &[
                ("/project/lib/first.dart", "extension Ex on int {}"),
                ("/project/lib/second.dart", "extension Ex on String {}"),
                (
                    "/project/lib/barrel.dart",
                    "export 'first.dart'; export 'second.dart';",
                ),
                (
                    "/project/lib/main.dart",
                    "import 'barrel.dart'; import 'first.dart';",
                ),
            ],
            &[],
        );
        assert_eq!(
            index.resolve_extension_declaration(
                library(&index, "/project/lib/main.dart"),
                "Ex".into()
            ),
            None
        );
    }

    #[test]
    fn duplicate_paths_to_same_declaration_converge() {
        let index = index(
            &[
                ("/project/lib/origin.dart", "class Box<T> {}"),
                ("/project/lib/barrel.dart", "export 'origin.dart';"),
                (
                    "/project/lib/main.dart",
                    "import 'origin.dart'; import 'barrel.dart'; Box<int>? value;",
                ),
            ],
            &[],
        );
        let main = "/project/lib/main.dart";
        assert_eq!(resolve(&index, main, "Box"), NameIdentity::Type);
        assert_eq!(
            declaration(&index, main, "Box"),
            declaration(&index, "/project/lib/origin.dart", "Box")
        );
    }

    #[test]
    fn external_package_import_does_not_bind_project_library() {
        let index = index(
            &[
                ("/workspace/lib/foo.dart", "class Foo {}"),
                (
                    "/workspace/lib/main.dart",
                    "import 'package:other/foo.dart'; Foo? value;",
                ),
            ],
            &[],
        );
        let file = "/workspace/lib/main.dart";
        assert_eq!(resolve(&index, file, "Foo"), NameIdentity::Unknown);
        assert_eq!(declaration(&index, file, "Foo"), None);
    }

    #[test]
    fn current_package_import_binds_project_library_when_owner_known() {
        let index = index(
            &[
                ("/workspace/lib/foo.dart", "class Foo {}"),
                (
                    "/workspace/lib/main.dart",
                    "import 'package:workspace/foo.dart'; Foo? value;",
                ),
            ],
            &[package("workspace", "/workspace/lib")],
        );
        let file = "/workspace/lib/main.dart";
        assert_eq!(resolve(&index, file, "Foo"), NameIdentity::Type);
        assert_eq!(
            declaration(&index, file, "Foo"),
            declaration(&index, "/workspace/lib/foo.dart", "Foo")
        );
    }

    #[test]
    fn package_imports_resolve_unique_workspace_packages_across_owners() {
        let index = index(
            &[
                ("/workspace/lib/foo.dart", "class OuterFoo {}"),
                (
                    "/workspace/lib/main.dart",
                    "import 'package:outer/foo.dart'; OuterFoo? value;",
                ),
                ("/workspace/tools/inner/lib/foo.dart", "class InnerFoo {}"),
                (
                    "/workspace/tools/inner/lib/main.dart",
                    "import 'package:inner/foo.dart'; import 'package:outer/foo.dart'; InnerFoo? own; OuterFoo? dependency;",
                ),
            ],
            &[
                package("outer", "/workspace/lib"),
                package("inner", "/workspace/tools/inner/lib"),
            ],
        );
        let inner = "/workspace/tools/inner/lib/main.dart";
        assert_eq!(resolve(&index, inner, "InnerFoo"), NameIdentity::Type);
        assert_eq!(resolve(&index, inner, "OuterFoo"), NameIdentity::Type);
        assert_eq!(
            declaration(&index, inner, "OuterFoo"),
            declaration(&index, "/workspace/lib/foo.dart", "OuterFoo")
        );
        assert_eq!(
            resolve(&index, "/workspace/lib/main.dart", "OuterFoo"),
            NameIdentity::Type
        );
    }

    #[test]
    fn duplicate_workspace_package_names_are_ambiguous() {
        let index = index(
            &[
                (
                    "/workspace/packages/first/lib/api.dart",
                    "class FirstApi {}",
                ),
                (
                    "/workspace/packages/second/lib/api.dart",
                    "class SecondApi {}",
                ),
                (
                    "/workspace/packages/consumer/lib/main.dart",
                    "import 'package:shared/api.dart'; FirstApi? first; SecondApi? second;",
                ),
            ],
            &[
                package("shared", "/workspace/packages/first/lib"),
                package("shared", "/workspace/packages/second/lib"),
            ],
        );
        let importer = "/workspace/packages/consumer/lib/main.dart";
        assert_eq!(resolve(&index, importer, "FirstApi"), NameIdentity::Unknown);
        assert_eq!(
            resolve(&index, importer, "SecondApi"),
            NameIdentity::Unknown
        );
    }

    #[test]
    fn package_import_target_cannot_escape_exact_lib_root() {
        let index = index(
            &[
                (
                    "/workspace/packages/b/private/api.dart",
                    "class PrivateApi {}",
                ),
                (
                    "/workspace/packages/a/lib/main.dart",
                    "import 'package:b/../private/api.dart'; PrivateApi? value;",
                ),
            ],
            &[package("b", "/workspace/packages/b/lib")],
        );
        assert_eq!(
            resolve(&index, "/workspace/packages/a/lib/main.dart", "PrivateApi"),
            NameIdentity::Unknown
        );
    }

    #[test]
    fn named_extensions_keep_external_origin_identity() {
        for (origin, expected) in [
            (
                ExternalOrigin::Sdk(InternedString::Static("dart:example")),
                DeclarationIdentity::sdk("dart:example", "Ex"),
            ),
            (
                ExternalOrigin::Package(InternedString::Static("example")),
                DeclarationIdentity::package("example", "Ex"),
            ),
        ] {
            let mut index = index(
                &[
                    (
                        "/project/lib/ext.dart",
                        "extension Ex on int { void m() {} }",
                    ),
                    ("/project/lib/main.dart", "import 'ext.dart';"),
                ],
                &[],
            );
            let extension_library = library(&index, "/project/lib/ext.dart");
            index.libraries[extension_library.0].origin = Some(origin);
            assert_eq!(
                index.resolve_extension_declaration(
                    library(&index, "/project/lib/main.dart"),
                    QualifiedName::from("Ex"),
                ),
                Some(expected),
            );
        }
    }

    #[test]
    fn shares_declarations_across_parts() {
        let index = index(
            &[
                (
                    "/project/lib/root.dart",
                    "library example; part 'part.dart';",
                ),
                (
                    "/project/lib/part.dart",
                    "part of example; class PartType {}",
                ),
            ],
            &[],
        );
        for file in ["/project/lib/root.dart", "/project/lib/part.dart"] {
            assert_eq!(resolve(&index, file, "PartType"), NameIdentity::Type);
            assert_eq!(
                index.declaration_file(library(&index, file), "PartType"),
                Some(FileId(1))
            );
        }
    }

    #[test]
    fn duplicate_part_declarations_have_no_source_owner() {
        let index = index(
            &[
                (
                    "/project/lib/root.dart",
                    "library example; part 'part.dart'; class Duplicate {}",
                ),
                (
                    "/project/lib/part.dart",
                    "part of example; class Duplicate {}",
                ),
            ],
            &[],
        );
        for file in ["/project/lib/root.dart", "/project/lib/part.dart"] {
            assert_eq!(resolve(&index, file, "Duplicate"), NameIdentity::Unknown);
            assert_eq!(declaration(&index, file, "Duplicate"), None);
            assert_eq!(
                index.declaration_file(library(&index, file), "Duplicate"),
                None
            );
        }
    }

    #[test]
    fn top_level_accessors_share_one_value_identity() {
        for source in [
            "int get value => 0; set value(int next) {}",
            "set value(int next) {} int get value => 0;",
        ] {
            let file = "/project/lib/main.dart";
            let index = index(&[(file, source)], &[]);
            assert_eq!(resolve(&index, file, "value"), NameIdentity::Value);
            assert!(
                index
                    .resolve_value_declaration(library(&index, file), "value".into())
                    .is_some()
            );
        }
    }

    #[test]
    fn typed_source_selectors_distinguish_member_categories_and_parameters() {
        use MemberSelectorKind as Kind;
        let file = "/project/lib/main.dart";
        let index = index(
            &[(
                file,
                r"
typedef Dep = Deprecated;
class Marker { const Marker(String message); }
typedef NotDep = Marker;
class C {
  @deprecated int get value => 0;
  @deprecated set value(int next) {}
  @deprecated static int get shared => 0;
  @deprecated int field = 0;
  @deprecated C.named([@deprecated int? argument]);
  @deprecated C operator -() => this;
  @deprecated C operator -(C other) => this;
  static void method({@deprecated int? argument}) {}
  @Dep('old') void aliased() {}
  @Marker('not deprecated') void marked() {}
  @NotDep('not deprecated') void aliasedMarker() {}
}
",
            )],
            &[],
        );
        let owner = declaration(&index, file, "C").unwrap();
        let aliased = MemberSelector::new(Kind::Read, false, "aliased");
        assert!(index.source_annotations(&owner, aliased.into()).is_some());
        for name in ["marked", "aliasedMarker"] {
            let selector = MemberSelector::new(Kind::Read, false, name);
            assert!(index.source_annotations(&owner, selector.into()).is_none());
        }
        for selector in [
            MemberSelector::new(Kind::Read, false, "value"),
            MemberSelector::new(Kind::Write, false, "value"),
            MemberSelector::new(Kind::Read, true, "shared"),
            MemberSelector::new(Kind::Field, false, "field"),
            MemberSelector::new(Kind::Constructor, false, "named"),
            MemberSelector::new(Kind::Operator(0), false, "-"),
            MemberSelector::new(Kind::Operator(1), false, "-"),
        ] {
            assert!(
                index.source_annotations(&owner, selector.into()).is_some(),
                "{selector:?}"
            );
        }
        for selector in [
            MemberSelector::new(Kind::Read, true, "value"),
            MemberSelector::new(Kind::Read, false, "shared"),
            MemberSelector::new(Kind::Write, true, "shared"),
            MemberSelector::new(Kind::Read, false, "field"),
            MemberSelector::new(Kind::Read, false, "named"),
            MemberSelector::new(Kind::Operator(2), false, "-"),
        ] {
            assert!(
                index.source_annotations(&owner, selector.into()).is_none(),
                "{selector:?}"
            );
        }
        let constructor = MemberSelector::new(Kind::Constructor, false, "named");
        assert_eq!(
            index.deprecated_positional_parameter(&owner, constructor, 0),
            Some("argument")
        );
        assert_eq!(
            index.deprecated_positional_parameter(&owner, constructor, 1),
            None
        );
        let method = index
            .static_member_selector(&owner, "method", false)
            .unwrap();
        assert!(index.source_annotations(&owner, method.into()).is_none());
        assert!(
            index
                .source_annotations(
                    &owner,
                    SourceSelector {
                        member: method,
                        parameter: Some("argument"),
                    }
                )
                .is_some()
        );
        assert!(
            index
                .source_annotations(
                    &owner,
                    SourceSelector {
                        member: constructor,
                        parameter: Some("missing"),
                    }
                )
                .is_none()
        );
    }

    #[test]
    fn accessor_pairs_across_parts_retain_both_metadata_selectors() {
        let root = "/project/lib/root.dart";
        let part = "/project/lib/part.dart";
        let index = index(
            &[
                (
                    root,
                    "library example; part 'part.dart'; @deprecated int get value => 0;",
                ),
                (part, "part of example; @deprecated set value(int next) {}"),
            ],
            &[],
        );
        for file in [root, part] {
            let owner = index
                .resolve_value_declaration(library(&index, file), "value".into())
                .unwrap();
            assert_eq!(
                index
                    .source_annotations(&owner, MemberSelector::DECLARATION.into())
                    .unwrap()
                    .0,
                Path::new(root)
            );
            assert_eq!(
                index
                    .source_annotations(
                        &owner,
                        MemberSelector::new(MemberSelectorKind::Write, false, "").into()
                    )
                    .unwrap()
                    .0,
                Path::new(part)
            );
            let setter = MemberSelector::new(MemberSelectorKind::Write, false, "");
            assert!(index.source_annotations(&owner, setter.into()).is_some());
            assert!(
                index
                    .source_annotations(
                        &owner,
                        SourceSelector {
                            member: MemberSelector::new(MemberSelectorKind::Write, false, "value"),
                            parameter: None,
                        }
                    )
                    .is_none()
            );
        }
    }

    #[test]
    fn accessor_collisions_remain_ambiguous() {
        for source in [
            "int get value => 0; int get value => 1;",
            "set value(int next) {} set value(int next) {}",
            "int get value => 0; set value(int next) {} int get value => 1;",
            "int get value => 0; set value(int next) {} set value(int next) {}",
            "int value = 0; int get value => 1; set value(int next) {}",
            "int get value => 0; int value = 1; set value(int next) {}",
            "int get value => 0; set value(int next) {} int value = 1;",
        ] {
            let file = "/project/lib/main.dart";
            let index = index(&[(file, source)], &[]);
            assert_eq!(resolve(&index, file, "value"), NameIdentity::Unknown);
            assert!(
                index
                    .resolve_value_declaration(library(&index, file), "value".into())
                    .is_none()
            );
        }
    }

    #[test]
    fn missing_sources_do_not_invent_new_fallback_members() {
        assert_eq!(sdk_member("dart:core", "identical"), None);
        assert_eq!(package_member("flutter", "immutable"), None);
    }

    #[test]
    fn unnamed_library_metadata_comes_from_loaded_sources() {
        let index = index(
            &[
                ("/project/a.dart", "class A {}"),
                ("/project/b.dart", "library named; class B {}"),
                ("/project/c.dart", "library; class C {}"),
            ],
            &[],
        );
        for (path, name, expected) in [
            ("/project/a.dart", "A", true),
            ("/project/b.dart", "B", false),
            ("/project/c.dart", "C", true),
        ] {
            let identity = index.declared(library(&index, path), name).unwrap();
            assert_eq!(
                index.identity_is_in_unnamed_library(&identity),
                Some(expected)
            );
        }
        assert_eq!(
            index.identity_is_in_unnamed_library(&DeclarationIdentity::OBJECT),
            None
        );
    }
}
