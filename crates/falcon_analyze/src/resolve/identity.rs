//! Conservative identity resolution for expression identifiers.
//!
//! This is intentionally smaller than a Dart element model. It answers only
//! whether a simple or import-prefixed name denotes a type, a value, or cannot
//! be resolved uniquely. Ambiguity and incomplete inputs always become
//! [`NameIdentity::Unknown`].

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use falcon_syntax::ast::{Identifier, ImportCombinator, Program, TopLevelDecl};

use super::group_libraries;
use super::library::canonical_or_lexical;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameIdentity {
    Type,
    Value,
    Unknown,
}

pub struct IdentitySource<'a> {
    pub path: &'a Path,
    pub program: &'a Program,
    pub has_parse_errors: bool,
}

#[derive(Debug, Clone)]
pub struct PackageIdentity {
    pub name: String,
    pub lib_root: PathBuf,
}

/// A library (a defining file plus its parts) of an [`IdentityIndex`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    pub fn from_identifiers(segments: &'a [Identifier]) -> Option<Self> {
        match segments {
            [name] => Some((&name.name).into()),
            [prefix, name] => Some(Self::prefixed(&prefix.name, &name.name)),
            _ => None,
        }
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
    declarations: HashMap<String, NameIdentity>,
    imports: Vec<Import>,
    exports: Vec<Export>,
    incomplete: bool,
    implicit_core: bool,
}

#[derive(Debug, Clone)]
struct Import {
    target: ImportTarget,
    prefix: Option<String>,
    combinators: Vec<ImportCombinator>,
}

#[derive(Debug, Clone)]
struct Export {
    target: ImportTarget,
    combinators: Vec<ImportCombinator>,
}

/// SDK and package identities come from closed allowlists, so their names are
/// the allowlists' own `'static` strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeclarationIdentity {
    Project {
        library: LibraryId,
        name: String,
    },
    Sdk {
        library: &'static str,
        name: &'static str,
    },
    Package {
        package: &'static str,
        name: &'static str,
    },
}

impl DeclarationIdentity {
    pub const OBJECT: Self = Self::Sdk {
        library: "dart:core",
        name: "Object",
    };

    pub fn name(&self) -> &str {
        match self {
            Self::Project { name, .. } => name,
            Self::Sdk { name, .. } | Self::Package { name, .. } => name,
        }
    }

    pub fn is_sdk(&self, library: &str, name: &str) -> bool {
        matches!(self, Self::Sdk { library: found_library, name: found_name }
            if *found_library == library && *found_name == name)
    }

    pub fn is_package(&self, package: &str, name: &str) -> bool {
        matches!(self, Self::Package { package: found_package, name: found_name }
            if *found_package == package && *found_name == name)
    }
}

#[derive(Debug, Clone)]
enum ImportTarget {
    Library(LibraryId),
    Sdk(String),
    Package(String),
    Unknown,
}

/// Project-wide library/name identity index.
#[derive(Debug, Clone, Default)]
pub struct IdentityIndex {
    libraries: Vec<Library>,
    file_library: Vec<LibraryId>,
    by_path: HashMap<PathBuf, FileId>,
}

