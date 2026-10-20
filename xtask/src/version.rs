use crate::workspace_root;
use regex::Regex;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// Docs, CLI data, and launcher sources: every file that pins the falcon version.
fn versioned_files(root: &Path) -> Vec<PathBuf> {
    let changelog = root.join("CHANGELOG.md");
    let mut files: Vec<PathBuf> = [root.to_path_buf(), root.join("docs")]
        .iter()
        .flat_map(|dir| fs::read_dir(dir).expect("read docs dir"))
        .map(|entry| entry.expect("read docs entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "md"))
        .filter(|path| *path != changelog)
        .collect();
    files.extend([
        root.join("website/src/data/cli.json"),
        root.join("packages/falcon/pubspec.yaml"),
        root.join("packages/falcon/lib/src/version.dart"),
        root.join("falcon.json"),
        root.join("schema/falcon.schema.json"),
        root.join("extensions/falcon-zed/Cargo.toml"),
        root.join("extensions/falcon-zed/extension.toml"),
        root.join("extensions/falcon-vscode/package.json"),
        root.join("extensions/falcon-vscode/package-lock.json"),
    ]);
    files
}

/// `text` with every pinned falcon version (release tag, archive name, mise pin,
/// cli.json `version`, launcher YAML version and Dart const) replaced by `version`.
fn with_version_references(text: &str, version: &str) -> String {
    let text = Regex::new(
        r#"(?m)(releases/download/v|falcon-|dart_falcon" = \{ version = "|"version": "|^version\s*=\s*"|^version: |^const version = ')\d+\.\d+\.\d+"#,
    )
    .unwrap()
    .replace_all(text, format!("${{1}}{version}"));
    Regex::new(r"(https://raw\.githubusercontent\.com/JacobDevelops/dart_falcon/)(?:main|v\d+\.\d+\.\d+)(/schema/falcon\.schema\.json)")
        .unwrap()
        .replace_all(&text, format!("${{1}}v{version}${{2}}"))
        .into_owned()
}

fn versioned_contents(path: &Path, text: &str, version: &str) -> String {
    if path
        .file_name()
        .is_some_and(|name| name == "package-lock.json")
    {
        Regex::new(
            r#"(?s)(\A\s*\{[^{}]*?"version"\s*:\s*"|"packages"\s*:\s*\{\s*""\s*:\s*\{[^{}]*?"version"\s*:\s*")[^"]+"#,
        )
        .unwrap()
        .replace_all(text, format!("${{1}}{version}"))
        .into_owned()
    } else {
        with_version_references(text, version)
    }
}

/// `cargo xtask version 0.4.2`: bump `[workspace.package]`, every pinned
/// reference in `versioned_files`, the workspace entries in Cargo.lock, and
/// mirror the release changelog into the pub.dev launcher.
pub(crate) fn run(version: &str) {
    if !Regex::new(r"^\d+\.\d+\.\d+$").unwrap().is_match(version) {
        eprintln!("error: expected a version like 0.4.2, got `{version}`");
        std::process::exit(1);
    }
    let root = workspace_root();
    let manifest_path = root.join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).expect("read Cargo.toml");
    let workspace_version =
        Regex::new(r#"(\[workspace\.package\]\nversion\s*=\s*")[^"]+"#).unwrap();
    if !workspace_version.is_match(&manifest) {
        eprintln!("error: Cargo.toml has no `version` directly under [workspace.package]");
        std::process::exit(1);
    }
    let manifest = workspace_version.replace(&manifest, format!("${{1}}{version}"));
    fs::write(&manifest_path, manifest.as_ref()).expect("write Cargo.toml");
    println!("updated {}", manifest_path.display());

    for path in versioned_files(&root) {
        let text = fs::read_to_string(&path).expect("read versioned file");
        let updated = versioned_contents(&path, &text, version);
        if updated != text {
            fs::write(&path, updated).expect("write versioned file");
            println!("updated {}", path.display());
        }
    }

    let launcher_changelog = root.join("packages/falcon/CHANGELOG.md");
    fs::copy(root.join("CHANGELOG.md"), &launcher_changelog).expect("copy launcher changelog");
    println!("updated {}", launcher_changelog.display());

    let status = Command::new("cargo")
        .args(["update", "--workspace", "--offline"])
        .current_dir(&root)
        .status()
        .expect("failed to spawn cargo update");
    if !status.success() {
        eprintln!("error: cargo update --workspace failed");
        std::process::exit(1);
    }
    let status = Command::new("cargo")
        .args([
            "update",
            "--manifest-path",
            "extensions/falcon-zed/Cargo.toml",
            "--workspace",
            "--offline",
        ])
        .current_dir(&root)
        .status()
        .expect("failed to spawn cargo update for Zed extension");
    if !status.success() {
        eprintln!("error: cargo update for Zed extension failed");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_urls_and_extension_toml_versions_are_rewritten() {
        let source = "version        = \"0.4.1\"\nhttps://raw.githubusercontent.com/JacobDevelops/dart_falcon/main/schema/falcon.schema.json\nhttps://raw.githubusercontent.com/JacobDevelops/dart_falcon/v0.4.1/schema/falcon.schema.json";
        let rewritten = with_version_references(source, "9.8.7");
        assert!(rewritten.starts_with("version        = \"9.8.7\""));
        assert_eq!(rewritten.matches("/v9.8.7/schema/").count(), 2);
    }

    #[test]
    fn npm_lock_version_update_preserves_dependency_versions() {
        let source = r#"{"version":"0.4.1","packages":{"":{"version":"0.4.1"},"node_modules/example":{"version":"1.2.3"}}}"#;
        let rewritten = versioned_contents(Path::new("package-lock.json"), source, "9.8.7");
        let lock: serde_json::Value = serde_json::from_str(&rewritten).unwrap();
        assert_eq!(lock["version"], "9.8.7");
        assert_eq!(lock["packages"][""]["version"], "9.8.7");
        assert_eq!(lock["packages"]["node_modules/example"]["version"], "1.2.3");
    }

    #[test]
    fn npm_lock_version_update_preserves_formatting_and_key_order() {
        let source = "{\n  \"name\": \"falcon\",\n  \"version\": \"0.4.3\",\n  \"lockfileVersion\": 3,\n  \"packages\": {\n    \"\": {\n      \"name\": \"falcon\",\n      \"version\": \"0.4.3\"\n    },\n    \"node_modules/example\": {\"version\": \"1.2.3\"}\n  }\n}\n";
        let expected = source.replace("0.4.3", "9.8.7");
        assert_eq!(
            versioned_contents(Path::new("package-lock.json"), source, "9.8.7"),
            expected
        );
    }

    #[test]
    fn pinned_version_references_match_the_workspace_version() {
        let version = env!("CARGO_PKG_VERSION");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for path in versioned_files(root) {
            let text = std::fs::read_to_string(&path).unwrap();
            assert!(
                text == versioned_contents(&path, &text, version),
                "{} pins a version other than {version}; run `cargo xtask version {version}`",
                path.display()
            );
        }
    }

    #[test]
    fn versioned_files_excludes_historical_changelog_pins() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::create_dir(root.join("docs")).unwrap();
        let changelog = root.join("CHANGELOG.md");
        std::fs::write(&changelog, "Download falcon-0.1.0-x86_64-linux.tar.gz").unwrap();
        let readme = root.join("README.md");
        std::fs::write(&readme, "Download falcon-0.4.2-x86_64-linux.tar.gz").unwrap();

        let files = versioned_files(root);
        assert!(!files.contains(&changelog));
        assert!(files.contains(&readme));
    }

    #[test]
    fn launcher_version_references_rewrite_actual_files() {
        let version = env!("CARGO_PKG_VERSION");
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let files = versioned_files(root);
        for (relative, prefix, suffix) in [
            ("packages/falcon/pubspec.yaml", "version: ", ""),
            (
                "packages/falcon/lib/src/version.dart",
                "const version = '",
                "';",
            ),
        ] {
            let path = root.join(relative);
            assert!(files.contains(&path), "{relative} must be versioned");
            let text = std::fs::read_to_string(path).unwrap();
            let current = format!("{prefix}{version}{suffix}");
            assert!(text.lines().any(|line| line == current));
            let bumped = with_version_references(&text, "99.0.0");
            assert_eq!(
                bumped,
                text.replace(&current, &format!("{prefix}99.0.0{suffix}"))
            );
            assert_ne!(bumped, text);
        }
    }

    #[test]
    fn version_references_rewrite_pins_and_nothing_else() {
        let text = r#"releases/download/v0.4.1/falcon-0.4.1-x86_64-linux.tar.gz
"github:JacobDevelops/dart_falcon" = { version = "0.4.1", matching = "falcon-" }
"version": "0.4.1"
version: 0.4.1
const version = '0.4.1';
  sdk: ^3.13.0
releases/latest/download/falcon-x86_64-linux.tar.gz regex 0.4.17"#;
        assert_eq!(
            with_version_references(text, "0.5.0"),
            r#"releases/download/v0.5.0/falcon-0.5.0-x86_64-linux.tar.gz
"github:JacobDevelops/dart_falcon" = { version = "0.5.0", matching = "falcon-" }
"version": "0.5.0"
version: 0.5.0
const version = '0.5.0';
  sdk: ^3.13.0
releases/latest/download/falcon-x86_64-linux.tar.gz regex 0.4.17"#
        );
    }
}
