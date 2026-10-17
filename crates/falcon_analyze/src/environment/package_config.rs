//! `.dart_tool/package_config.json`: package name → root, library root and
//! language version.

use std::path::{Path, PathBuf};

use serde_json::Value;

use super::LanguageVersion;
use crate::resolve::{canonical_or_lexical, lexical_normalize};

/// One package entry of a package configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfiguredPackage {
    pub name: String,
    /// The package root (`rootUri`); a file belongs to the package whose root
    /// is its longest ancestor.
    pub root: PathBuf,
    /// The directory `package:<name>/` URIs resolve against (`packageUri`).
    pub lib_root: PathBuf,
    pub language_version: Option<LanguageVersion>,
}

/// A parsed `package_config.json` (version 2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageConfig {
    /// The `package_config.json` file itself.
    pub path: PathBuf,
    pub packages: Vec<ConfiguredPackage>,
    /// `flutterRoot`, written by `flutter pub get`.
    pub flutter_root: Option<PathBuf>,
}

impl PackageConfig {
    /// The nearest `.dart_tool/package_config.json` at or above `directory`.
    pub fn discover(directory: &Path) -> Option<PathBuf> {
        directory
            .ancestors()
            .map(|ancestor| ancestor.join(".dart_tool").join("package_config.json"))
            .find(|candidate| candidate.is_file())
    }

    /// Read and parse the configuration at `path`; `None` when unreadable or
    /// not a version-2 configuration. Package roots are canonicalized so they
    /// compare equal to the canonical paths of loaded files.
    pub fn load(path: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(path).ok()?;
        let mut config = Self::parse(path, &text)?;
        for package in &mut config.packages {
            package.root = canonical_or_lexical(&package.root);
            package.lib_root = canonical_or_lexical(&package.lib_root);
        }
        Some(config)
    }

    /// Parse configuration `text` read from `path`. Relative URIs resolve
    /// against the directory containing `path`.
    pub fn parse(path: &Path, text: &str) -> Option<Self> {
        let json: Value = serde_json::from_str(text).ok()?;
        if json.get("configVersion")?.as_u64()? != 2 {
            return None;
        }
        let base = absolute(path.parent()?);
        let packages = json
            .get("packages")?
            .as_array()?
            .iter()
            .filter_map(|package| {
                let name = package.get("name")?.as_str()?.to_string();
                let root = resolve_directory_uri(&base, package.get("rootUri")?.as_str()?)?;
                let lib_root = match package.get("packageUri").and_then(Value::as_str) {
                    Some(uri) => resolve_directory_uri(&root, uri)?,
                    None => root.clone(),
                };
                let language_version = package
                    .get("languageVersion")
                    .and_then(Value::as_str)
                    .and_then(LanguageVersion::parse);
                Some(ConfiguredPackage {
                    name,
                    root,
                    lib_root,
                    language_version,
                })
            })
            .collect();
        let flutter_root = json
            .get("flutterRoot")
            .and_then(Value::as_str)
            .and_then(|uri| resolve_directory_uri(&base, uri));
        Some(Self {
            path: path.to_path_buf(),
            packages,
            flutter_root,
        })
    }

    pub fn package(&self, name: &str) -> Option<&ConfiguredPackage> {
        self.packages.iter().find(|package| package.name == name)
    }

    /// The package whose root is the longest ancestor of `path`.
    pub fn package_containing(&self, path: &Path) -> Option<&ConfiguredPackage> {
        let path = canonical_or_lexical(&absolute(path));
        self.package_containing_canonical(&path)
    }

    pub(crate) fn package_containing_canonical(&self, path: &Path) -> Option<&ConfiguredPackage> {
        self.packages
            .iter()
            .filter(|package| path.starts_with(&package.root))
            .max_by_key(|package| package.root.components().count())
    }

    /// The package whose library root is the longest ancestor of `path`.
    pub fn package_library_containing(&self, path: &Path) -> Option<&ConfiguredPackage> {
        let path = canonical_or_lexical(&absolute(path));
        self.package_library_containing_canonical(&path)
    }

    pub(crate) fn package_library_containing_canonical(
        &self,
        path: &Path,
    ) -> Option<&ConfiguredPackage> {
        self.packages
            .iter()
            .filter(|package| path.starts_with(&package.lib_root))
            .max_by_key(|package| package.lib_root.components().count())
    }

    /// The file a `package:` URI names; `None` for unknown packages and paths
    /// escaping the package's library root.
    pub fn resolve_package_uri(&self, uri: &str) -> Option<PathBuf> {
        let (name, rest) = uri.strip_prefix("package:")?.split_once('/')?;
        let package = self.package(name)?;
        let target = lexical_normalize(&package.lib_root.join(percent_decode(rest)?));
        target.starts_with(&package.lib_root).then_some(target)
    }

