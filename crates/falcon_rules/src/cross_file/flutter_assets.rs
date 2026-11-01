use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use falcon_analyze::{AnalysisEnvironment, ManifestSnapshot, ProjectFile};
use falcon_diagnostics::Span;
use falcon_syntax::cst::{ArgList, Expr, NamedArg, NodeKind, StringLitNode};
use marked_yaml::{LoaderOptions, Node, parse_yaml_with_options, types::MarkedScalarNode};

use super::{scalar_source_span, yaml_character_to_byte, yaml_parser_source};
use crate::lint::string_lit::{LiteralPart, decode_escapes, literal_parts, multiline_lead_len};
use crate::lint::support::directive_strings::uri_value;

pub(super) struct AssetEntry {
    pub path: String,
    pub span: Option<Span>,
}

impl AssetEntry {
    pub fn path_covers(declaration: &str, path: &str) -> bool {
        if declaration.ends_with('/') {
            path.strip_prefix(declaration)
                .is_some_and(|child| !child.is_empty() && !child.contains('/'))
        } else {
            declaration == path
        }
    }
}

pub(super) struct AssetPackage<'a> {
    pub pubspec: PathBuf,
    pub source: &'a str,
    pub name: Option<String>,
    pub entries: Vec<AssetEntry>,
    pub declarations: Vec<String>,
    pub files: Vec<&'a ProjectFile>,
    pub generated: bool,
}

pub(super) fn asset_packages<'a>(
    environment: &'a AnalysisEnvironment,
    files: &'a [ProjectFile],
    include_spans: bool,
) -> Vec<AssetPackage<'a>> {
    let mut packages = BTreeMap::new();
    for file in files {
        let Some(metadata) = environment.package(file.path()) else {
            continue;
        };
        let package = packages
            .entry(&metadata.manifest)
            .or_insert_with(|| (metadata, Vec::new()));
        if file
            .path()
            .extension()
            .is_some_and(|extension| extension == "dart")
        {
            package.1.push(file);
        }
    }
    packages
        .into_iter()
        .filter_map(|(pubspec, (metadata, files))| {
            let ManifestSnapshot::Valid { source, yaml } = metadata.snapshot() else {
                return None;
            };
            let pubspec = pubspec.clone();
            let source = source.as_str();
            // marked-yaml 0.8 rejects anchors and has no option to enable them.
            let Ok(root) =
                parse_yaml_with_options(0, yaml_parser_source(source), LoaderOptions::default())
            else {
                return if include_spans {
                    None
                } else {
                    fallback_asset_package(pubspec, source, yaml, files)
                };
            };
            let root = root.as_mapping()?;
            let flutter = root.get_node("flutter")?.as_mapping();
            let name = root.get_scalar("name").map(|name| name.as_str().to_owned());
            let generated = root.get_node("flutter_gen").is_some()
                || ["dependencies", "dev_dependencies"]
                    .into_iter()
                    .any(|section| {
                        root.get_mapping(section).is_some_and(|dependencies| {
                            dependencies.get_node("flutter_gen_runner").is_some()
                        })
                    });
            let assets = flutter.and_then(|flutter| flutter.get_sequence("assets"));
            let capacity = assets.map_or(0, |assets| assets.len());
            let mut entries = Vec::with_capacity(if include_spans { capacity } else { 0 });
            let declarations = if include_spans {
                Vec::new()
            } else {
                asset_declarations(yaml).map(str::to_owned).collect()
            };
            for scalar in assets
                .into_iter()
                .flat_map(|assets| assets.iter())
                .filter_map(asset_scalar)
                .filter(|_| include_spans)
            {
                let path = scalar.as_str().to_owned();
                let span = scalar
                    .span()
                    .start()
                    .and_then(|marker| yaml_character_to_byte(source, marker.character()))
                    .and_then(|start| scalar_source_span(source, start));
                entries.push(AssetEntry { path, span });
            }
            Some(AssetPackage {
                pubspec,
                source,
                name,
                entries,
                declarations,
                files,
                generated,
            })
        })
        .collect()
}

fn asset_scalar(node: &Node) -> Option<&MarkedScalarNode> {
    node.as_scalar()
        .or_else(|| node.as_mapping()?.get_scalar("path"))
}

