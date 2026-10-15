//! Dart SDK discovery and `dart:` library mapping.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::PackageConfig;
use crate::resolve::canonical_or_lexical;

/// `libraries.json` targets consulted in order. The analyzer resolves `dart:`
/// URIs as the VM does, and additionally knows the web-only libraries
/// (`dart:html`, `dart:js_util`, …), which only the web targets declare.
const TARGETS: &[&str] = &["vm", "dart2js", "dartdevc"];

/// A Dart SDK: its root and the defining file of every `dart:` library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DartSdk {
    pub root: PathBuf,
    libraries: HashMap<String, PathBuf>,
    uris: HashMap<PathBuf, String>,
}

impl DartSdk {
    /// Locate the SDK the analyzer would use for a project with `config`:
    /// `<flutterRoot>/bin/cache/dart-sdk`, then `$DART_SDK`, then
    /// `$FLUTTER_ROOT/bin/cache/dart-sdk`, then the SDK containing a `dart`
    /// executable on `PATH` (shims that are not inside an SDK are skipped).
    pub fn locate(config: Option<&PackageConfig>) -> Option<Self> {
        let flutter_sdk = |root: &Path| root.join("bin").join("cache").join("dart-sdk");
        config
            .and_then(|config| config.flutter_root.as_deref())
            .map(flutter_sdk)
            .into_iter()
            .chain(std::env::var_os("DART_SDK").map(PathBuf::from))
            .chain(std::env::var_os("FLUTTER_ROOT").map(|root| flutter_sdk(Path::new(&root))))
            .chain(path_sdk_candidates())
            .find_map(|root| Self::load(&root))
    }

    /// Load the SDK at `root`; `None` unless `root/lib/libraries.json` maps
    /// `dart:core`.
    pub fn load(root: &Path) -> Option<Self> {
        let root = std::fs::canonicalize(root).ok()?;
        let lib = root.join("lib");
        let text = std::fs::read_to_string(lib.join("libraries.json")).ok()?;
        let json: Value = serde_json::from_str(&text).ok()?;
        let mut libraries = HashMap::new();
        for target in TARGETS {
            collect_target(&json, target, &lib, &mut libraries, 0);
        }
        if !libraries.contains_key("dart:core") {
            return None;
        }
        let uris = libraries
            .iter()
            .map(|(uri, path)| (path.clone(), uri.clone()))
            .collect();
        Some(Self {
            root,
            libraries,
            uris,
        })
    }

    /// The defining file of `dart:<name>`.
    pub fn library(&self, uri: &str) -> Option<&Path> {
        self.libraries.get(uri).map(PathBuf::as_path)
    }

    /// The `dart:` URI whose defining file is `path`.
    pub fn library_uri(&self, path: &Path) -> Option<&str> {
        self.uris.get(path).map(String::as_str)
    }

    /// Replace the library mappings with an embedder's (`_embedder.yaml`
    /// `embedded_libs`, as `sky_engine` ships for Flutter). The analyzer then
    /// sees only the embedded `dart:` libraries, each at the embedder's copy:
    /// in a Flutter package `dart:mirrors` and `dart:cli` do not exist.
    pub fn apply_embedder(&mut self, embedder: &Embedder) {
        self.libraries = embedder.libraries.iter().cloned().collect();
        self.uris = embedder
            .libraries
            .iter()
            .map(|(uri, path)| (path.clone(), uri.clone()))
            .collect();
    }
}

/// `dart:` libraries an embedder package (`lib/_embedder.yaml`) supplies.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Embedder {
    libraries: Vec<(String, PathBuf)>,
}

impl Embedder {
    /// The embedder declared by the packages of `config`, if any.
    pub fn from_config(config: &PackageConfig) -> Option<Self> {
        config.packages.iter().find_map(|package| {
            let file = package.lib_root.join("_embedder.yaml");
            let text = std::fs::read_to_string(&file).ok()?;
            Self::parse(&package.lib_root, &text)
        })
    }

    /// Parse `_embedder.yaml` text whose paths are relative to `lib_root`.
    pub fn parse(lib_root: &Path, text: &str) -> Option<Self> {
        let yaml: serde_yaml::Value = serde_yaml::from_str(text).ok()?;
        let libraries: Vec<_> = yaml
            .get("embedded_libs")?
            .as_mapping()?
            .iter()
            .filter_map(|(uri, path)| {
                let uri = uri.as_str()?.strip_prefix("dart:")?;
                let path = canonical_or_lexical(&lib_root.join(path.as_str()?));
                Some((format!("dart:{uri}"), path))
            })
            .collect();
        (!libraries.is_empty()).then_some(Self { libraries })
    }
}

