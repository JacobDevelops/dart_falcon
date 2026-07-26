//! Conservative identity resolution for expression identifiers.
//!
//! This is intentionally smaller than a Dart element model. It answers only
//! whether a simple or import-prefixed name denotes a type, a value, or cannot
//! be resolved uniquely. Ambiguity and incomplete inputs always become
//! [`NameIdentity::Unknown`].

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use falcon_syntax::ast::{ImportCombinator, Program, TopLevelDecl};

use super::group_libraries;

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

#[derive(Debug, Clone)]
struct Library {
    declarations: HashMap<String, NameIdentity>,
    imports: Vec<Import>,
    exports: Vec<Export>,
    incomplete: bool,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DeclarationIdentity {
    Project { library: usize, name: String },
    Sdk { library: String, name: String },
    Package { package: String, name: String },
}

#[derive(Debug, Clone)]
enum ImportTarget {
    Library(usize),
    Sdk(String),
    Package(String),
    Unknown,
}

/// Project-wide library/name identity index.
#[derive(Debug, Clone, Default)]
pub struct IdentityIndex {
    libraries: Vec<Library>,
    by_path: HashMap<PathBuf, usize>,
}

impl IdentityIndex {
    pub fn from_project_files(files: &[IdentitySource<'_>], packages: &[PackageIdentity]) -> Self {
        let grouped_input: Vec<(PathBuf, &Program)> = files
            .iter()
            .map(|file| (file.path.to_path_buf(), file.program))
            .collect();
        let grouping = group_libraries(&grouped_input);

        let mut root_to_library = HashMap::new();
        let mut file_library = vec![0; files.len()];
        let mut libraries = Vec::<Library>::new();
        for (i, file_library_slot) in file_library.iter_mut().enumerate() {
            let root = grouping
                .siblings(i)
                .iter()
                .copied()
                .chain(std::iter::once(i))
                .min()
                .unwrap_or(i);
            let library = *root_to_library.entry(root).or_insert_with(|| {
                let index = libraries.len();
                libraries.push(Library {
                    declarations: HashMap::new(),
                    imports: Vec::new(),
                    exports: Vec::new(),
                    incomplete: grouping.is_unresolved(i),
                });
                index
            });
            *file_library_slot = library;
        }

        let mut by_path = HashMap::new();
        for (i, file) in files.iter().enumerate() {
            let library = file_library[i];
            by_path.insert(normalize(file.path), library);
            libraries[library].incomplete |= file.has_parse_errors;
            for declaration in &file.program.declarations {
                for (name, identity) in declaration_identities(declaration) {
                    merge(&mut libraries[library].declarations, name, identity);
                }
            }
        }

        for (i, file) in files.iter().enumerate() {
            let library = file_library[i];
            for import in &file.program.imports {
                libraries[library].imports.push(Import {
                    target: resolve_uri(file.path, &import.uri.value, &by_path, packages),
                    prefix: import.as_name.as_ref().map(|id| id.name.clone()),
                    combinators: import.combinators.clone(),
                });
            }
            for export in &file.program.exports {
                libraries[library].exports.push(Export {
                    target: resolve_uri(file.path, &export.uri.value, &by_path, packages),
                    combinators: export.combinators.clone(),
                });
            }
        }

        Self { libraries, by_path }
    }

    /// Resolve a one-segment name or a two-segment import-prefixed name as it is
    /// visible from `file`. Longer dotted names denote member access, not a type
    /// literal identity.
    pub fn resolve(&self, file: &Path, segments: &[String]) -> NameIdentity {
        let Some(&library) = self.by_path.get(&normalize(file)) else {
            return NameIdentity::Unknown;
        };
        if self.libraries[library].incomplete {
            return NameIdentity::Unknown;
        }
        match segments {
            [name] => self.resolve_unprefixed(library, name),
            [prefix, name] => self.resolve_prefixed(library, prefix, name),
            _ => NameIdentity::Unknown,
        }
    }

    pub(super) fn library_identity(&self, file: &Path) -> Option<usize> {
        self.by_path.get(&normalize(file)).copied()
    }

    pub fn extension_visible(
        &self,
        file: &Path,
        declaring_file: &Path,
        extension_name: Option<&str>,
    ) -> bool {
        let Some(&from) = self.by_path.get(&normalize(file)) else {
            return false;
        };
        let Some(&target) = self.by_path.get(&normalize(declaring_file)) else {
            return false;
        };
        if from == target {
            return true;
        }
        self.libraries[from].imports.iter().any(|import| {
            import.prefix.is_none()
                && extension_name.is_none_or(|name| allows(&import.combinators, name))
                && matches!(import.target, ImportTarget::Library(library) if self.library_exports(library, target, extension_name, &mut HashSet::new()))
        })
    }

    fn library_exports(
        &self,
        library: usize,
        target: usize,
        extension_name: Option<&str>,
        visited: &mut HashSet<usize>,
    ) -> bool {
        if library == target {
            return true;
        }
        if !visited.insert(library) {
            return false;
        }
        self.libraries[library].exports.iter().any(|export| {
            extension_name.is_none_or(|name| allows(&export.combinators, name))
                && matches!(export.target, ImportTarget::Library(next) if self.library_exports(next, target, extension_name, visited))
        })
    }

    /// Resolve a written type name to its canonical declaring library identity.
    /// Ambiguous imports, values shadowing types, and incomplete libraries return
    /// `None` rather than guessing from the final name segment.
    pub fn resolve_declaration(
        &self,
        file: &Path,
        segments: &[String],
    ) -> Option<DeclarationIdentity> {
        let &library = self.by_path.get(&normalize(file))?;
        if self.libraries[library].incomplete {
            return None;
        }
        match segments {
            [name] => self.resolve_unprefixed_declaration(library, name),
            [prefix, name] => self.resolve_prefixed_declaration(library, prefix, name),
            _ => None,
        }
    }

    /// Resolve a visible top-level value to its canonical declaring library.
    pub fn resolve_value_declaration(
        &self,
        file: &Path,
        segments: &[String],
    ) -> Option<DeclarationIdentity> {
        let &library = self.by_path.get(&normalize(file))?;
        if self.libraries[library].incomplete {
            return None;
        }
        let (prefix, name) = match segments {
            [name] => (None, name.as_str()),
            [prefix, name] => (Some(prefix.as_str()), name.as_str()),
            _ => return None,
        };
        if prefix.is_none() {
            if self.libraries[library].declarations.get(name) == Some(&NameIdentity::Value) {
                return Some(DeclarationIdentity::Project {
                    library,
                    name: name.to_string(),
                });
            }
            if self.libraries[library].declarations.contains_key(name) {
                return None;
            }
        }
        let candidates = self.libraries[library]
            .imports
            .iter()
            .filter(|import| {
                import.prefix.as_deref() == prefix && allows(&import.combinators, name)
            })
            .filter_map(|import| match &import.target {
                ImportTarget::Library(target) => {
                    self.exported_value(*target, name, &mut HashSet::new())
                }
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
        file: &Path,
        segments: &[String],
    ) -> Option<DeclarationIdentity> {
        let &library = self.by_path.get(&normalize(file))?;
        if self.libraries[library].incomplete {
            return None;
        }
        let (prefix, name) = match segments {
            [name] => (None, name.as_str()),
            [prefix, name] => (Some(prefix.as_str()), name.as_str()),
            _ => return None,
        };
        if prefix.is_none() && self.libraries[library].declarations.contains_key(name) {
            return None;
        }
        let candidates = self.libraries[library]
            .imports
            .iter()
            .filter(|import| {
                import.prefix.as_deref() == prefix && allows(&import.combinators, name)
            })
            .filter_map(|import| match &import.target {
                ImportTarget::Sdk(uri) if is_sdk_type(uri, name) || is_sdk_member(uri, name) => {
                    Some(DeclarationIdentity::Sdk {
                        library: uri.clone(),
                        name: name.to_string(),
                    })
                }
                ImportTarget::Package(package) if is_known_package_member(package, name) => {
                    Some(DeclarationIdentity::Package {
                        package: package.clone(),
                        name: name.to_string(),
                    })
                }
                ImportTarget::Library(_)
                | ImportTarget::Sdk(_)
                | ImportTarget::Package(_)
                | ImportTarget::Unknown => None,
            })
            .collect();
        unique_declaration(candidates)
    }

    /// Resolve an imported SDK top-level member without guessing from its name.
    pub fn resolve_sdk_member(
        &self,
        file: &Path,
        segments: &[String],
    ) -> Option<DeclarationIdentity> {
        let &library = self.by_path.get(&normalize(file))?;
        if self.libraries[library].incomplete {
            return None;
        }
        let (prefix, name) = match segments {
            [name] => (None, name.as_str()),
            [prefix, name] => (Some(prefix.as_str()), name.as_str()),
            _ => return None,
        };
        if prefix.is_none() && self.libraries[library].declarations.contains_key(name) {
            return None;
        }
        let imports = &self.libraries[library].imports;
        let mut candidates: Vec<_> = imports
            .iter()
            .filter(|import| {
                import.prefix.as_deref() == prefix && allows(&import.combinators, name)
            })
            .filter_map(|import| match &import.target {
                ImportTarget::Sdk(uri) if is_sdk_member(uri, name) => {
                    Some(DeclarationIdentity::Sdk {
                        library: uri.clone(),
                        name: name.to_string(),
                    })
                }
                _ => None,
            })
            .collect();
        let has_explicit_core = imports.iter().any(|import| {
            import.prefix.is_none()
                && matches!(&import.target, ImportTarget::Sdk(uri) if uri == "dart:core")
        });
        if prefix.is_none() && !has_explicit_core && is_sdk_member("dart:core", name) {
            candidates.push(DeclarationIdentity::Sdk {
                library: "dart:core".to_string(),
                name: name.to_string(),
            });
        }
        unique_declaration(candidates)
    }

    fn resolve_unprefixed_declaration(
        &self,
        library: usize,
        name: &str,
    ) -> Option<DeclarationIdentity> {
        if self.libraries[library].declarations.contains_key(name) {
            return (self.libraries[library].declarations.get(name) == Some(&NameIdentity::Type))
                .then(|| DeclarationIdentity::Project {
                    library,
                    name: name.to_string(),
                });
        }
        let mut candidates = Vec::new();
        for import in &self.libraries[library].imports {
            if import.prefix.is_some() || !allows(&import.combinators, name) {
                continue;
            }
            if matches!(import.target, ImportTarget::Unknown) {
                return None;
            }
            if let Some(candidate) = self.target_declaration(&import.target, name) {
                candidates.push(candidate);
            }
        }
        if !self.libraries[library].imports.iter().any(|import| {
            import.prefix.is_none()
                && matches!(&import.target, ImportTarget::Sdk(uri) if uri == "dart:core")
        }) && is_sdk_type("dart:core", name)
        {
            candidates.push(DeclarationIdentity::Sdk {
                library: "dart:core".to_string(),
                name: name.to_string(),
            });
        }
        unique_declaration(candidates)
    }

    fn resolve_prefixed_declaration(
        &self,
        library: usize,
        prefix: &str,
        name: &str,
    ) -> Option<DeclarationIdentity> {
        if self.libraries[library].declarations.contains_key(prefix) {
            return None;
        }
        let imports = self.libraries[library].imports.iter().filter(|import| {
            import.prefix.as_deref() == Some(prefix) && allows(&import.combinators, name)
        });
        let mut candidates = Vec::new();
        for import in imports {
            if matches!(import.target, ImportTarget::Unknown) {
                return None;
            }
            if let Some(candidate) = self.target_declaration(&import.target, name) {
                candidates.push(candidate);
            }
        }
        unique_declaration(candidates)
    }

    fn target_declaration(&self, target: &ImportTarget, name: &str) -> Option<DeclarationIdentity> {
        match target {
            ImportTarget::Library(library) => {
                self.exported_declaration(*library, name, &mut HashSet::new())
            }
            ImportTarget::Sdk(uri) if is_sdk_type(uri, name) => Some(DeclarationIdentity::Sdk {
                library: uri.clone(),
                name: name.to_string(),
            }),
            ImportTarget::Package(package) if is_known_package_type(package, name) => {
                Some(DeclarationIdentity::Package {
                    package: package.clone(),
                    name: name.to_string(),
                })
            }
            ImportTarget::Sdk(_) | ImportTarget::Package(_) | ImportTarget::Unknown => None,
        }
    }

    fn resolve_unprefixed(&self, library: usize, name: &str) -> NameIdentity {
        if let Some(identity) = self.libraries[library].declarations.get(name) {
            return *identity;
        }

        let imports = self.libraries[library]
            .imports
            .iter()
            .filter(|import| import.prefix.is_none() && allows(&import.combinators, name))
            .collect::<Vec<_>>();
        let has_explicit_core = imports
            .iter()
            .any(|import| matches!(&import.target, ImportTarget::Sdk(uri) if uri == "dart:core"));
        if self.resolve_unprefixed_declaration(library, name).is_some() {
            return NameIdentity::Type;
        }

        let mut result = None;
        for import in imports {
            combine(&mut result, self.target_identity(&import.target, name));
        }
        if !has_explicit_core && is_core_type(name) {
            combine(&mut result, NameIdentity::Type);
        }
        result.unwrap_or(NameIdentity::Unknown)
    }

    fn resolve_prefixed(&self, library: usize, prefix: &str, name: &str) -> NameIdentity {
        if self.libraries[library].declarations.contains_key(prefix) {
            return NameIdentity::Unknown;
        }
        let imports = self.libraries[library]
            .imports
            .iter()
            .filter(|import| {
                import.prefix.as_deref() == Some(prefix) && allows(&import.combinators, name)
            })
            .collect::<Vec<_>>();
        if self
            .resolve_prefixed_declaration(library, prefix, name)
            .is_some()
        {
            return NameIdentity::Type;
        }

        let mut result = None;
        for import in &imports {
            combine(&mut result, self.target_identity(&import.target, name));
        }
        if imports.is_empty() {
            NameIdentity::Unknown
        } else {
            result.unwrap_or(NameIdentity::Unknown)
        }
    }

    fn exported_value(
        &self,
        library: usize,
        name: &str,
        visiting: &mut HashSet<usize>,
    ) -> Option<DeclarationIdentity> {
        if name.starts_with('_') || self.libraries[library].incomplete || !visiting.insert(library)
        {
            return None;
        }
        if self.libraries[library].declarations.get(name) == Some(&NameIdentity::Value) {
            visiting.remove(&library);
            return Some(DeclarationIdentity::Project {
                library,
                name: name.to_string(),
            });
        }
        let candidates = self.libraries[library]
            .exports
            .iter()
            .filter(|export| allows(&export.combinators, name))
            .filter_map(|export| match &export.target {
                ImportTarget::Library(target) => self.exported_value(*target, name, visiting),
                _ => None,
            })
            .collect();
        visiting.remove(&library);
        unique_declaration(candidates)
    }

    fn exported_declaration(
        &self,
        library: usize,
        name: &str,
        visiting: &mut HashSet<usize>,
    ) -> Option<DeclarationIdentity> {
        if name.starts_with('_') || self.libraries[library].incomplete || !visiting.insert(library)
        {
            return None;
        }
        if self.libraries[library].declarations.get(name) == Some(&NameIdentity::Type) {
            visiting.remove(&library);
            return Some(DeclarationIdentity::Project {
                library,
                name: name.to_string(),
            });
        }
        let candidates = self.libraries[library]
            .exports
            .iter()
            .filter(|export| allows(&export.combinators, name))
            .filter_map(|export| match &export.target {
                ImportTarget::Library(target) => self.exported_declaration(*target, name, visiting),
                ImportTarget::Sdk(uri) if is_sdk_type(uri, name) => {
                    Some(DeclarationIdentity::Sdk {
                        library: uri.clone(),
                        name: name.to_string(),
                    })
                }
                ImportTarget::Package(package) if is_known_package_type(package, name) => {
                    Some(DeclarationIdentity::Package {
                        package: package.clone(),
                        name: name.to_string(),
                    })
                }
                _ => None,
            })
            .collect();
        visiting.remove(&library);
        unique_declaration(candidates)
    }

    fn exported(&self, library: usize, name: &str, visiting: &mut HashSet<usize>) -> NameIdentity {
        if name.starts_with('_') || self.libraries[library].incomplete || !visiting.insert(library)
        {
            return NameIdentity::Unknown;
        }
        if let Some(identity) = self.libraries[library].declarations.get(name) {
            visiting.remove(&library);
            return *identity;
        }
        let mut result = None;
        for export in &self.libraries[library].exports {
            if !allows(&export.combinators, name) {
                continue;
            }
            let identity = match &export.target {
                ImportTarget::Library(target) => self.exported(*target, name, visiting),
                ImportTarget::Sdk(uri) if is_sdk_type(uri, name) => NameIdentity::Type,
                ImportTarget::Package(package) if is_known_package_type(package, name) => {
                    NameIdentity::Type
                }
                ImportTarget::Sdk(_) | ImportTarget::Package(_) | ImportTarget::Unknown => {
                    NameIdentity::Unknown
                }
            };
            combine(&mut result, identity);
        }
        visiting.remove(&library);
        result.unwrap_or(NameIdentity::Unknown)
    }

    fn target_identity(&self, target: &ImportTarget, name: &str) -> NameIdentity {
        match target {
            ImportTarget::Library(library) => self.exported(*library, name, &mut HashSet::new()),
            ImportTarget::Sdk(uri) if is_sdk_type(uri, name) => NameIdentity::Type,
            ImportTarget::Package(package) if is_known_package_type(package, name) => {
                NameIdentity::Type
            }
            ImportTarget::Sdk(_) | ImportTarget::Package(_) | ImportTarget::Unknown => {
                NameIdentity::Unknown
            }
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
        Some(existing) if *existing == identity && identity != NameIdentity::Unknown => {
            *existing = NameIdentity::Unknown;
        }
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
    by_path: &HashMap<PathBuf, usize>,
    packages: &[PackageIdentity],
) -> ImportTarget {
    if uri.starts_with("dart:") {
        return ImportTarget::Sdk(uri.to_string());
    }
    if let Some(rest) = uri.strip_prefix("package:") {
        let Some((package, subpath)) = rest.split_once('/') else {
            return ImportTarget::Unknown;
        };
        let mut matches = packages
            .iter()
            .filter(|candidate| candidate.name == package);
        let Some(matched) = matches.next() else {
            return ImportTarget::Package(package.to_string());
        };
        if matches.next().is_some() {
            return ImportTarget::Unknown;
        }
        let lib_root = normalize(&matched.lib_root);
        let target = normalize(&lib_root.join(subpath));
        if !target.starts_with(&lib_root) {
            return ImportTarget::Unknown;
        }
        return by_path
            .get(&target)
            .copied()
            .map(ImportTarget::Library)
            .unwrap_or(ImportTarget::Unknown);
    }
    let parent = from.parent().unwrap_or_else(|| Path::new(""));
    by_path
        .get(&normalize(&parent.join(uri)))
        .copied()
        .map(ImportTarget::Library)
        .unwrap_or(ImportTarget::Unknown)
}

fn normalize(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        let mut out = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                        out.pop();
                    } else if !matches!(out.components().next_back(), Some(Component::RootDir)) {
                        out.push(component.as_os_str());
                    }
                }
                Component::CurDir => {}
                other => out.push(other.as_os_str()),
            }
        }
        out
    })
}

