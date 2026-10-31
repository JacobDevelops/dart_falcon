//! Report template ARB message keys with no identifier reference in analyzed Dart sources.
//!
//! Reads Flutter `gen-l10n` paths from `l10n.yaml`, or defaults to
//! `lib/l10n/app_en.arb` when no configuration exists. Diagnostics point to the
//! template key; metadata keys starting with `@` are ignored. The configured
//! output localization file and its locale siblings in the output directory do
//! not establish usage. Locale suffixes must occur in an ARB filename or its
//! `@@locale` metadata; unrecognized siblings conservatively establish usage.
//! Any matching identifier-like Dart token outside those files counts, including
//! declarations, contextual keywords (even in keyword positions), getters, calls,
//! tear-offs and interpolation.
//! Receiver types and the configured output class are deliberately not required,
//! since extension accessors can hide them. References in other analyzed packages
//! count too, supporting shared localization packages and example applications.
//! Malformed configuration and missing or malformed templates are skipped.

use std::borrow::Cow;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use falcon_analyze::{CrossFileRule, ProjectFile, SemanticIndexes};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span};

use super::{canonical_package_path, enclosing_pubspec};

#[derive(Default)]
pub struct UnusedL10n;

#[derive(serde::Deserialize)]
#[serde(default, rename_all = "kebab-case")]
struct LocalizationConfig {
    arb_dir: PathBuf,
    template_arb_file: PathBuf,
    output_localization_file: PathBuf,
    output_dir: Option<PathBuf>,
}

struct LocalizationTemplate {
    path: PathBuf,
    source: String,
    output_file: PathBuf,
    locales: HashSet<String>,
}

impl Default for LocalizationConfig {
    fn default() -> Self {
        Self {
            arb_dir: PathBuf::from("lib/l10n"),
            template_arb_file: PathBuf::from("app_en.arb"),
            output_localization_file: PathBuf::from("app_localizations.dart"),
            output_dir: None,
        }
    }
}

impl CrossFileRule for UnusedL10n {
    fn prepare_project(
        &self,
        _environment: &falcon_analyze::AnalysisEnvironment,
        files: &[ProjectFile],
        _config: &FalconConfig,
        _indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        let packages: BTreeSet<_> = files
            .iter()
            .filter_map(|file| enclosing_pubspec(&file.path))
            .collect();
        let mut templates = Vec::with_capacity(packages.len());
        for pubspec in packages {
            let Some(root) = pubspec.parent() else {
                continue;
            };
            let settings_path = root.join("l10n.yaml");
            let settings = if settings_path.exists() {
                let Some(settings) = falcon_config::read_metadata_text(&settings_path)
                    .ok()
                    .and_then(|source| serde_yaml::from_str::<LocalizationConfig>(&source).ok())
                else {
                    continue;
                };
                settings
            } else {
                LocalizationConfig::default()
            };
            let template = root
                .join(&settings.arb_dir)
                .join(&settings.template_arb_file);
            let Ok(source) = falcon_config::read_metadata_text(&template) else {
                continue;
            };
            let Some(messages) = serde_json::from_str::<serde_json::Value>(&source)
                .ok()
                .filter(serde_json::Value::is_object)
            else {
                continue;
            };
            let locales = localization_locales(&root.join(&settings.arb_dir), &template, &messages);
            let output_dir = canonical_package_path(
                &root.join(settings.output_dir.as_ref().unwrap_or(&settings.arb_dir)),
            );
            let output_file = output_dir.join(&settings.output_localization_file);
            templates.push(LocalizationTemplate {
                path: template,
                source,
                output_file,
                locales,
            });
        }
        if templates.is_empty() {
            return Vec::new();
        }
        let mut used = HashSet::new();
        for file in files {
            if file
                .path
                .extension()
                .is_none_or(|extension| extension != "dart")
            {
                continue;
            }
            let path = canonical_package_path(&file.path);
            if templates
                .iter()
                .any(|template| is_generated(&path, &template.output_file, &template.locales))
            {
                continue;
            }
            used.extend(
                file.tree()
                    .tokens(file.source())
                    .filter(|token| token.kind().is_identifier_like())
                    .map(falcon_syntax::cst::SyntaxToken::text),
            );
        }
        let mut diagnostics = Vec::new();
        for template in templates {
            for (key, span) in template_keys(&template.source) {
                if key.starts_with('@') || used.contains(key.as_ref()) {
                    continue;
                }
                let mut diagnostic = Diagnostic::new(
                    "",
                    Severity::Warning,
                    format!("Localization key `{key}` is never used."),
                    template.path.to_string_lossy().into_owned(),
                    span,
                );
                diagnostic.resolve_position(&template.source);
                diagnostics.push(diagnostic);
            }
        }
        diagnostics
    }
}

fn is_generated(path: &Path, output: &Path, locales: &HashSet<String>) -> bool {
    if path == output {
        return true;
    }
    if path.parent() != output.parent() {
        return false;
    }
    let Some(stem) = output.file_stem().and_then(|stem| stem.to_str()) else {
        return false;
    };
    path.file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix(stem))
        .and_then(|suffix| suffix.strip_prefix('_'))
        .is_some_and(|locale| locales.contains(locale))
}

fn localization_locales(
    arb_dir: &Path,
    template: &Path,
    messages: &serde_json::Value,
) -> HashSet<String> {
    let mut locales = HashSet::new();
    if let Some(locale) = messages.get("@@locale").and_then(serde_json::Value::as_str) {
        locales.insert(locale.to_owned());
    }
    let Ok(entries) = std::fs::read_dir(arb_dir) else {
        return locales;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|extension| extension != "arb") {
            continue;
        }
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            for (offset, _) in stem.match_indices('_') {
                locales.insert(stem[offset + 1..].to_owned());
            }
        }
        if path != template
            && let Some(locale) = falcon_config::read_metadata_text(&path)
                .ok()
                .and_then(|source| serde_json::from_str::<serde_json::Value>(&source).ok())
                .and_then(|messages| messages.get("@@locale")?.as_str().map(ToOwned::to_owned))
        {
            locales.insert(locale);
        }
    }
    locales
}

fn template_keys(source: &str) -> impl Iterator<Item = (Cow<'_, str>, Span)> {
    let bytes = source.as_bytes();
    let mut depth = 0;
    let mut offset = 0;
    std::iter::from_fn(move || {
        while offset < bytes.len() {
            match bytes[offset] {
                b'{' | b'[' => depth += 1,
                b'}' | b']' => depth -= 1,
                b'"' => {
                    let start = offset;
                    offset += 1;
                    while offset < bytes.len() {
                        match bytes[offset] {
                            b'\\' => offset += 2,
                            b'"' => break,
                            _ => offset += 1,
                        }
                    }
                    let end = offset + 1;
                    if depth == 1
                        && bytes[end..].iter().find(|byte| !byte.is_ascii_whitespace())
                            == Some(&b':')
                    {
                        offset = end;
                        let spelling = &source[start + 1..end - 1];
                        let key = if spelling.contains('\\') {
                            Cow::Owned(serde_json::from_str::<String>(&source[start..end]).ok()?)
                        } else {
                            Cow::Borrowed(spelling)
                        };
                        return Some((key, Span { start, end }));
                    }
                }
                _ => {}
            }
            offset += 1;
        }
        None
    })
}