    /// The language version of a file at `path` from its package entry; the
    /// analyzer applies [`LanguageVersion::CURRENT`] when there is none.
    pub fn language_version(&self, path: &Path) -> Option<LanguageVersion> {
        self.package_containing(path)?.language_version
    }

    pub(crate) fn language_version_canonical(&self, path: &Path) -> Option<LanguageVersion> {
        self.package_containing_canonical(path)?.language_version
    }
}

/// `path` made absolute (against the working directory) and lexically normalized.
pub(crate) fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        lexical_normalize(path)
    } else {
        std::env::current_dir()
            .map(|cwd| lexical_normalize(&cwd.join(path)))
            .unwrap_or_else(|_| lexical_normalize(path))
    }
}

/// Resolve a directory URI (`file:///abs/`, or a path relative to `base`).
fn resolve_directory_uri(base: &Path, uri: &str) -> Option<PathBuf> {
    let path = if let Some(rest) = uri.strip_prefix("file://") {
        // `file:///abs` (empty authority) or `file://localhost/abs`.
        let rest = rest.strip_prefix("localhost").unwrap_or(rest);
        PathBuf::from(percent_decode(rest)?)
    } else if uri.contains(':') && !uri.starts_with('.') && !uri.starts_with('/') {
        // Another scheme (or a Windows drive letter we cannot model).
        return None;
    } else {
        base.join(percent_decode(uri)?)
    };
    Some(lexical_normalize(&path))
}

/// Append a UTF-8 URI path, escaping bytes outside RFC 3986's path characters.
pub fn percent_encode(text: &str, output: &mut String) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'/' | b':'
                    | b'@'
                    | b'-'
                    | b'.'
                    | b'_'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
            )
        {
            output.push(char::from(byte));
        } else {
            output.push('%');
            output.push(char::from(HEX[usize::from(byte >> 4)]));
            output.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
}

/// Decode the `%XX` escapes of a URI component; `None` when malformed or
/// not UTF-8.
pub fn percent_decode(text: &str) -> Option<String> {
    if !text.contains('%') {
        return Some(text.to_string());
    }
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = std::str::from_utf8(bytes.get(index + 1..index + 3)?).ok()?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"{
      "configVersion": 2,
      "packages": [
        {"name": "dep", "rootUri": "file:///cache/dep%201.0", "packageUri": "lib/", "languageVersion": "3.4"},
        {"name": "app", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.7"},
        {"name": "nested", "rootUri": "../tool/nested/", "packageUri": "lib/"}
      ],
      "flutterRoot": "file:///opt/flutter"
    }"#;

    fn config() -> PackageConfig {
        PackageConfig::parse(
            Path::new("/work/app/.dart_tool/package_config.json"),
            CONFIG,
        )
        .unwrap()
    }

    #[test]
    fn resolves_roots_and_versions() {
        let config = config();
        let dep = config.package("dep").unwrap();
        assert_eq!(dep.root, PathBuf::from("/cache/dep 1.0"));
        assert_eq!(dep.lib_root, PathBuf::from("/cache/dep 1.0/lib"));
        assert_eq!(dep.language_version, Some(LanguageVersion::new(3, 4)));
        assert_eq!(
            config.package("app").unwrap().root,
            PathBuf::from("/work/app")
        );
        assert_eq!(config.flutter_root, Some(PathBuf::from("/opt/flutter")));
    }

    #[test]
    fn longest_root_owns_a_file() {
        let config = config();
        let owner = |path: &str| {
            config
                .package_containing(Path::new(path))
                .map(|package| package.name.as_str())
        };
        assert_eq!(owner("/work/app/test/a_test.dart"), Some("app"));
        assert_eq!(owner("/work/app/tool/nested/lib/a.dart"), Some("nested"));
        assert_eq!(owner("/elsewhere/a.dart"), None);
        assert_eq!(
            config.language_version(Path::new("/work/app/lib/a.dart")),
            Some(LanguageVersion::new(3, 7))
        );
        assert_eq!(
            config.language_version(Path::new("/work/app/tool/nested/lib/a.dart")),
            None
        );
    }

    #[test]
    fn resolves_package_uris_inside_lib_only() {
        let config = config();
        assert_eq!(
            config.resolve_package_uri("package:dep/src/a.dart"),
            Some(PathBuf::from("/cache/dep 1.0/lib/src/a.dart"))
        );
        assert_eq!(
            config.resolve_package_uri("package:dep/../pubspec.yaml"),
            None
        );
        assert_eq!(config.resolve_package_uri("package:missing/a.dart"), None);
    }

    #[test]
    fn rejects_other_versions() {
        assert!(
            PackageConfig::parse(
                Path::new("/p/.dart_tool/package_config.json"),
                r#"{"configVersion": 1, "packages": []}"#
            )
            .is_none()
        );
    }
}
