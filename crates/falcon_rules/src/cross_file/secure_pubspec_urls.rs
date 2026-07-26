//! Disallow insecure URL schemes in `pubspec.yaml` string values.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, ProjectFile};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use serde_yaml::Value;

use super::{enclosing_pubspec, yaml_string_span};

pub struct SecurePubspecUrls;

const NAME: &str = "secure-pubspec-urls";

impl CrossFileRule for SecurePubspecUrls {
    fn name(&self) -> &'static str {
        NAME
    }

    fn analyze_project(&self, files: &[ProjectFile], _config: &FalconConfig) -> Vec<Diagnostic> {
        project_pubspecs(files)
            .into_iter()
            .flat_map(|path| check_pubspec(&path))
            .collect()
    }
}

fn check_pubspec(path: &Path) -> Vec<Diagnostic> {
    let Ok(source) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(root) = serde_yaml::from_str::<Value>(&source) else {
        return Vec::new();
    };

    url_values(&root)
        .into_iter()
        .filter_map(|value| {
            let protocol = insecure_protocol(value.as_str()?)?;
            let span = yaml_string_span(&source, &root, value)?;
            let mut diagnostic = Diagnostic::new(
                NAME,
                Severity::Warning,
                format!("The '{protocol}' protocol shouldn't be used because it isn't secure."),
                path.to_string_lossy().into_owned(),
                span,
            );
            diagnostic.resolve_position(&source);
            Some(diagnostic)
        })
        .collect()
}

fn project_pubspecs(files: &[ProjectFile]) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut paths = Vec::new();
    for file in files {
        if let Some(path) = enclosing_pubspec(&file.path)
            && seen.insert(path.clone())
        {
            paths.push(path);
        }
    }
    paths
}

fn insecure_protocol(value: &str) -> Option<&'static str> {
    let (scheme, _) = value.split_once(':')?;
    if scheme.eq_ignore_ascii_case("http") {
        Some("http")
    } else if scheme.eq_ignore_ascii_case("git") {
        Some("git")
    } else {
        None
    }
}

fn url_values(root: &Value) -> Vec<&Value> {
    let Some(pubspec) = root.as_mapping() else {
        return Vec::new();
    };
    let mut values = Vec::new();

    for key in ["documentation", "homepage", "issue_tracker", "repository"] {
        if let Some(value) = pubspec.get(Value::String(key.to_string()))
            && value.is_string()
        {
            values.push(value);
        }
    }
    for key in ["dependencies", "dependency_overrides", "dev_dependencies"] {
        let Some(dependencies) = pubspec
            .get(Value::String(key.to_string()))
            .and_then(Value::as_mapping)
        else {
            continue;
        };
        for dependency in dependencies.values() {
            let Some(dependency) = dependency.as_mapping() else {
                continue;
            };
            for source in ["git", "hosted"] {
                let Some(source) = dependency.get(Value::String(source.to_string())) else {
                    continue;
                };
                if source.is_string() {
                    values.push(source);
                } else if let Some(url) = source
                    .as_mapping()
                    .and_then(|source| source.get(Value::String("url".to_string())))
                    && url.is_string()
                {
                    values.push(url);
                }
            }
        }
    }

    values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_only_insecure_url_schemes() {
        assert_eq!(insecure_protocol("http://example.com"), Some("http"));
        assert_eq!(insecure_protocol("GIT:example.com/repo"), Some("git"));
        assert_eq!(insecure_protocol("https://example.com"), None);
        assert_eq!(insecure_protocol("git@github.com:owner/repo.git"), None);
    }

    #[test]
    fn selects_only_supported_url_fields() {
        let root: Value = serde_yaml::from_str(
            "homepage: http://example.com\nfunding: [git://fund.example]\ndescription: http://ignored.example\ndependencies: {pkg: {git: {url: git://repo.example}}, direct: http://direct.example, ignored: {host: http://host.example}}\n",
        )
        .unwrap();
        let values: Vec<&str> = url_values(&root)
            .into_iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(values, ["http://example.com", "git://repo.example"]);
    }

    #[test]
    fn fixture_reports_every_insecure_url() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/secure-pubspec-urls/cross-file/bad-nested/pubspec.yaml");
        assert_eq!(check_pubspec(&path).len(), 6);
    }
}