impl IdentityIndex {
    pub fn from_project_files(files: &[IdentitySource<'_>], packages: &[PackageIdentity]) -> Self {
        let grouped_input: Vec<(PathBuf, &Program)> = files
            .iter()
            .map(|file| (file.path.to_path_buf(), file.program))
            .collect();
        let grouping = group_libraries(&grouped_input);

        let mut root_to_library = HashMap::new();
        let mut libraries = Vec::<Library>::new();
        let file_library: Vec<LibraryId> = (0..files.len())
            .map(|i| {
                let root = grouping.siblings(i).iter().copied().fold(i, usize::min);
                *root_to_library.entry(root).or_insert_with(|| {
                    libraries.push(Library {
                        declarations: HashMap::new(),
                        imports: Vec::new(),
                        exports: Vec::new(),
                        incomplete: grouping.is_unresolved(i),
                        implicit_core: true,
                    });
                    LibraryId(libraries.len() - 1)
                })
            })
            .collect();

        let mut by_path = HashMap::new();
        for (index, (file, &library)) in files.iter().zip(&file_library).enumerate() {
            by_path.insert(canonical_or_lexical(file.path), FileId(index));
            let library = &mut libraries[library.0];
            library.incomplete |= file.has_parse_errors;
            for declaration in &file.program.declarations {
                for (name, identity) in declaration_identities(declaration) {
                    merge(&mut library.declarations, name, identity);
                }
            }
        }

        for (file, &library) in files.iter().zip(&file_library) {
            let library = &mut libraries[library.0];
            for import in &file.program.imports {
                if import.uri.value == "dart:core" {
                    library.implicit_core = false;
                }
                library.imports.push(Import {
                    target: resolve_uri(
                        file.path,
                        &import.uri.value,
                        &by_path,
                        &file_library,
                        packages,
                    ),
                    prefix: import.as_name.as_ref().map(|id| id.name.clone()),
                    combinators: import.combinators.clone(),
                });
            }
            for export in &file.program.exports {
                library.exports.push(Export {
                    target: resolve_uri(
                        file.path,
                        &export.uri.value,
                        &by_path,
                        &file_library,
                        packages,
                    ),
                    combinators: export.combinators.clone(),
                });
            }
        }

        Self {
            libraries,
            file_library,
            by_path,
        }
    }

    /// The indexed file at `path`, normalizing its spelling.
    pub fn file(&self, path: &Path) -> Option<FileId> {
        self.by_path.get(&canonical_or_lexical(path)).copied()
    }

    pub fn library(&self, file: FileId) -> LibraryId {
        self.file_library[file.0]
    }

    fn get(&self, library: LibraryId) -> &Library {
        &self.libraries[library.0]
    }

    fn complete(&self, library: LibraryId) -> Option<&Library> {
        Some(self.get(library)).filter(|library| !library.incomplete)
    }

    /// Classify `name` as it is visible from `library`.
    pub fn resolve(&self, library: LibraryId, name: QualifiedName<'_>) -> NameIdentity {
        if self.complete(library).is_none() {
            return NameIdentity::Unknown;
        }
        match name.prefix {
            None => self.resolve_unprefixed(library, name.name),
            Some(prefix) => self.resolve_prefixed(library, prefix, name.name),
        }
    }

    pub fn extension_visible(
        &self,
        from: LibraryId,
        declaring: LibraryId,
        extension_name: Option<&str>,
    ) -> bool {
        from == declaring
            || self.get(from).imports.iter().any(|import| {
                import.prefix.is_none()
                    && extension_name.is_none_or(|name| allows(&import.combinators, name))
                    && matches!(import.target, ImportTarget::Library(library) if self.library_exports(library, declaring, extension_name, &mut HashSet::new()))
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

    /// Resolve a written type name to its canonical declaring library identity.
    /// Ambiguous imports, values shadowing types, and incomplete libraries return
    /// `None` rather than guessing from the final name segment.
    pub fn resolve_declaration(
        &self,
        library: LibraryId,
        name: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        self.complete(library)?;
        match name.prefix {
            None => self.resolve_unprefixed_declaration(library, name.name),
            Some(prefix) => self.resolve_prefixed_declaration(library, prefix, name.name),
        }
    }

    /// Resolve a visible top-level value to its canonical declaring library.
    pub fn resolve_value_declaration(
        &self,
        library: LibraryId,
        QualifiedName { prefix, name }: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.complete(library)?;
        if prefix.is_none() {
            match unit.declarations.get(name) {
                Some(NameIdentity::Value) => {
                    return Some(DeclarationIdentity::Project {
                        library,
                        name: name.to_string(),
                    });
                }
                Some(_) => return None,
                None => {}
            }
        }
        let candidates = self
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
            .collect();
        unique_declaration(candidates)
    }

    /// Resolve an imported SDK or package member without guessing from its name.
    ///
    /// Unlike [`Self::resolve_declaration`], this also supports annotations and
    /// other top-level values from external libraries. Project declarations are
    /// intentionally excluded because their value/type namespace is indexed
    /// separately.
    pub fn resolve_imported_member(
        &self,
        library: LibraryId,
        QualifiedName { prefix, name }: QualifiedName<'_>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.complete(library)?;
        if prefix.is_none() && unit.declarations.contains_key(name) {
            return None;
        }
        let candidates = self
            .visible_imports(unit, prefix, name)
            .filter_map(|import| match &import.target {
                ImportTarget::Sdk(uri) => sdk_type(uri, name).or_else(|| sdk_member(uri, name)),
                ImportTarget::Package(package) => package_member(package, name),
                ImportTarget::Library(_) | ImportTarget::Unknown => None,
            })
            .collect();
        unique_declaration(candidates)
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
        let mut candidates: Vec<_> = self
            .visible_imports(unit, prefix, name)
            .filter_map(|import| match &import.target {
                ImportTarget::Sdk(uri) => sdk_member(uri, name),
                _ => None,
            })
            .collect();
        if prefix.is_none() && unit.implicit_core {
            candidates.extend(sdk_member("dart:core", name));
        }
        unique_declaration(candidates)
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

    fn resolve_unprefixed_declaration(
        &self,
        library: LibraryId,
        name: &str,
    ) -> Option<DeclarationIdentity> {
        let unit = self.get(library);
        if let Some(identity) = unit.declarations.get(name) {
            return (*identity == NameIdentity::Type).then(|| DeclarationIdentity::Project {
                library,
                name: name.to_string(),
            });
        }
        let mut candidates = Vec::new();
        for import in self.visible_imports(unit, None, name) {
            if matches!(import.target, ImportTarget::Unknown) {
                return None;
            }
            candidates.extend(self.target_declaration(&import.target, name));
        }
        if unit.implicit_core {
            candidates.extend(sdk_type("dart:core", name));
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
        let mut candidates = Vec::new();
        for import in self.visible_imports(unit, Some(prefix), name) {
            if matches!(import.target, ImportTarget::Unknown) {
                return None;
            }
            candidates.extend(self.target_declaration(&import.target, name));
        }
        unique_declaration(candidates)
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
        if let Some(identity) = unit.declarations.get(name) {
            return *identity;
        }
        if self.resolve_unprefixed_declaration(library, name).is_some() {
            return NameIdentity::Type;
        }
        let mut result = None;
        for import in self.visible_imports(unit, None, name) {
            combine(&mut result, self.target_identity(&import.target, name));
        }
        if unit.implicit_core && sdk_type("dart:core", name).is_some() {
            combine(&mut result, NameIdentity::Type);
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
        let mut result = None;
        for import in self.visible_imports(unit, Some(prefix), name) {
            combine(&mut result, self.target_identity(&import.target, name));
        }
        result.unwrap_or(NameIdentity::Unknown)
    }

    fn exported_declaration(
        &self,
        library: LibraryId,
        name: &str,
        namespace: NameIdentity,
        visiting: &mut HashSet<LibraryId>,
    ) -> Option<DeclarationIdentity> {
        let unit = self.get(library);
        if name.starts_with('_') || unit.incomplete || !visiting.insert(library) {
            return None;
        }
        if unit.declarations.get(name) == Some(&namespace) {
            visiting.remove(&library);
            return Some(DeclarationIdentity::Project {
                library,
                name: name.to_string(),
            });
        }
        let candidates = unit
            .exports
            .iter()
            .filter(|export| allows(&export.combinators, name))
            .filter_map(|export| match &export.target {
                ImportTarget::Library(target) => {
                    self.exported_declaration(*target, name, namespace, visiting)
                }
                ImportTarget::Sdk(uri) if namespace == NameIdentity::Type => sdk_type(uri, name),
                ImportTarget::Package(package) if namespace == NameIdentity::Type => {
                    package_type(package, name)
                }
                _ => None,
            })
            .collect();
        visiting.remove(&library);
        unique_declaration(candidates)
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
        if let Some(identity) = unit.declarations.get(name) {
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

fn declaration_identities(declaration: &TopLevelDecl) -> Vec<(&str, NameIdentity)> {
    match declaration {
        TopLevelDecl::Class(x) => vec![(&x.name.name, NameIdentity::Type)],
        TopLevelDecl::ClassTypeAlias(x) => vec![(&x.name.name, NameIdentity::Type)],
        TopLevelDecl::Mixin(x) => vec![(&x.name.name, NameIdentity::Type)],
        TopLevelDecl::MixinClass(x) => vec![(&x.name.name, NameIdentity::Type)],
        TopLevelDecl::Enum(x) => vec![(&x.name.name, NameIdentity::Type)],
        TopLevelDecl::ExtensionType(x) => vec![(&x.name.name, NameIdentity::Type)],
        TopLevelDecl::TypeAlias(x) => vec![(&x.name.name, NameIdentity::Type)],
        TopLevelDecl::Function(x) => vec![(&x.name.name, NameIdentity::Value)],
        TopLevelDecl::Variable(x) => x
            .declarators
            .iter()
            .map(|declarator| (declarator.name.name.as_str(), NameIdentity::Value))
            .collect(),
        TopLevelDecl::Extension(_) | TopLevelDecl::Error(_) => Vec::new(),
    }
}

fn merge(map: &mut HashMap<String, NameIdentity>, name: &str, identity: NameIdentity) {
    map.entry(name.to_string())
        .and_modify(|existing| *existing = NameIdentity::Unknown)
        .or_insert(identity);
}

fn unique_declaration(candidates: Vec<DeclarationIdentity>) -> Option<DeclarationIdentity> {
    let mut candidates = candidates.into_iter();
    let first = candidates.next()?;
    candidates
        .all(|candidate| candidate == first)
        .then_some(first)
}

fn combine(result: &mut Option<NameIdentity>, identity: NameIdentity) {
    match result {
        None => *result = Some(identity),
        // A name two imports can both supply is ambiguous, even when both
        // supply the same kind of thing.
        Some(existing) => *existing = NameIdentity::Unknown,
    }
}

fn allows(combinators: &[ImportCombinator], name: &str) -> bool {
    combinators.iter().all(|combinator| match combinator {
        ImportCombinator::Show(names, _) => names.iter().any(|id| id.name == name),
        ImportCombinator::Hide(names, _) => names.iter().all(|id| id.name != name),
    })
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
        ],
    ),
    ("dart:async", &["Future", "FutureOr", "Timer", "Completer"]),
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
];

const SDK_MEMBERS: Allowlist = &[
    ("dart:core", &["print", "pragma"]),
    ("dart:async", &["scheduleMicrotask"]),
    ("dart:js_interop", &["JS", "staticInterop"]),
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
            "Color",
            "Text",
            "Container",
            "Builder",
            "ElevatedButton",
            "GestureDetector",
            "MaterialApp",
        ],
    ),
];

const PACKAGE_MEMBERS: Allowlist = &[
    (
        "meta",
        &["visibleForTesting", "immutable", "Target", "TargetKind"],
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
    ("test", &["TestOn", "Timeout", "Tags", "OnPlatform"]),
];

fn allowed(list: Allowlist, library: &str, name: &str) -> Option<(&'static str, &'static str)> {
    let (library, names) = list.iter().find(|(candidate, _)| *candidate == library)?;
    let name = names.iter().find(|candidate| **candidate == name)?;
    Some((library, name))
}

fn sdk_type(library: &str, name: &str) -> Option<DeclarationIdentity> {
    allowed(SDK_TYPES, library, name)
        .map(|(library, name)| DeclarationIdentity::Sdk { library, name })
}

fn sdk_member(library: &str, name: &str) -> Option<DeclarationIdentity> {
    allowed(SDK_MEMBERS, library, name)
        .map(|(library, name)| DeclarationIdentity::Sdk { library, name })
}

fn package_type(package: &str, name: &str) -> Option<DeclarationIdentity> {
    allowed(PACKAGE_TYPES, package, name)
        .map(|(package, name)| DeclarationIdentity::Package { package, name })
}

fn package_member(package: &str, name: &str) -> Option<DeclarationIdentity> {
    package_type(package, name).or_else(|| {
        allowed(PACKAGE_MEMBERS, package, name)
            .map(|(package, name)| DeclarationIdentity::Package { package, name })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    fn index(inputs: &[(&str, &str)], packages: &[PackageIdentity]) -> IdentityIndex {
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, errors))| {
                assert!(errors.is_empty(), "{errors:?}");
                IdentitySource {
                    path: Path::new(path),
                    program,
                    has_parse_errors: false,
                }
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
        }
    }
}