fn is_sdk_member(library: &str, name: &str) -> bool {
    matches!(
        (library, name),
        ("dart:core", "print" | "pragma")
            | ("dart:async", "scheduleMicrotask")
            | ("dart:js_interop", "JS" | "staticInterop")
    )
}

fn is_sdk_type(library: &str, name: &str) -> bool {
    match library {
        "dart:core" => is_core_type(name),
        "dart:async" => matches!(name, "Future" | "FutureOr" | "Timer" | "Completer"),
        "dart:ui" => name == "Color",
        "dart:collection" => matches!(
            name,
            "Queue"
                | "ListQueue"
                | "DoubleLinkedQueue"
                | "HashMap"
                | "HashSet"
                | "LinkedHashMap"
                | "LinkedHashSet"
                | "SplayTreeMap"
                | "SplayTreeSet"
        ),
        "dart:js_interop" => matches!(
            name,
            "JSAny"
                | "JSObject"
                | "JSFunction"
                | "JSExportedDartFunction"
                | "JSBoxedDartObject"
                | "JSArray"
                | "JSPromise"
                | "JSNumber"
                | "JSBoolean"
                | "JSString"
                | "JSSymbol"
                | "JSBigInt"
                | "JSArrayBuffer"
                | "JSDataView"
                | "JSTypedArray"
                | "JSInt8Array"
                | "JSUint8Array"
                | "JSUint8ClampedArray"
                | "JSInt16Array"
                | "JSUint16Array"
                | "JSInt32Array"
                | "JSUint32Array"
                | "JSFloat32Array"
                | "JSFloat64Array"
                | "JSBigInt64Array"
                | "JSBigUint64Array"
        ),
        _ => false,
    }
}