fn collect_target(
    json: &Value,
    target: &str,
    lib: &Path,
    libraries: &mut HashMap<String, PathBuf>,
    depth: usize,
) {
    let Some(entry) = json.get(target) else {
        return;
    };
    if depth > 8 {
        return;
    }
    if let Some(declared) = entry.get("libraries").and_then(Value::as_object) {
        for (name, library) in declared {
            if let Some(uri) = library.get("uri").and_then(Value::as_str) {
                libraries
                    .entry(format!("dart:{name}"))
                    .or_insert_with(|| canonical_or_lexical(&lib.join(uri)));
            }
        }
    }
    for include in entry
        .get("include")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        if let Some(included) = include.get("target").and_then(Value::as_str) {
            collect_target(json, included, lib, libraries, depth + 1);
        }
    }
}

/// Raw locator inputs retained until a configuration event, including absent
/// SDK roots and uncanonicalized PATH executables whose symlinks may retarget.
pub(super) fn discovery_inputs() -> Vec<PathBuf> {
    let mut inputs: Vec<_> = std::env::var_os("DART_SDK")
        .map(PathBuf::from)
        .into_iter()
        .chain(
            std::env::var_os("FLUTTER_ROOT")
                .map(|root| PathBuf::from(root).join("bin/cache/dart-sdk")),
        )
        .collect();
    if let Some(path) = std::env::var_os("PATH") {
        inputs.extend(std::env::split_paths(&path).map(|directory| directory.join("dart")));
    }
    inputs.extend(path_sdk_candidates());
    inputs
}

/// SDK roots implied by `dart` executables on `PATH`: `<sdk>/bin/dart`, or a
/// Flutter checkout's `bin/dart` wrapper next to `bin/cache/dart-sdk`.
fn path_sdk_candidates() -> Vec<PathBuf> {
    let Some(path) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    std::env::split_paths(&path)
        .map(|directory| directory.join("dart"))
        .filter(|executable| executable.is_file())
        .filter_map(|executable| std::fs::canonicalize(executable).ok())
        .filter_map(|executable| {
            let bin = executable.parent()?;
            let flutter = bin.join("cache").join("dart-sdk");
            Some(if flutter.is_dir() {
                flutter
            } else {
                bin.parent()?.to_path_buf()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_vm_libraries_then_web_only_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(
            lib.join("libraries.json"),
            r#"{
              "vm_common": {"libraries": {"core": {"uri": "core/core.dart"}, "io": {"uri": "io/io.dart"}}},
              "vm": {"include": [{"target": "vm_common"}], "libraries": {"cli": {"uri": "cli/cli.dart"}}},
              "dart2js": {"libraries": {"html": {"uri": "html/dart2js/html_dart2js.dart"}, "core": {"uri": "web/core.dart"}}}
            }"#,
        )
        .unwrap();
        let sdk = DartSdk::load(dir.path()).unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        assert_eq!(
            sdk.library("dart:core"),
            Some(root.join("lib/core/core.dart").as_path())
        );
        assert_eq!(
            sdk.library("dart:html"),
            Some(root.join("lib/html/dart2js/html_dart2js.dart").as_path())
        );
        assert_eq!(sdk.library("dart:ui"), None);
        for uri in ["dart:core", "dart:html", "dart:io", "dart:cli"] {
            assert_eq!(sdk.library_uri(sdk.library(uri).unwrap()), Some(uri));
        }
    }

    #[test]
    fn rejects_directories_without_core() {
        let dir = tempfile::tempdir().unwrap();
        assert!(DartSdk::load(dir.path()).is_none());
    }

    #[test]
    fn embedder_overrides_libraries() {
        let embedder = Embedder::parse(
            Path::new("/sky/lib"),
            "embedded_libs:\n  \"dart:core\": \"core/core.dart\"\n  \"dart:ui\": \"ui/ui.dart\"\n",
        )
        .unwrap();
        let mut sdk = DartSdk {
            uris: HashMap::new(),
            root: PathBuf::from("/sdk"),
            libraries: HashMap::from([
                (
                    "dart:core".to_string(),
                    PathBuf::from("/sdk/lib/core/core.dart"),
                ),
                (
                    "dart:mirrors".to_string(),
                    PathBuf::from("/sdk/lib/mirrors/mirrors.dart"),
                ),
            ]),
        };
        sdk.apply_embedder(&embedder);
        assert_eq!(
            sdk.library("dart:core"),
            Some(Path::new("/sky/lib/core/core.dart"))
        );
        assert_eq!(
            sdk.library("dart:ui"),
            Some(Path::new("/sky/lib/ui/ui.dart"))
        );
        assert_eq!(sdk.library("dart:mirrors"), None);
        assert_eq!(
            sdk.library_uri(Path::new("/sky/lib/ui/ui.dart")),
            Some("dart:ui")
        );
        assert_eq!(
            sdk.library_uri(Path::new("/sdk/lib/mirrors/mirrors.dart")),
            None
        );
    }
}
