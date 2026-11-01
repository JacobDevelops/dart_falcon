//! Immutable, bounded Flutter localization inputs for one environment generation.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::AnalysisEnvironment;
use falcon_config::metadata_content as text;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct LocalizationConfig {
    arb_dir: PathBuf,
    template_arb_file: PathBuf,
    output_localization_file: PathBuf,
    output_dir: Option<PathBuf>,
}

impl Default for LocalizationConfig {
    fn default() -> Self {
        Self {
            arb_dir: "lib/l10n".into(),
            template_arb_file: "app_en.arb".into(),
            output_localization_file: "app_localizations.dart".into(),
            output_dir: None,
        }
    }
}

/// Parsed localization inputs; rules borrow these without acquiring disk facts.
#[derive(Debug, Clone)]
pub struct LocalizationTemplate {
    pub path: PathBuf,
    pub source: String,
    pub output_file: PathBuf,
    pub locales: HashSet<String>,
}

#[derive(Debug, Clone)]
pub(super) struct LocalizationSnapshot {
    settings: PathBuf,
    arb_dir: Option<PathBuf>,
    template_path: Option<PathBuf>,
    template: Option<LocalizationTemplate>,
    failures: Vec<(PathBuf, String)>,
}

impl LocalizationSnapshot {
    pub(super) fn load(root: &Path) -> Self {
        let mut snapshot = Self {
            settings: root.join("l10n.yaml"),
            arb_dir: None,
            template_path: None,
            template: None,
            failures: Vec::new(),
        };
        let settings = snapshot.settings.clone();
        let config = match snapshot.read(&settings) {
            Ok(Some(source)) => match serde_yaml::from_str::<LocalizationConfig>(text(&source)) {
                Ok(config) => config,
                Err(error) => {
                    snapshot.failures.push((
                        settings,
                        format!("malformed localization configuration: {error}"),
                    ));
                    return snapshot;
                }
            },
            Ok(None) => LocalizationConfig::default(),
            Err(()) => return snapshot,
        };
        let arb_dir = crate::lexical_normalize(&root.join(&config.arb_dir));
        snapshot.arb_dir = Some(arb_dir.clone());
        let path =
            crate::lexical_normalize(&root.join(&config.arb_dir).join(&config.template_arb_file));
        let normalized_template = path.clone();
        snapshot.template_path = Some(normalized_template.clone());
        let Ok(Some(source)) = snapshot.read(&path) else {
            return snapshot;
        };
        let Some(messages) = snapshot.messages(&path, &source) else {
            return snapshot;
        };
        let mut locales = HashSet::new();
        add_locale(&messages, &mut locales);
        match std::fs::read_dir(&arb_dir) {
            Ok(entries) => {
                for entry in entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(error) if missing(&error) => continue,
                        Err(error) => {
                            snapshot.failures.push((arb_dir.clone(), error.to_string()));
                            continue;
                        }
                    };
                    let sibling = entry.path();
                    if sibling
                        .extension()
                        .is_none_or(|extension| extension != "arb")
                    {
                        continue;
                    }
                    if let Some(stem) = sibling.file_stem().and_then(|stem| stem.to_str()) {
                        for (offset, _) in stem.match_indices('_') {
                            locales.insert(stem[offset + 1..].to_owned());
                        }
                    }
                    if sibling != normalized_template
                        && let Ok(Some(source)) = snapshot.read(&sibling)
                        && let Some(messages) = snapshot.messages(&sibling, &source)
                    {
                        add_locale(&messages, &mut locales);
                    }
                }
            }
            Err(error) if missing(&error) => return snapshot,
            Err(error) => snapshot.failures.push((arb_dir, error.to_string())),
        }
        if snapshot.failures.is_empty() {
            snapshot.template = Some(LocalizationTemplate {
                path,
                source,
                output_file: crate::lexical_normalize(
                    &root
                        .join(config.output_dir.as_ref().unwrap_or(&config.arb_dir))
                        .join(config.output_localization_file),
                ),
                locales,
            });
        }
        snapshot
    }

    fn read(&mut self, path: &Path) -> Result<Option<String>, ()> {
        match falcon_config::read_metadata_text(path) {
            Ok(source) => Ok(Some(source)),
            Err(error) if missing(&error) => Ok(None),
            Err(error) => {
                self.failures.push((path.to_path_buf(), error.to_string()));
                Err(())
            }
        }
    }

    fn messages(&mut self, path: &Path, source: &str) -> Option<serde_json::Value> {
        match serde_json::from_str::<serde_json::Value>(text(source)) {
            Ok(messages) if messages.is_object() => Some(messages),
            result => {
                let reason = match result {
                    Err(error) => error.to_string(),
                    Ok(_) => "expected an ARB object".to_owned(),
                };
                self.failures.push((
                    path.to_path_buf(),
                    format!("malformed localization messages: {reason}"),
                ));
                None
            }
        }
    }

    pub(super) fn affected_by(&self, path: &Path) -> bool {
        self.settings.starts_with(path)
            || self
                .template_path
                .as_ref()
                .is_some_and(|template| template.starts_with(path))
            || self.arb_dir.as_ref().is_some_and(|directory| {
                directory.starts_with(path)
                    || (path.parent() == Some(directory.as_path())
                        && path.extension().is_some_and(|extension| extension == "arb"))
            })
    }

    pub(super) fn watch_roots(&self) -> impl Iterator<Item = &Path> {
        self.arb_dir
            .as_deref()
            .into_iter()
            .chain(self.template_path.as_deref().and_then(Path::parent))
    }

    pub(super) fn settings(&self) -> &Path {
        &self.settings
    }

    pub(super) fn template_path(&self) -> Option<&Path> {
        self.template_path.as_deref()
    }

    pub(super) fn source(&self, path: &Path) -> Option<&str> {
        let template = self.template.as_ref()?;
        self.template_path
            .as_deref()
            .filter(|template| *template == path)
            .map(|_| template.source.as_str())
    }

    pub(super) fn failures(&self) -> impl Iterator<Item = &PathBuf> {
        self.failures.iter().map(|(path, _)| path)
    }

    pub(super) fn template(
        &self,
        environment: &AnalysisEnvironment,
    ) -> Option<&LocalizationTemplate> {
        for (path, reason) in &self.failures {
            environment.warn_optional_metadata(path, path, reason);
        }
        self.template.as_ref()
    }
}

fn missing(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
    )
}

fn add_locale(messages: &serde_json::Value, locales: &mut HashSet<String>) {
    if let Some(locale) = messages.get("@@locale").and_then(serde_json::Value::as_str) {
        locales.insert(locale.to_owned());
    }
}