fn is_known_package_member(package: &str, name: &str) -> bool {
    is_known_package_type(package, name)
        || matches!(
            (package, name),
            (
                "meta",
                "visibleForTesting" | "immutable" | "Target" | "TargetKind"
            ) | (
                "flutter",
                "runApp"
                    | "debugPrint"
                    | "kDebugMode"
                    | "kReleaseMode"
                    | "kProfileMode"
                    | "kIsWeb"
                    | "kIsWasm"
            ) | ("test", "TestOn" | "Timeout" | "Tags" | "OnPlatform")
        )
}

fn is_known_package_type(package: &str, name: &str) -> bool {
    match package {
        "fixnum" => matches!(name, "Int32" | "Int64"),
        "flutter" => matches!(
            name,
            "Widget"
                | "StatelessWidget"
                | "StatefulWidget"
                | "InheritedWidget"
                | "ProxyWidget"
                | "RenderObjectWidget"
                | "Key"
                | "BuildContext"
                | "State"
                | "TextEditingController"
                | "ScrollController"
                | "PageController"
                | "TabController"
                | "AnimationController"
                | "Color"
                | "Text"
                | "Container"
                | "Builder"
                | "ElevatedButton"
                | "GestureDetector"
                | "MaterialApp"
        ),
        _ => false,
    }
}

