use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

use falcon_analyze::ProjectFile;
use falcon_diagnostics::Span;
use falcon_syntax::cst::{ArgList, Expr, NamedArg, NodeKind, StringLitNode};
use marked_yaml::{LoaderOptions, Node, parse_yaml_with_options, types::MarkedScalarNode};

use super::{character_to_byte, enclosing_pubspec, scalar_source_span};
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
    pub source: String,
    pub name: Option<String>,
    pub entries: Vec<AssetEntry>,
    pub declarations: Vec<String>,
    pub files: Vec<&'a ProjectFile>,
    pub generated: bool,
}

pub(super) fn asset_packages(files: &[ProjectFile], include_spans: bool) -> Vec<AssetPackage<'_>> {
    let mut packages: BTreeMap<PathBuf, Vec<&ProjectFile>> = BTreeMap::new();
    for file in files {
        let Some(pubspec) = enclosing_pubspec(&file.path) else {
            continue;
        };
        let package = packages.entry(pubspec).or_default();
        if file
            .path
            .extension()
            .is_some_and(|extension| extension == "dart")
        {
            package.push(file);
        }
    }
    packages
        .into_iter()
        .filter_map(|(pubspec, files)| {
            let source = falcon_config::read_metadata_text(&pubspec).ok()?;
            // marked-yaml 0.8 rejects anchors and has no option to enable them.
            let Ok(root) = parse_yaml_with_options(0, &source, LoaderOptions::default()) else {
                return if include_spans {
                    None
                } else {
                    fallback_asset_package(pubspec, source, files)
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
            let mut declarations = Vec::with_capacity(if include_spans { 0 } else { capacity });
            for scalar in assets
                .into_iter()
                .flat_map(|assets| assets.iter())
                .filter_map(asset_scalar)
            {
                let path = scalar.as_str().to_owned();
                if include_spans {
                    let span = scalar
                        .span()
                        .start()
                        .and_then(|marker| character_to_byte(&source, marker.character()))
                        .and_then(|start| scalar_source_span(&source, start));
                    entries.push(AssetEntry { path, span });
                } else {
                    declarations.push(path);
                }
            }
            if !include_spans && let Some(flutter) = flutter {
                additional_declarations(flutter, &mut declarations);
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

fn additional_declarations(
    flutter: &marked_yaml::types::MarkedMappingNode,
    declarations: &mut Vec<String>,
) {
    for family in flutter
        .get_sequence("fonts")
        .into_iter()
        .flat_map(|fonts| fonts.iter())
        .filter_map(Node::as_mapping)
    {
        declarations.extend(
            family
                .get_sequence("fonts")
                .into_iter()
                .flat_map(|fonts| fonts.iter())
                .filter_map(Node::as_mapping)
                .filter_map(|font| font.get_scalar("asset"))
                .map(|asset| asset.as_str().to_owned()),
        );
    }
    declarations.extend(
        flutter
            .get_sequence("shaders")
            .into_iter()
            .flat_map(|shaders| shaders.iter())
            .filter_map(Node::as_scalar)
            .map(|shader| shader.as_str().to_owned()),
    );
    for component in flutter
        .get_sequence("deferred-components")
        .into_iter()
        .flat_map(|components| components.iter())
        .filter_map(Node::as_mapping)
    {
        declarations.extend(
            component
                .get_sequence("assets")
                .into_iter()
                .flat_map(|assets| assets.iter())
                .filter_map(asset_scalar)
                .map(|asset| asset.as_str().to_owned()),
        );
    }
}

fn fallback_asset_package(
    pubspec: PathBuf,
    source: String,
    files: Vec<&ProjectFile>,
) -> Option<AssetPackage<'_>> {
    use serde_yaml::Value;

    let root: Value = serde_yaml::from_str(&source).ok()?;
    let flutter = root.get("flutter")?;
    let name = root.get("name").and_then(Value::as_str).map(str::to_owned);
    let generated = root.get("flutter_gen").is_some()
        || ["dependencies", "dev_dependencies"]
            .into_iter()
            .any(|section| {
                root.get(section)
                    .and_then(|dependencies| dependencies.get("flutter_gen_runner"))
                    .is_some()
            });
    let asset_path = |asset: &Value| {
        let scalar = asset.get("path").unwrap_or(asset);
        scalar.as_str().map(str::to_owned)
    };
    let mut declarations: Vec<_> = flutter
        .get("assets")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(asset_path)
        .collect();
    for family in flutter
        .get("fonts")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
    {
        declarations.extend(
            family
                .get("fonts")
                .and_then(Value::as_sequence)
                .into_iter()
                .flatten()
                .filter_map(|font| font.get("asset").and_then(Value::as_str))
                .map(str::to_owned),
        );
    }
    declarations.extend(
        flutter
            .get("shaders")
            .and_then(Value::as_sequence)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_owned),
    );
    for component in flutter
        .get("deferred-components")
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
    {
        declarations.extend(
            component
                .get("assets")
                .and_then(Value::as_sequence)
                .into_iter()
                .flatten()
                .filter_map(asset_path),
        );
    }
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
    file.parsed
        .tree
        .root(&file.parsed.source)
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