/// Asset declarations shared by rule evaluation and filesystem-event admission.
pub fn asset_declarations(root: &serde_yaml::Value) -> impl Iterator<Item = &str> {
    fn sequence(value: &serde_yaml::Value) -> &[serde_yaml::Value] {
        value.as_sequence().map_or(&[], Vec::as_slice)
    }
    fn asset_path(asset: &serde_yaml::Value) -> Option<&str> {
        asset.get("path").unwrap_or(asset).as_str()
    }
    let flutter = &root["flutter"];
    sequence(&flutter["assets"])
        .iter()
        .filter_map(asset_path)
        .chain(sequence(&flutter["fonts"]).iter().flat_map(|family| {
            sequence(&family["fonts"])
                .iter()
                .filter_map(|font| font["asset"].as_str())
        }))
        .chain(
            sequence(&flutter["shaders"])
                .iter()
                .filter_map(serde_yaml::Value::as_str),
        )
        .chain(
            sequence(&flutter["deferred-components"])
                .iter()
                .flat_map(|component| sequence(&component["assets"]).iter().filter_map(asset_path)),
        )
}

fn fallback_asset_package<'a>(
    pubspec: PathBuf,
    source: &'a str,
    root: &serde_yaml::Value,
    files: Vec<&'a ProjectFile>,
) -> Option<AssetPackage<'a>> {
    use serde_yaml::Value;
    root.get("flutter")?;
    let name = root.get("name").and_then(Value::as_str).map(str::to_owned);
    let generated = root.get("flutter_gen").is_some()
        || ["dependencies", "dev_dependencies"]
            .into_iter()
            .any(|section| {
                root.get(section)
                    .and_then(|dependencies| dependencies.get("flutter_gen_runner"))
                    .is_some()
            });
    let declarations = asset_declarations(root).map(str::to_owned).collect();
    Some(AssetPackage {
        pubspec,
        source,
        name,
        entries: Vec::new(),
        declarations,
        files,
        generated,
    })
}

pub(super) fn asset_strings(file: &ProjectFile) -> impl Iterator<Item = StringLitNode<'_>> {
    file.tree()
        .root(file.source())
        .descendants()
        .filter_map(StringLitNode::cast)
        .filter(|literal| {
            !literal
                .syntax()
                .parent()
                .is_some_and(|parent| parent.kind() == NodeKind::AdjacentStrings)
                && !literal.syntax().ancestors().any(|parent| {
                    matches!(
                        parent.kind(),
                        NodeKind::ImportDirective
                            | NodeKind::ExportDirective
                            | NodeKind::PartDirective
                            | NodeKind::PartOfDirective
                    )
                })
                && literal.segments().all(|(segment, _)| segment.terminated)
        })
}