fn is_core_type(name: &str) -> bool {
    matches!(
        name,
        "Object"
            | "Enum"
            | "Function"
            | "Record"
            | "Type"
            | "Symbol"
            | "Null"
            | "Never"
            | "bool"
            | "num"
            | "int"
            | "double"
            | "String"
            | "Pattern"
            | "Comparable"
            | "RegExp"
            | "Iterable"
            | "Iterator"
            | "List"
            | "Set"
            | "Map"
            | "MapEntry"
            | "Duration"
            | "DateTime"
            | "Uri"
            | "BigInt"
            | "Stopwatch"
            | "StackTrace"
            | "Expando"
            | "WeakReference"
            | "Finalizer"
            | "dynamic"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    #[test]
    fn resolves_imports_prefixes_combinators_and_ambiguity() {
        let inputs = [
            (
                PathBuf::from("/project/lib/a.dart"),
                "class Foo {} typedef Alias = Foo; class Dup {} final value = 0;",
            ),
            (PathBuf::from("/project/lib/c.dart"), "class Dup {}"),
            (
                PathBuf::from("/project/lib/b.dart"),
                "import 'a.dart' as p show Foo, Alias; import 'a.dart' show Dup; import 'c.dart' show Dup;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        assert!(parsed.iter().all(|(_, errors)| errors.is_empty()));
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, _))| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let index = IdentityIndex::from_project_files(&sources, &[]);
        let file = Path::new("/project/lib/b.dart");

        assert_eq!(
            index.resolve(file, &["p".into(), "Foo".into()]),
            NameIdentity::Type
        );
        assert_eq!(
            index.resolve(file, &["p".into(), "Alias".into()]),
            NameIdentity::Type
        );
        assert_eq!(
            index.resolve(file, &["p".into(), "value".into()]),
            NameIdentity::Unknown
        );
        assert_eq!(index.resolve(file, &["Dup".into()]), NameIdentity::Unknown);
        assert_eq!(index.resolve(file, &["int".into()]), NameIdentity::Type);
    }

    #[test]
    fn reexports_preserve_original_declaration_identity() {
        let inputs = [
            (PathBuf::from("/project/lib/origin.dart"), "class Box<T> {}"),
            (
                PathBuf::from("/project/lib/barrel.dart"),
                "export 'origin.dart';",
            ),
            (
                PathBuf::from("/project/lib/chain.dart"),
                "export 'barrel.dart';",
            ),
            (
                PathBuf::from("/project/lib/main.dart"),
                "import 'chain.dart'; Box<int>? value;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, errors))| IdentitySource {
                path,
                program,
                has_parse_errors: !errors.is_empty(),
            })
            .collect();
        let index = IdentityIndex::from_project_files(&sources, &[]);
        let origin =
            index.resolve_declaration(Path::new("/project/lib/origin.dart"), &["Box".to_string()]);
        let imported =
            index.resolve_declaration(Path::new("/project/lib/main.dart"), &["Box".to_string()]);
        assert_eq!(imported, origin);
    }

    #[test]
    fn duplicate_paths_to_same_declaration_converge() {
        let inputs = [
            (PathBuf::from("/project/lib/origin.dart"), "class Box<T> {}"),
            (
                PathBuf::from("/project/lib/barrel.dart"),
                "export 'origin.dart';",
            ),
            (
                PathBuf::from("/project/lib/main.dart"),
                "import 'origin.dart'; import 'barrel.dart'; Box<int>? value;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        assert!(parsed.iter().all(|(_, errors)| errors.is_empty()));
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, _))| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let index = IdentityIndex::from_project_files(&sources, &[]);
        let origin =
            index.resolve_declaration(Path::new("/project/lib/origin.dart"), &["Box".to_string()]);
        let main = Path::new("/project/lib/main.dart");

        assert_eq!(
            index.resolve(main, &["Box".to_string()]),
            NameIdentity::Type
        );
        assert_eq!(
            index.resolve_declaration(main, &["Box".to_string()]),
            origin
        );
    }

    #[test]
    fn external_package_import_does_not_bind_project_library() {
        let inputs = [
            (PathBuf::from("/workspace/lib/foo.dart"), "class Foo {}"),
            (
                PathBuf::from("/workspace/lib/main.dart"),
                "import 'package:other/foo.dart'; Foo? value;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        assert!(parsed.iter().all(|(_, errors)| errors.is_empty()));
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, _))| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let index = IdentityIndex::from_project_files(&sources, &[]);

        let file = Path::new("/workspace/lib/main.dart");
        assert_eq!(index.resolve(file, &["Foo".into()]), NameIdentity::Unknown);
        assert_eq!(index.resolve_declaration(file, &["Foo".into()]), None);
    }

    #[test]
    fn current_package_import_binds_project_library_when_owner_known() {
        let inputs = [
            (PathBuf::from("/workspace/lib/foo.dart"), "class Foo {}"),
            (
                PathBuf::from("/workspace/lib/main.dart"),
                "import 'package:workspace/foo.dart'; Foo? value;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        assert!(parsed.iter().all(|(_, errors)| errors.is_empty()));
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, _))| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let package = PackageIdentity {
            name: "workspace".to_string(),
            lib_root: PathBuf::from("/workspace/lib"),
        };
        let index = IdentityIndex::from_project_files(&sources, &[package]);

        let file = Path::new("/workspace/lib/main.dart");
        assert_eq!(index.resolve(file, &["Foo".into()]), NameIdentity::Type);
        assert_eq!(
            index.resolve_declaration(file, &["Foo".into()]),
            index.resolve_declaration(Path::new("/workspace/lib/foo.dart"), &["Foo".into()])
        );
    }

    #[test]
    fn package_imports_resolve_unique_workspace_packages_across_owners() {
        let inputs = [
            (
                PathBuf::from("/workspace/lib/foo.dart"),
                "class OuterFoo {}",
            ),
            (
                PathBuf::from("/workspace/lib/main.dart"),
                "import 'package:outer/foo.dart'; OuterFoo? value;",
            ),
            (
                PathBuf::from("/workspace/tools/inner/lib/foo.dart"),
                "class InnerFoo {}",
            ),
            (
                PathBuf::from("/workspace/tools/inner/lib/main.dart"),
                "import 'package:inner/foo.dart'; import 'package:outer/foo.dart'; InnerFoo? own; OuterFoo? dependency;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, errors))| IdentitySource {
                path,
                program,
                has_parse_errors: !errors.is_empty(),
            })
            .collect();
        let packages = [
            PackageIdentity {
                name: "outer".to_string(),
                lib_root: PathBuf::from("/workspace/lib"),
            },
            PackageIdentity {
                name: "inner".to_string(),
                lib_root: PathBuf::from("/workspace/tools/inner/lib"),
            },
        ];
        let index = IdentityIndex::from_project_files(&sources, &packages);
        let inner = Path::new("/workspace/tools/inner/lib/main.dart");

        assert_eq!(
            index.resolve(inner, &["InnerFoo".into()]),
            NameIdentity::Type
        );
        assert_eq!(
            index.resolve(inner, &["OuterFoo".into()]),
            NameIdentity::Type
        );
        assert_eq!(
            index.resolve_declaration(inner, &["OuterFoo".into()]),
            index.resolve_declaration(Path::new("/workspace/lib/foo.dart"), &["OuterFoo".into()])
        );
        assert_eq!(
            index.resolve(Path::new("/workspace/lib/main.dart"), &["OuterFoo".into()]),
            NameIdentity::Type
        );
    }

    #[test]
    fn duplicate_workspace_package_names_are_ambiguous() {
        let inputs = [
            (
                PathBuf::from("/workspace/packages/first/lib/api.dart"),
                "class FirstApi {}",
            ),
            (
                PathBuf::from("/workspace/packages/second/lib/api.dart"),
                "class SecondApi {}",
            ),
            (
                PathBuf::from("/workspace/packages/consumer/lib/main.dart"),
                "import 'package:shared/api.dart'; FirstApi? first; SecondApi? second;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        assert!(parsed.iter().all(|(_, errors)| errors.is_empty()));
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, _))| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let packages = [
            PackageIdentity {
                name: "shared".to_string(),
                lib_root: PathBuf::from("/workspace/packages/first/lib"),
            },
            PackageIdentity {
                name: "shared".to_string(),
                lib_root: PathBuf::from("/workspace/packages/second/lib"),
            },
        ];
        let index = IdentityIndex::from_project_files(&sources, &packages);
        let importer = Path::new("/workspace/packages/consumer/lib/main.dart");

        assert_eq!(
            index.resolve(importer, &["FirstApi".into()]),
            NameIdentity::Unknown
        );
        assert_eq!(
            index.resolve(importer, &["SecondApi".into()]),
            NameIdentity::Unknown
        );
    }

    #[test]
    fn package_import_target_cannot_escape_exact_lib_root() {
        let inputs = [
            (
                PathBuf::from("/workspace/packages/b/private/api.dart"),
                "class PrivateApi {}",
            ),
            (
                PathBuf::from("/workspace/packages/a/lib/main.dart"),
                "import 'package:b/../private/api.dart'; PrivateApi? value;",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        assert!(parsed.iter().all(|(_, errors)| errors.is_empty()));
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, _))| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let package = PackageIdentity {
            name: "b".to_string(),
            lib_root: PathBuf::from("/workspace/packages/b/lib"),
        };
        let index = IdentityIndex::from_project_files(&sources, &[package]);

        assert_eq!(
            index.resolve(
                Path::new("/workspace/packages/a/lib/main.dart"),
                &["PrivateApi".into()]
            ),
            NameIdentity::Unknown
        );
    }

    #[test]
    fn shares_declarations_across_parts() {
        let inputs = [
            (
                PathBuf::from("/project/lib/root.dart"),
                "library example; part 'part.dart';",
            ),
            (
                PathBuf::from("/project/lib/part.dart"),
                "part of example; class PartType {}",
            ),
        ];
        let parsed: Vec<_> = inputs.iter().map(|(_, source)| parse(source)).collect();
        assert!(parsed.iter().all(|(_, errors)| errors.is_empty()));
        let sources: Vec<_> = inputs
            .iter()
            .zip(&parsed)
            .map(|((path, _), (program, _))| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let index = IdentityIndex::from_project_files(&sources, &[]);

        for file in ["/project/lib/root.dart", "/project/lib/part.dart"] {
            assert_eq!(
                index.resolve(Path::new(file), &["PartType".into()]),
                NameIdentity::Type
            );
        }
    }
}