pub(super) fn literal_value(literal: StringLitNode<'_>) -> Option<Cow<'_, str>> {
    if literal.interpolations().next().is_some() {
        return None;
    }
    if let Some(segment) = literal.delimiter()
        && (segment.is_raw || !segment.content.contains('\\'))
    {
        let content = if segment.triple {
            &segment.content[multiline_lead_len(segment.content)..]
        } else {
            segment.content
        };
        return Some(Cow::Borrowed(content));
    }
    Some(Cow::Owned(uri_value(literal)))
}

pub(super) fn dynamic_parts(literal: StringLitNode<'_>) -> Option<(String, String)> {
    literal.interpolations().next()?;
    let mut prefix = String::new();
    let mut suffix = String::new();
    let mut dynamic = false;
    for (segment, _) in literal.segments() {
        for (index, part) in literal_parts(&segment).into_iter().enumerate() {
            match part {
                LiteralPart::Interpolation(_) => {
                    dynamic = true;
                    suffix.clear();
                }
                LiteralPart::Text(text) => {
                    let text = if index == 0 && segment.triple {
                        &text[multiline_lead_len(text)..]
                    } else {
                        text
                    };
                    let text = if segment.is_raw {
                        Cow::Borrowed(text)
                    } else {
                        decode_escapes(text)
                    };
                    if dynamic {
                        suffix.push_str(&text);
                    } else {
                        prefix.push_str(&text);
                    }
                }
            }
        }
    }
    Some((prefix, suffix))
}

pub(super) fn literal_package(literal: StringLitNode<'_>) -> Option<Cow<'_, str>> {
    let argument = package_argument(literal)?;
    let Expr::StringLit(package) = argument.value() else {
        return None;
    };
    literal_value(package)
}

pub(super) fn has_package_argument(literal: StringLitNode<'_>) -> bool {
    package_argument(literal).is_some()
}

fn package_argument(literal: StringLitNode<'_>) -> Option<NamedArg<'_>> {
    let arguments = literal
        .syntax()
        .ancestors()
        .take_while(|ancestor| !matches!(ancestor.kind(), NodeKind::FuncExpr | NodeKind::Block))
        .find_map(ArgList::cast)?;
    arguments
        .named()
        .find(|argument| argument.name().name() == "package")
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{AnalysisEnvironment, CrossFileRule, ParsedFile};
    use falcon_config::FalconConfig;
    use falcon_diagnostics::{Diagnostic, Severity};
    use falcon_syntax::cst::Span as CstSpan;
    use std::sync::Arc;

    fn files(root: &std::path::Path) -> [ProjectFile; 1] {
        [ProjectFile::new(
            root.join("lib/main.dart"),
            Arc::new(ParsedFile::parse("const path = 'assets/missing.png';")),
        )
        .unwrap()]
    }

    fn assert_asset_diagnostics(source: &str, unused_span: CstSpan) {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        std::fs::write(&manifest, source).unwrap();
        let files = files(root.path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        let mut unused = Diagnostic::new(
            "",
            Severity::Warning,
            "Asset `assets/unused.png` is declared in `pubspec.yaml` but never referenced.",
            manifest.to_string_lossy(),
            unused_span,
        );
        unused.resolve_position(source);
        let mut missing = Diagnostic::new(
            "",
            Severity::Warning,
            "Asset `assets/missing.png` is not declared under `flutter: assets:` in `pubspec.yaml`.",
            files[0].path().to_string_lossy(),
            CstSpan::new(13, 33),
        );
        missing.resolve_position(files[0].source());
        for (rule, expected) in [
            (
                &super::super::unused_assets::UnusedAssets as &dyn CrossFileRule,
                unused,
            ),
            (
                &super::super::missing_assets::MissingAssets as &dyn CrossFileRule,
                missing,
            ),
        ] {
            assert_eq!(
                serde_json::to_value(rule.prepare_project(
                    &environment,
                    &files,
                    &FalconConfig::default(),
                    None,
                ))
                .unwrap(),
                serde_json::to_value([expected]).unwrap(),
            );
        }
    }

    #[test]
    fn bom_first_flutter_key_preserves_exact_asset_diagnostics() {
        assert_asset_diagnostics(
            "\u{feff}flutter:\n  assets:\n    - assets/unused.png\nname: app\n",
            CstSpan::new(28, 45),
        );
    }

    #[test]
    fn bom_first_flutter_key_with_crlf_preserves_exact_asset_diagnostics() {
        assert_asset_diagnostics(
            "\u{feff}flutter:\r\n  assets:\r\n    - assets/unused.png\r\nname: app\r\n",
            CstSpan::new(30, 47),
        );
    }

    #[test]
    fn assets_use_manifest_snapshot_after_manifest_deletion() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        std::fs::write(
            &manifest,
            "name: app\nflutter:\n  assets:\n    - assets/unused.png\n",
        )
        .unwrap();
        let files = files(root.path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        let rules: [&dyn CrossFileRule; 2] = [
            &super::super::unused_assets::UnusedAssets,
            &super::super::missing_assets::MissingAssets,
        ];
        let config = FalconConfig::default();
        let before: Vec<_> = rules
            .iter()
            .map(|rule| rule.prepare_project(&environment, &files, &config, None))
            .collect();
        assert_eq!(before.iter().map(Vec::len).collect::<Vec<_>>(), [1, 1]);
        std::fs::remove_file(&manifest).unwrap();
        for (rule, expected) in rules.iter().zip(before) {
            assert_eq!(
                serde_json::to_value(rule.prepare_project(&environment, &files, &config, None))
                    .unwrap(),
                serde_json::to_value(expected).unwrap(),
            );
        }
    }

    #[test]
    fn duplicate_key_manifests_warn_and_skip_both_asset_rules() {
        let root = tempfile::tempdir().unwrap();
        let manifest = root.path().join("pubspec.yaml");
        std::fs::write(
            &manifest,
            "name: app\nname: duplicate\nflutter:\n  assets:\n    - assets/unused.png\n",
        )
        .unwrap();
        let files = files(root.path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        assert!(matches!(
            environment.package(files[0].path()).unwrap().snapshot(),
            ManifestSnapshot::Malformed(_),
        ));
        for rule in [
            &super::super::unused_assets::UnusedAssets as &dyn CrossFileRule,
            &super::super::missing_assets::MissingAssets,
        ] {
            assert!(
                rule.prepare_project(&environment, &files, &FalconConfig::default(), None)
                    .is_empty()
            );
        }
        let warnings = environment.take_warnings();
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains(manifest.to_str().unwrap()));
        assert!(warnings[0].contains("malformed manifest:"));
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
    }

    #[cfg(unix)]
    #[test]
    fn dangling_asset_symlink_does_not_exempt_other_directory_files() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("pubspec.yaml"),
            "flutter:\n  assets:\n    - assets/icons/\n",
        )
        .unwrap();
        let directory = root.path().join("assets/icons");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("unused.png"), "asset").unwrap();
        let files = files(root.path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        let rule = super::super::unused_assets::UnusedAssets;
        let config = FalconConfig::default();
        let before = rule.prepare_project(&environment, &files, &config, None);
        assert_eq!(before.len(), 1);
        let broken = directory.join("broken.png");
        std::os::unix::fs::symlink("absent.png", &broken).unwrap();
        environment.refresh_dependency_inputs(&[broken]);
        assert_eq!(
            serde_json::to_value(rule.prepare_project(&environment, &files, &config, None))
                .unwrap(),
            serde_json::to_value(before).unwrap(),
        );
        assert_eq!(environment.take_warnings(), Vec::<String>::new());
    }

    #[cfg(unix)]
    #[test]
    fn asset_entry_io_failures_warn_and_exempt_the_directory() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("pubspec.yaml"),
            "flutter:\n  assets:\n    - assets/icons/\n",
        )
        .unwrap();
        let directory = root.path().join("assets/icons");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("unused.png"), "asset").unwrap();
        let cycle = directory.join("cycle.png");
        std::os::unix::fs::symlink("cycle.png", &cycle).unwrap();
        let reason = std::fs::metadata(&cycle).unwrap_err().to_string();
        let files = files(root.path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        let diagnostics = super::super::unused_assets::UnusedAssets.prepare_project(
            &environment,
            &files,
            &FalconConfig::default(),
            None,
        );
        assert!(diagnostics.is_empty());
        assert_eq!(
            environment.take_warnings(),
            [format!(
                "{}: {}: {reason}",
                directory.display(),
                cycle.display()
            )],
        );
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_asset_directory_warns_once_per_episode_and_is_exempt() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("pubspec.yaml"),
            "flutter:\n  assets:\n    - assets/icons/\n",
        )
        .unwrap();
        let directory = root.path().join("assets/icons");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("unused.png"), "asset").unwrap();
        let files = files(root.path());
        let mut environment = AnalysisEnvironment::discover_with_sdk_locator(
            files.iter().map(ProjectFile::path),
            |_| None,
        );
        let permissions = std::fs::metadata(&directory).unwrap().permissions();
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o0)).unwrap();
        if std::fs::read_dir(&directory).is_ok() {
            std::fs::set_permissions(&directory, permissions).unwrap();
            eprintln!("skipping unreadable asset directory: permissions are bypassed");
            return;
        }
        let rule = super::super::unused_assets::UnusedAssets;
        let config = FalconConfig::default();
        let first = rule.prepare_project(&environment, &files, &config, None);
        let warnings = environment.take_warnings();
        let repeated = rule.prepare_project(&environment, &files, &config, None);
        let repeated_warnings = environment.take_warnings();
        environment.reset();
        environment.extend(files.iter().map(ProjectFile::path));
        let next = rule.prepare_project(&environment, &files, &config, None);
        let next_warnings = environment.take_warnings();
        std::fs::set_permissions(&directory, permissions).unwrap();
        assert!(first.is_empty() && repeated.is_empty() && next.is_empty());
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains(directory.to_str().unwrap()));
        assert!(warnings[0].contains("Permission denied"));
        assert_eq!(repeated_warnings, Vec::<String>::new());
        assert_eq!(warnings, next_warnings);
    }
}
