use crate::{
    corpus::cross_file_project_roots,
    support::{FalconRunner, build_falcon, docgen_config_for, recursive_files},
    workspace_root,
};
use falcon_rules::meta::{Domain, RULE_METADATA, RuleMeta, RuleSource};
use serde_json::{Map, Value, json};
use std::{
    collections::HashSet,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

fn character_column(source: &str, line: usize, utf16_column: usize) -> usize {
    let text = source.lines().nth(line.saturating_sub(1)).unwrap_or("");
    let mut units = 0;
    let prefix_bytes: usize = text
        .chars()
        .take_while(|character| {
            units += character.len_utf16();
            units < utf16_column
        })
        .map(char::len_utf8)
        .sum();
    let column = expectation_segments(text)
        .map(|(start, segment)| {
            let end = prefix_bytes.saturating_sub(start).min(segment.len());
            segment[..end].chars().count()
        })
        .sum::<usize>();
    let trimmed_length = expectation_segments(text)
        .flat_map(|(_, segment)| segment.chars())
        .enumerate()
        .filter_map(|(index, character)| (!character.is_whitespace()).then_some(index + 1))
        .last()
        .unwrap_or(0);
    1 + column.min(trimmed_length)
}

/// Map a rule's `snake_case` module name from its kebab id.
fn snake_of(name: &str) -> String {
    name.replace('-', "_")
}

/// The source file path for a rule's implementation.
fn rule_source_path(root: &Path, meta: &RuleMeta) -> PathBuf {
    let snake = snake_of(meta.name);
    if meta.is_cross_file() {
        root.join(format!("crates/falcon_rules/src/cross_file/{snake}.rs"))
    } else {
        root.join(format!(
            "crates/falcon_rules/src/lint/{}/{}.rs",
            meta.group.as_str(),
            snake
        ))
    }
}

const PROVENANCE: [&str; 4] = ["Ported from", "Port of", "Original to", "Adopted from"];

fn extract_description(lines: &[String], rule_name: &str) -> String {
    let mut doc = String::new();
    for line in lines {
        let segment = line.trim();
        if segment.is_empty() {
            if !doc.is_empty() {
                break;
            }
            continue;
        }
        if !doc.is_empty() {
            doc.push(' ');
        }
        doc.push_str(segment);
        if doc.contains(". ") {
            break;
        }
    }
    if doc.is_empty() {
        eprintln!("warning: {rule_name}: no //! module doc found for description");
        return String::new();
    }

    // Take the first sentence (up to and including the first period followed by a
    // space, or the whole string if there is no sentence break).
    let first = match doc.find(". ") {
        Some(idx) => doc[..=idx].trim().to_string(),
        None => doc.trim().to_string(),
    };

    // Safety net: if the first sentence is itself a provenance note, drop it.
    if PROVENANCE.iter().any(|p| first.starts_with(p)) {
        return String::new();
    }
    first
}

/// Read the summary and full markdown docs, stripping provenance from the latter.
fn extract_docs(source_path: &Path, rule_name: &str) -> (String, String) {
    let Ok(content) = fs::read_to_string(source_path) else {
        eprintln!(
            "warning: {rule_name}: cannot read source {} for docs",
            source_path.display()
        );
        return (String::new(), String::new());
    };

    // Collect the contiguous leading run of `//!` lines (blank `//!` lines
    // included as paragraph breaks). Stop at the first non-`//!` line once the
    // block has started.
    let mut lines: Vec<String> = Vec::new();
    let mut started = false;
    let mut description_before_code = true;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("//!") {
            started = true;
            // Drop exactly one leading space after the marker, if present.
            let text = rest.strip_prefix(' ').unwrap_or(rest);
            lines.push(text.to_string());
        } else if started {
            break;
        } else if !trimmed.is_empty() && !trimmed.starts_with("//") {
            description_before_code = false;
        }
        // Before the block starts, skip anything (blank lines, attributes).
    }
    let description = extract_description(
        if description_before_code { &lines } else { &[] },
        rule_name,
    );

    strip_provenance(&mut lines);

    // Trim leading and trailing blank lines so the markdown starts at the summary.
    while lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.trim().is_empty()) {
        lines.pop();
    }

    if lines.is_empty() {
        eprintln!("warning: {rule_name}: no //! module doc found for docs");
    }
    (description, lines.join("\n"))
}

/// Byte index of the period that ends a sentence: one followed by whitespace or
/// the end of the text. A period inside `List.filled` or `package:lints` does not
/// count, so a wrapped provenance sentence is cut at its real boundary.
fn sentence_end(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|&(i, c)| c == '.' && text[i + 1..].chars().next().is_none_or(char::is_whitespace))
        .map(|(i, _)| i)
}

fn strip_provenance(lines: &mut Vec<String>) {
    let mut i = 0;
    while i < lines.len() {
        let Some(cut) = PROVENANCE.iter().filter_map(|p| lines[i].find(p)).min() else {
            i += 1;
            continue;
        };
        let end = sentence_end(&lines[i][cut..]);
        let wraps = end.is_none();
        if let Some(end) = end {
            let rest = cut + end + 1;
            let rest = rest + lines[i][rest..].len() - lines[i][rest..].trim_start().len();
            lines[i].replace_range(cut..rest, "");
        } else {
            lines[i].truncate(cut);
        }
        let trimmed_len = lines[i].trim_end().len();
        lines[i].truncate(trimmed_len);

        while wraps && i + 1 < lines.len() && !lines[i + 1].trim().is_empty() {
            match sentence_end(&lines[i + 1]) {
                // Anything after the closing period belongs to the next
                // sentence, so keep it.
                Some(p) => {
                    let rest = lines[i + 1][p + 1..].trim_start().to_string();
                    if rest.is_empty() {
                        lines.remove(i + 1);
                    } else {
                        lines[i + 1] = rest;
                    }
                    break;
                }
                None => {
                    lines.remove(i + 1);
                }
            }
        }
        if lines[i].is_empty() {
            lines.remove(i);
            if i > 0
                && lines[i - 1].trim().is_empty()
                && lines.get(i).is_some_and(|line| line.trim().is_empty())
            {
                lines.remove(i);
            }
        } else {
            i += 1;
        }
    }
}

fn expectation_segments(mut line: &str) -> impl Iterator<Item = (usize, &str)> {
    let length = line.len();
    std::iter::from_fn(move || {
        if line.is_empty() {
            return None;
        }
        let offset = length - line.len();
        if let Some(start) = line.find("/* expect:") {
            let before = &line[..start];
            line = line[start..]
                .find("*/")
                .map_or("", |end| &line[start + end + 2..]);
            Some((offset, before))
        } else {
            Some((offset, std::mem::take(&mut line)))
        }
    })
}

/// Remove all `/* expect: … */` annotation comments from fixture source, then
/// rstrip each line so the displayed code is clean.
fn strip_expectations(source: &str) -> String {
    let mut cleaned = String::with_capacity(source.len());
    for (i, line) in source.lines().enumerate() {
        if i > 0 {
            cleaned.push('\n');
        }
        let mut out = String::with_capacity(line.len());
        for (_, segment) in expectation_segments(line) {
            out.push_str(segment);
        }
        cleaned.push_str(out.trim_end());
    }
    cleaned
}

/// Human-facing source provenance for a rule.
fn source_json(source: &RuleSource) -> Value {
    let (kind, label, upstream_id, upstream_url): (&str, &str, Option<String>, Option<String>) =
        match source {
            RuleSource::Lints(id) => (
                "Lints",
                "Dart lints",
                Some((*id).to_string()),
                Some(format!("https://dart.dev/tools/linter-rules/{id}")),
            ),
            // DCM rule ids don't map to a stable per-rule URL without the rule's
            // DCM category, so keep the id + label but leave the URL null.
            RuleSource::DartCodeLinter(id) => (
                "DartCodeLinter",
                "Dart Code Metrics",
                Some((*id).to_string()),
                None,
            ),
            RuleSource::PyramidLint(id) => (
                "PyramidLint",
                "Pyramid Lint",
                Some((*id).to_string()),
                Some("https://pub.dev/packages/pyramid_lint".to_string()),
            ),
            RuleSource::Falcon => ("Falcon", "Falcon", None, None),
        };
    json!({
        "kind": kind,
        "label": label,
        "upstreamId": upstream_id,
        "upstreamUrl": upstream_url,
    })
}

/// One CLI argument (option or positional) rendered from its clap definition.
fn cli_arg_json(arg: &clap::Arg) -> Value {
    // A flag (no value) carries no meaningful value name — only surface one for
    // args that actually take a value.
    let takes_value = arg.get_num_args().is_some_and(|r| r.takes_values());
    let value_name = takes_value
        .then(|| {
            arg.get_value_names()
                .and_then(|vs| vs.first())
                .map(ToString::to_string)
        })
        .flatten();

    let defaults: Vec<String> = arg
        .get_default_values()
        .iter()
        .map(|v| v.to_string_lossy().to_string())
        .collect();
    let default = match defaults.as_slice() {
        [] => Value::Null,
        [one] => json!(one),
        many => json!(many),
    };

    let possible_values: Vec<String> = arg
        .get_possible_values()
        .iter()
        .map(|pv| pv.get_name().to_string())
        .collect();

    json!({
        "name": arg.get_id().as_str(),
        "long": arg.get_long().map(|l| format!("--{l}")),
        "short": arg.get_short().map(|c| format!("-{c}")),
        "valueName": value_name,
        "default": default,
        "help": arg.get_help().map(ToString::to_string),
        "possibleValues": possible_values,
        "positional": arg.is_positional(),
    })
}

/// Whether an arg is a clap auto-generated help/version flag (excluded from the
/// reference — every command has them and they carry no falcon-specific meaning).
fn is_builtin_flag(arg: &clap::Arg) -> bool {
    matches!(arg.get_id().as_str(), "help" | "version")
}

/// Build the CLI reference from falcon's real clap `Command` tree: global args
/// plus one section per subcommand (name, about, usage, args).
fn cli_reference_json(mut root: clap::Command) -> Value {
    // Propagate globals into subcommands and finalize usage strings.
    root.build();

    let global_ids: HashSet<String> = root
        .get_arguments()
        .filter(|a| a.is_global_set())
        .map(|a| a.get_id().to_string())
        .collect();

    let mut globals: Vec<(&str, Value)> = root
        .get_arguments()
        .filter(|a| a.is_global_set() && !is_builtin_flag(a))
        .map(|a| (a.get_id().as_str(), cli_arg_json(a)))
        .collect();
    globals.sort_by(|a, b| a.0.cmp(b.0));
    let globals: Vec<Value> = globals.into_iter().map(|(_, v)| v).collect();

    // After `build()`, clap's rendered usage already carries the `falcon` bin
    // name; only the leading "Usage: " label needs trimming.
    let root_usage_raw = root.render_usage().to_string();
    let root_usage = root_usage_raw
        .strip_prefix("Usage: ")
        .unwrap_or(&root_usage_raw)
        .to_string();

    let mut subcommands: Vec<(String, Value)> = Vec::new();
    for sub in root.get_subcommands() {
        // Skip clap's auto-generated `help` subcommand — it is not a falcon command.
        if sub.get_name() == "help" {
            continue;
        }
        let mut sub = sub.clone();
        sub.build();
        let name = sub.get_name().to_string();

        let usage_raw = sub.render_usage().to_string();
        let usage = usage_raw
            .strip_prefix("Usage: ")
            .unwrap_or(&usage_raw)
            .to_string();

        let args: Vec<Value> = sub
            .get_arguments()
            .filter(|a| !global_ids.contains(a.get_id().as_str()) && !is_builtin_flag(a))
            .map(cli_arg_json)
            .collect();

        subcommands.push((
            name.clone(),
            json!({
                "name": name,
                "about": sub.get_about().map(ToString::to_string),
                "usage": usage,
                "args": args,
            }),
        ));
    }
    subcommands.sort_by(|a, b| a.0.cmp(&b.0));
    let subcommands: Vec<Value> = subcommands.into_iter().map(|(_, v)| v).collect();

    json!({
        "name": root.get_name(),
        "about": root.get_about().map(ToString::to_string),
        "version": root.get_version(),
        "usage": root_usage,
        "globalArgs": globals,
        "subcommands": subcommands,
    })
}

/// Resolve a `{"$ref": "#/definitions/X"}` node against the root schema, or
/// return the node unchanged when it is not a local `$ref`.
fn schema_resolve<'a>(root: &'a Value, node: &'a Value) -> &'a Value {
    node.get("$ref")
        .and_then(|r| r.as_str())
        .and_then(|r| r.strip_prefix("#/definitions/"))
        .and_then(|name| root.pointer(&format!("/definitions/{name}")))
        .unwrap_or(node)
}

/// A human-readable type label for a schema node (following `$ref`s).
fn schema_type_label(root: &Value, node: &Value) -> String {
    let resolved = schema_resolve(root, node);
    if let Some(branches) = resolved
        .get("oneOf")
        .or_else(|| resolved.get("anyOf"))
        .and_then(Value::as_array)
    {
        let mut types: Vec<_> = branches
            .iter()
            .map(|branch| schema_type_label(root, branch))
            .collect();
        types.sort_unstable();
        types.dedup();
        return types.join(" | ");
    }
    let type_label = |kind: &str| {
        if kind == "array" {
            let item = resolved
                .get("items")
                .map_or_else(|| "value".to_string(), |item| schema_type_label(root, item));
            if item.contains(" | ") {
                format!("({item})[]")
            } else {
                format!("{item}[]")
            }
        } else {
            kind.to_string()
        }
    };
    match resolved.get("type") {
        Some(Value::String(kind)) => type_label(kind),
        Some(Value::Array(parts)) => parts
            .iter()
            .filter_map(Value::as_str)
            .map(type_label)
            .collect::<Vec<_>>()
            .join(" | "),
        _ if resolved.get("enum").is_some() => "string".to_string(),
        _ => "object".to_string(),
    }
}

/// The accepted enum values, including a rule entry's severity branch.
fn schema_allowed_values(root: &Value, node: &Value) -> Vec<Value> {
    let resolved = schema_resolve(root, node);
    let branches = resolved
        .get("oneOf")
        .or_else(|| resolved.get("anyOf"))
        .and_then(Value::as_array);
    let values = if let Some(Value::Array(values)) = resolved.get("enum") {
        values.clone()
    } else if let Some(branches) = branches {
        branches
            .iter()
            .flat_map(|branch| schema_allowed_values(root, branch))
            .collect()
    } else if let Some(items) = resolved.get("items") {
        schema_allowed_values(root, items)
    } else {
        Vec::new()
    };
    let mut allowed = values;
    for index in (0..allowed.len()).rev() {
        if allowed[..index].contains(&allowed[index]) {
            allowed.remove(index);
        }
    }
    let nullable = match resolved.get("type") {
        Some(Value::String(kind)) => kind == "null",
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind.as_str() == Some("null")),
        _ => false,
    } || branches.is_some_and(|branches| {
        branches.iter().any(|branch| {
            schema_resolve(root, branch)
                .get("type")
                .and_then(Value::as_str)
                == Some("null")
        })
    });
    if !allowed.is_empty() && nullable && !allowed.contains(&Value::Null) {
        allowed.push(Value::Null);
    }
    allowed
}

/// Build one flat config-reference entry for a leaf key.
fn config_leaf(root: &Value, node: &Value, path: &str) -> Value {
    let resolved = schema_resolve(root, node);
    let description = node
        .get("description")
        .or_else(|| resolved.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let mut obj = Map::new();
    obj.insert("path".into(), json!(path));
    obj.insert("type".into(), json!(schema_type_label(root, node)));
    let allowed = schema_allowed_values(root, node);
    if !allowed.is_empty() {
        obj.insert("allowedValues".into(), Value::Array(allowed));
    }
    if let Some(default) = node.get("default").or_else(|| resolved.get("default")) {
        obj.insert("default".into(), default.clone());
    }
    obj.insert("description".into(), json!(description));
    Value::Object(obj)
}

const OPTIONS_START: &str = "<!-- rule-options:start -->";
const OPTIONS_END: &str = "<!-- rule-options:end -->";

fn markdown_cell(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace('`', "&#96;")
        .replace(['\r', '\n'], " ")
}

fn render_option_rows(root: &Value, node: &Value, rule: &str, path: &str, output: &mut String) {
    let resolved = schema_resolve(root, node);
    if let Some(properties) = resolved.get("properties").and_then(Value::as_object) {
        let mut properties: Vec<_> = properties.iter().collect();
        properties.sort_unstable_by_key(|(name, _)| *name);
        for (name, property) in properties {
            let child_path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            let leaf = config_leaf(root, property, &child_path);
            let default = leaf.get("default").map_or_else(
                || {
                    let required = resolved
                        .get("required")
                        .and_then(Value::as_array)
                        .is_some_and(|fields| {
                            fields
                                .iter()
                                .any(|field| field.as_str() == Some(name.as_str()))
                        });
                    if required { "required" } else { "unset" }.to_string()
                },
                |value| format!("`{}`", markdown_cell(&value.to_string())),
            );
            let allowed = leaf
                .get("allowedValues")
                .and_then(Value::as_array)
                .map_or_default(|values| {
                    values
                        .iter()
                        .map(|value| format!("`{}`", markdown_cell(&value.to_string())))
                        .collect::<Vec<_>>()
                        .join(", ")
                });
            writeln!(
                output,
                "| `{rule}` | `{child_path}` | {} | {default} | {} | {allowed} |",
                markdown_cell(leaf["type"].as_str().unwrap_or("")),
                markdown_cell(leaf["description"].as_str().unwrap_or("")),
            )
            .expect("writing to a string cannot fail");
            render_option_rows(root, property, rule, &child_path, output);
        }
    } else if let Some(items) = resolved.get("items") {
        render_option_rows(root, items, rule, &format!("{path}[]"), output);
    } else if let Some(branches) = resolved
        .get("oneOf")
        .or_else(|| resolved.get("anyOf"))
        .and_then(Value::as_array)
    {
        for branch in branches {
            render_option_rows(root, branch, rule, path, output);
        }
    }
}

fn rule_options_markdown(document: &str, schemas: &[(&str, Value)]) -> Result<String, String> {
    let (before, rest) = document
        .split_once(OPTIONS_START)
        .ok_or_else(|| format!("configuration.md is missing {OPTIONS_START}"))?;
    let (_, after) = rest
        .split_once(OPTIONS_END)
        .ok_or_else(|| format!("configuration.md is missing {OPTIONS_END}"))?;
    let mut output = format!(
        "{before}{OPTIONS_START}\n\n| Rule | Option | Type | Default | Description | Allowed values |\n|------|--------|------|---------|-------------|----------------|\n"
    );
    for (rule, schema) in schemas {
        if !schema.get("properties").is_some_and(Value::is_object) {
            return Err(format!("{rule}: options schema has no object properties"));
        }
        render_option_rows(schema, schema, rule, "", &mut output);
    }
    write!(output, "\n{OPTIONS_END}{after}").expect("writing to a string cannot fail");
    Ok(output)
}

/// Emit the optional `recommended` toggle and a templated rule entry (per-rule
/// detail lives in rules.json, so the reference stays structural).
fn emit_rules_section(root: &Value, node: &Value, path: &str, out: &mut Vec<Value>) {
    if let Some(rec) = node.pointer("/properties/recommended") {
        out.push(config_leaf(root, rec, &format!("{path}.recommended")));
    }
    let rule_config = node
        .get("properties")
        .and_then(Value::as_object)
        .and_then(|groups| {
            groups.values().find_map(|group| {
                schema_resolve(root, group)
                    .get("properties")
                    .and_then(Value::as_object)
                    .and_then(|rules| rules.values().next())
            })
        })
        .expect("rules section contains a rule schema");
    let mut entry = config_leaf(root, rule_config, &format!("{path}.<group>.<rule>"));
    entry["type"] = json!("rule level (or object with level + options)");
    out.push(entry);
}

/// Recursively walk the config schema, emitting a flat list of config-key
/// entries. Object keys recurse; `rules` sections and repetitive `overrides`
/// items collapse to templated paths; everything else is a leaf.
fn walk_config_schema(root: &Value, node: &Value, path: &str, out: &mut Vec<Value>) {
    let resolved = schema_resolve(root, node);
    // Nullable sections still expose their object's keys in the reference.
    if let Some(branches) = resolved
        .get("anyOf")
        .or_else(|| resolved.get("oneOf"))
        .and_then(Value::as_array)
        && branches.len() == 2
        && branches
            .iter()
            .any(|branch| schema_resolve(root, branch)["type"] == "null")
        && let Some(object) = branches
            .iter()
            .find(|branch| schema_resolve(root, branch)["properties"].is_object())
    {
        walk_config_schema(root, object, path, out);
        return;
    }

    if let Some(Value::Object(props)) = resolved.get("properties") {
        if path
            .rsplit_once('.')
            .is_some_and(|(_, section)| section == "rules")
        {
            emit_rules_section(root, resolved, path, out);
            return;
        }
        for (key, child) in props {
            let child_path = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            walk_config_schema(root, child, &child_path, out);
        }
        return;
    }

    // Array of objects → descend into the item schema with a `[]` marker.
    if resolved.get("type").and_then(|t| t.as_str()) == Some("array")
        && let Some(items) = resolved.get("items")
        && matches!(
            schema_resolve(root, items).get("properties"),
            Some(Value::Object(_))
        )
    {
        walk_config_schema(root, items, &format!("{path}[]"), out);
        return;
    }

    out.push(config_leaf(root, node, path));
}

/// Build the per-key config reference by walking the committed
/// `schema/falcon.schema.json` (the metadata-generated source of truth).
fn config_reference_json(root: &Path) -> Value {
    let schema_path = root.join("schema/falcon.schema.json");
    let schema: Value = match fs::read_to_string(&schema_path) {
        Ok(s) => serde_json::from_str(&s).expect("schema is valid JSON"),
        Err(e) => {
            eprintln!(
                "error: cannot read {} for config reference: {e}",
                schema_path.display()
            );
            std::process::exit(1);
        }
    };
    let mut out: Vec<Value> = Vec::new();
    walk_config_schema(&schema, &schema, "", &mut out);
    Value::Array(out)
}

fn project_fixture_directory(rule_dir: &Path, bad_source: Option<&str>) -> PathBuf {
    let cross_file = rule_dir.join("cross-file");
    let project = rule_dir.join("project");
    // A project directory can contain support files rather than examples.
    // Keep a real root bad fixture; only replace intentionally silent fallbacks.
    let has_bad_expectations =
        bad_source.is_some_and(|source| !crate::corpus::parse_expectations(source).is_empty());
    if !cross_file.is_dir() && project.is_dir() && !has_bad_expectations {
        project
    } else {
        cross_file
    }
}

fn read_fixture(path: &Path) -> Result<Option<String>, String> {
    match fs::read_to_string(path) {
        Ok(source) => Ok(Some(source)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read fixture {}: {error}", path.display())),
    }
}

/// Emit `website/src/data/rules.json` and `website/src/data/domains.json` from
/// `RULE_METADATA`, corpus fixtures, and real diagnostics from the falcon
/// binary. These committed files are the docs website's only data source; the
/// site build never needs cargo.
pub(crate) fn run(check: bool, falcon_bin: Option<PathBuf>) -> Result<(), String> {
    let root = workspace_root();
    let corpus_root = root.join("crates/falcon_rules/tests/corpus");

    let falcon_bin = falcon_bin.unwrap_or_else(|| build_falcon(false));
    let falcon = FalconRunner::new(falcon_bin, &root, &corpus_root);
    if RULE_METADATA.is_empty() {
        return Err("RULE_METADATA is empty".to_string());
    }

    let temp_directory =
        tempfile::tempdir().map_err(|error| format!("cannot create temp config: {error}"))?;
    let temp_config = temp_directory.path().join("falcon.json");

    // Deterministic order: by group, then name.
    let mut metas: Vec<&RuleMeta> = RULE_METADATA.iter().collect();
    metas.sort_by(|a, b| {
        a.group
            .as_str()
            .cmp(b.group.as_str())
            .then(a.name.cmp(b.name))
    });

    let (records, with_bad, with_good, with_diags) =
        collect_rule_records(&root, &corpus_root, &temp_config, &falcon, &metas)?;

    let domains: Vec<Value> = Domain::ALL
        .iter()
        .map(|domain| {
            let mut rules: Vec<&str> = metas
                .iter()
                .filter(|m| m.domain == Some(*domain))
                .map(|m| m.name)
                .collect();
            rules.sort_unstable();
            json!({ "name": domain.as_str(), "rules": rules })
        })
        .collect();

    let data_dir = root.join("website/src/data");
    let rules_path = data_dir.join("rules.json");
    let domains_path = data_dir.join("domains.json");
    let cli_path = data_dir.join("cli.json");
    let config_ref_path = data_dir.join("config-reference.json");

    let rules_json = serde_json::to_string_pretty(&records).expect("serialize rules.json");
    let domains_json =
        serde_json::to_string_pretty(&Value::Array(domains)).expect("serialize domains.json");
    let cli_json = serde_json::to_string_pretty(&cli_reference_json(falcon_cli::args::command()))
        .expect("serialize cli.json");
    let config_ref = config_reference_json(&root);
    let config_ref_count = config_ref.as_array().map_or(0, Vec::len);
    let config_ref_json =
        serde_json::to_string_pretty(&config_ref).expect("serialize config-reference.json");
    let configuration_path = root.join("docs/configuration.md");
    let configuration = fs::read_to_string(&configuration_path)
        .map_err(|error| format!("cannot read {}: {error}", configuration_path.display()))?;
    let configuration = rule_options_markdown(
        &configuration,
        &falcon_rules::schema::rule_options_schemas(),
    )?;

    for (path, contents) in [
        (&rules_path, rules_json.as_str()),
        (&domains_path, domains_json.as_str()),
        (&cli_path, cli_json.as_str()),
        (&config_ref_path, config_ref_json.as_str()),
        (
            &configuration_path,
            configuration.strip_suffix('\n').unwrap_or(&configuration),
        ),
    ] {
        write_or_check(path, contents, check)?;
    }

    println!();
    println!("── docgen ──────────────────────────────────────────────");
    println!("  Rules:              {}", records.len());
    println!("  With bad example:   {with_bad}");
    println!("  With good example:  {with_good}");
    println!("  With diagnostics:   {with_diags}");
    println!(
        "  Cross-file rules:   {}",
        metas.iter().filter(|m| m.is_cross_file()).count()
    );
    println!("  Config-ref keys:    {config_ref_count}");
    let action = if check { "Checked" } else { "Wrote" };
    println!("  {action}: {}", rules_path.display());
    println!("  {action}: {}", domains_path.display());
    println!("  {action}: {}", cli_path.display());
    println!("  {action}: {}", config_ref_path.display());
    println!("  {action}: {}", configuration_path.display());
    println!("────────────────────────────────────────────────────────");
    Ok(())
}

fn collect_rule_records(
    root: &Path,
    corpus_root: &Path,
    temp_config: &Path,
    falcon: &FalconRunner,
    metas: &[&RuleMeta],
) -> Result<(Vec<Value>, usize, usize, usize), String> {
    let mut records: Vec<Value> = Vec::with_capacity(metas.len());
    let mut with_bad = 0usize;
    let mut with_good = 0usize;
    let mut with_diags = 0usize;

    for meta in metas {
        let rule_dir = corpus_root.join(meta.name);

        let source_path = rule_source_path(root, meta);
        let (description, docs) = extract_docs(&source_path, meta.name);

        let config = docgen_config_for(corpus_root, meta, temp_config)?;

        // Test-domain rules only inspect `_test.dart` files, so their examples come from those fixtures.
        let fixture = |stem: &str| {
            let test = rule_dir.join(format!("{stem}_test.dart"));
            if test.exists() {
                test
            } else {
                rule_dir.join(format!("{stem}.dart"))
            }
        };
        let bad_path = fixture("bad");
        let good_path = fixture("good");
        let bad_source = read_fixture(&bad_path)?;
        let cross_file_dir = project_fixture_directory(&rule_dir, bad_source.as_deref());
        let bad_example = bad_source.as_deref().map(strip_expectations);
        let good_example = read_fixture(&good_path)?.map(|source| strip_expectations(&source));
        if bad_example.is_some() {
            with_bad += 1;
        }
        if good_example.is_some() {
            with_good += 1;
        }

        // Cross-file examples include nested Dart files and pubspecs. Relative
        // paths keep identically named files from independent packages distinct.
        let mut cross_file_sources: Vec<(String, String)> = Vec::new();
        if cross_file_dir.is_dir() {
            for file in recursive_files(&cross_file_dir).into_iter().filter(|path| {
                matches!(
                    path.extension().and_then(|extension| extension.to_str()),
                    Some("dart" | "yaml")
                )
            }) {
                let source = read_fixture(&file)?.ok_or_else(|| {
                    format!("cannot read fixture {}: file is missing", file.display())
                })?;
                let relative = file
                    .strip_prefix(&cross_file_dir)
                    .map_err(|error| format!("invalid fixture path {}: {error}", file.display()))?;
                cross_file_sources.push((relative.to_string_lossy().replace('\\', "/"), source));
            }
        }
        let cross_file_examples: Option<Value> = if cross_file_sources.is_empty() {
            None
        } else {
            Some(Value::Array(
                cross_file_sources
                    .iter()
                    .map(|(path, content)| {
                        json!({ "path": path, "content": strip_expectations(content) })
                    })
                    .collect(),
            ))
        };

        let (diagnostics, had_fixture) = fixture_diagnostics(
            meta,
            falcon,
            &config,
            &cross_file_dir,
            &cross_file_sources,
            &bad_path,
            bad_source.as_deref(),
        )?;

        validate_record(meta.name, &description, had_fixture, diagnostics.len())?;
        if !diagnostics.is_empty() {
            with_diags += 1;
        }

        records.push(json!({
            "name": meta.name,
            "group": meta.group.as_str(),
            "domains": meta.domain_names(),
            "recommended": meta.recommended,
            "crossFile": meta.is_cross_file(),
            "source": source_json(&meta.source),
            "description": description,
            "docs": docs,
            "examples": {
                "bad": bad_example,
                "good": good_example,
                "crossFile": cross_file_examples,
            },
            "diagnostics": diagnostics,
        }));
    }
    Ok((records, with_bad, with_good, with_diags))
}

fn fixture_diagnostics(
    meta: &RuleMeta,
    falcon: &FalconRunner,
    config: &Path,
    cross_file_dir: &Path,
    cross_file_sources: &[(String, String)],
    bad_path: &Path,
    bad_source: Option<&str>,
) -> Result<(Vec<Value>, bool), String> {
    let mut diagnostics: Vec<Value> = Vec::new();
    let had_fixture;
    if !cross_file_sources.is_empty() {
        had_fixture = true;
        for project in cross_file_project_roots(cross_file_dir) {
            for d in falcon
                .run(&project, Some(config))?
                .into_iter()
                .filter(|d| d.rule == meta.name)
            {
                let relative = Path::new(&d.file_path)
                    .strip_prefix(cross_file_dir)
                    .map_or_else(
                        |_| d.file_path.clone(),
                        |path| path.to_string_lossy().replace('\\', "/"),
                    );
                let source = cross_file_sources
                    .iter()
                    .find(|(path, _)| path == &relative)
                    .map_or("", |(_, source)| source.as_str());
                let (line, col) = d.resolved_position()?;
                diagnostics.push(json!({
                    "message": d.message,
                    "line": line,
                    "column": character_column(source, line, col),
                    "file": relative,
                }));
            }
        }
    } else if bad_path.exists() {
        had_fixture = true;
        let source = bad_source.unwrap_or_default();
        for d in falcon
            .run(bad_path, Some(config))?
            .into_iter()
            .filter(|d| d.rule == meta.name)
        {
            let (line, col) = d.resolved_position()?;
            diagnostics.push(json!({
                "message": d.message,
                "line": line,
                "column": character_column(source, line, col),
                "file": Value::Null,
            }));
        }
    } else {
        had_fixture = false;
    }
    Ok((diagnostics, had_fixture))
}

fn validate_record(
    name: &str,
    description: &str,
    had_fixture: bool,
    diagnostics: usize,
) -> Result<(), String> {
    if description.trim().is_empty() {
        return Err(format!("{name}: empty rule description"));
    }
    if had_fixture && diagnostics == 0 {
        return Err(format!("{name}: bad fixture emitted zero diagnostics"));
    }
    Ok(())
}

fn write_or_check(path: &Path, contents: &str, check: bool) -> Result<(), String> {
    let generated = format!("{contents}\n");
    if check {
        let current = fs::read_to_string(path).map_err(|error| {
            format!(
                "cannot read {}: {error}; run `cargo xtask docgen`",
                path.display()
            )
        })?;
        if current != generated {
            return Err(format!(
                "{} is out of date; run `cargo xtask docgen`",
                path.display()
            ));
        }
    } else {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
        }
        fs::write(path, generated)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_options_table_uses_schemas_and_preserves_surrounding_prose() {
        let document = format!("Before\n{OPTIONS_START}\nold table\n{OPTIONS_END}\nAfter\n");
        let schemas = [(
            "example-rule",
            json!({
                "properties": {
                    "threshold": {
                        "type": "integer",
                        "default": 5,
                        "description": "Maximum | count.\nMore detail."
                    },
                    "items": {
                        "type": "array",
                        "items": {"type": "string"},
                        "default": []
                    }
                }
            }),
        )];
        let generated = rule_options_markdown(&document, &schemas).unwrap();
        assert!(generated.starts_with(&format!("Before\n{OPTIONS_START}\n")));
        assert!(generated.ends_with(&format!("{OPTIONS_END}\nAfter\n")));
        assert!(!generated.contains("old table"));
        assert!(generated.contains("| `example-rule` | `items` | string[] | `[]` |  |"));
        assert!(generated.contains(
            "| `example-rule` | `threshold` | integer | `5` | Maximum &#124; count. More detail. |"
        ));
        assert_eq!(
            rule_options_markdown(&generated, &schemas).unwrap(),
            generated
        );
    }

    #[test]
    fn rule_options_table_renders_nested_fields_and_enum_values() {
        let document = format!("{OPTIONS_START}{OPTIONS_END}");
        let schema = json!({
            "properties": {
                "rules": {
                    "type": "array",
                    "items": {"$ref": "#/definitions/Entry"},
                    "default": []
                },
                "items": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["class-name"],
                        "properties": {
                            "class-name": {"type": "string"},
                            "replacement": {"type": "string"}
                        }
                    }
                },
                "settings": {
                    "anyOf": [
                        {"$ref": "#/definitions/Entry"},
                        {"type": "null"}
                    ],
                    "default": null
                },
                "directions": {
                    "type": ["array", "null"],
                    "items": {"$ref": "#/definitions/Direction"},
                    "default": null
                },
                "optional-directions": {
                    "anyOf": [
                        {"type": "array", "items": {"$ref": "#/definitions/Direction"}},
                        {"type": "null"}
                    ]
                },
                "optional-rules": {
                    "oneOf": [
                        {"type": "array", "items": {"$ref": "#/definitions/Entry"}},
                        {"type": "null"}
                    ]
                },
                "order": {
                    "anyOf": [
                        {"$ref": "#/definitions/Direction"},
                        {"type": "null"}
                    ],
                    "default": null
                }
            },
            "definitions": {
                "Entry": {
                    "type": "object",
                    "required": ["deny", "message", "files"],
                    "properties": {
                        "deny": {
                            "type": "array",
                            "items": {"type": "string"},
                            "description": "Names to reject."
                        },
                        "message": {"type": "string"},
                        "files": {"type": "array", "items": {"type": "string"}}
                    }
                },
                "Direction": {
                    "type": "string",
                    "enum": ["ascending", "descending"]
                }
            }
        });
        let generated = rule_options_markdown(&document, &[("example", schema)]).unwrap();
        assert!(generated.contains("| `example` | `rules` | object[] | `[]` |  |  |"));
        assert!(generated.contains(
            "| `example` | `rules[].deny` | string[] | required | Names to reject. |  |"
        ));
        assert!(generated.contains("| `example` | `rules[].message` | string | required |  |  |"));
        assert!(generated.contains("| `example` | `rules[].files` | string[] | required |  |  |"));
        assert!(
            generated.contains("| `example` | `items[].class-name` | string | required |  |  |")
        );
        assert!(generated.contains("| `example` | `items[].replacement` | string | unset |  |  |"));
        assert!(generated.contains("| `example` | `settings.deny` | string[] | required |"));
        assert!(generated.contains(
            "| `example` | `order` | null &#124; string | `null` |  | `\"ascending\"`, `\"descending\"`, `null` |"
        ));
        assert!(generated.contains(
            "| `example` | `directions` | string[] &#124; null | `null` |  | `\"ascending\"`, `\"descending\"`, `null` |"
        ));
        assert!(generated.contains(
            "| `example` | `optional-directions` | null &#124; string[] | unset |  | `\"ascending\"`, `\"descending\"`, `null` |"
        ));
        assert!(generated.contains(
            "| `example` | `optional-rules[].deny` | string[] | required | Names to reject. |  |"
        ));
    }

    #[test]
    fn rule_options_table_renders_nullable_arrays_consistently() {
        let document = format!("{OPTIONS_START}{OPTIONS_END}");
        for array in [
            json!({"type": ["array", "null"], "items": {"type": "string"}}),
            json!({"anyOf": [
                {"type": "array", "items": {"type": "string"}},
                {"type": "null"}
            ]}),
        ] {
            let schema = json!({"properties": {"order": array}});
            let generated = rule_options_markdown(&document, &[("example", schema)]).unwrap();
            assert!(generated.contains("string[]"));
            assert!(!generated.contains("array"));
        }
        let schema = json!({
            "type": "array",
            "items": {"type": ["string", "null"]}
        });
        assert_eq!(schema_type_label(&schema, &schema), "(string | null)[]");
        let schema = json!({
            "anyOf": [
                {"type": "string", "enum": ["ascending", "descending", "ascending"]},
                {"type": "string", "enum": ["descending"]},
                {"type": "null"}
            ]
        });
        assert_eq!(
            schema_allowed_values(&schema, &schema),
            vec![json!("ascending"), json!("descending"), Value::Null]
        );
    }

    #[test]
    fn rule_options_table_requires_markers_and_object_properties() {
        assert!(rule_options_markdown("No markers", &[]).is_err());
        assert!(rule_options_markdown(OPTIONS_START, &[]).is_err());
        let document = format!("{OPTIONS_START}{OPTIONS_END}");
        assert!(
            rule_options_markdown(&document, &[("example", json!({"type": "string"}))])
                .unwrap_err()
                .contains("options schema has no object properties")
        );
    }

    #[test]
    fn override_rule_reference_stays_structural_without_a_preset_toggle() {
        let schema = falcon_rules::schema::config_schema();
        let mut entries = Vec::new();
        walk_config_schema(&schema, &schema, "", &mut entries);
        for section in ["linter", "cross-file"] {
            let prefix = format!("overrides[].{section}.rules");
            let paths: Vec<_> = entries
                .iter()
                .filter_map(|entry| entry["path"].as_str())
                .filter(|path| path.starts_with(&prefix))
                .collect();
            assert_eq!(paths, [format!("{prefix}.<group>.<rule>")]);
        }
    }

    #[test]
    fn inline_rule_schemas_supply_structural_reference_severities() {
        let schema = falcon_rules::schema::config_schema();
        let mut entries = Vec::new();
        walk_config_schema(&schema, &schema, "", &mut entries);
        for path in [
            "linter.rules.<group>.<rule>",
            "cross-file.rules.<group>.<rule>",
            "overrides[].linter.rules.<group>.<rule>",
            "overrides[].cross-file.rules.<group>.<rule>",
        ] {
            let entry = entries.iter().find(|entry| entry["path"] == path).unwrap();
            assert_eq!(entry["type"], "rule level (or object with level + options)");
            assert_eq!(
                entry["allowedValues"],
                schema["definitions"]["ruleLevel"]["enum"]
            );
            assert!(entry["allowedValues"].is_array());
        }
    }

    #[test]
    fn option_union_types_are_not_rendered_as_rule_entries() {
        let schema = json!({
            "oneOf": [
                {"type": "string", "enum": ["ascending", "descending"]},
                {"type": "null"}
            ]
        });
        assert_eq!(schema_type_label(&schema, &schema), "null | string");
    }

    #[test]
    fn nullable_object_sections_expose_referenced_keys() {
        for union in ["anyOf", "oneOf"] {
            let schema = json!({
                "definitions": {
                    "section": {
                        "type": "object",
                        "properties": {"enabled": {"type": ["boolean", "null"]}}
                    }
                },
                "properties": {
                    "section": {
                        union: [
                            {"type": "null"},
                            {"$ref": "#/definitions/section"}
                        ]
                    }
                }
            });
            let mut entries = Vec::new();
            walk_config_schema(&schema, &schema, "", &mut entries);
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0]["path"], "section.enabled");
            assert_eq!(entries[0]["type"], "boolean | null");
        }
    }

    #[test]
    fn docgen_allows_missing_optional_fixtures() {
        let directory = tempfile::tempdir().unwrap();
        for name in ["bad.dart", "good.dart"] {
            assert_eq!(read_fixture(&directory.path().join(name)).unwrap(), None);
        }
    }

    #[test]
    fn docgen_rejects_non_utf8_fixtures() {
        let directory = tempfile::tempdir().unwrap();
        for name in [
            "bad.dart",
            "good.dart",
            "cross-file/main.dart",
            "cross-file/pubspec.yaml",
        ] {
            let path = directory.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, [0xff]).unwrap();
            let error = read_fixture(&path).unwrap_err();
            assert!(error.contains("cannot read fixture"), "{error}");
            assert!(error.contains(&path.display().to_string()), "{error}");
        }
    }

    #[test]
    fn docgen_rejects_unreadable_existing_fixtures() {
        let directory = tempfile::tempdir().unwrap();
        // A directory fails read_to_string even when tests run as root.
        for name in ["bad.dart", "good.dart"] {
            let path = directory.path().join(name);
            fs::create_dir(&path).unwrap();
            let error = read_fixture(&path).unwrap_err();
            assert!(error.contains("cannot read fixture"), "{error}");
            assert!(error.contains(&path.display().to_string()), "{error}");
        }
    }

    #[test]
    fn docgen_preserves_readable_fixture_contents() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("bad.dart");
        let source = "void main() {} /* expect: example */\n";
        fs::write(&path, source).unwrap();
        assert_eq!(read_fixture(&path).unwrap().as_deref(), Some(source));
    }

    #[test]
    fn docgen_selects_real_project_fixtures_instead_of_silent_bad_files() {
        let corpus =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/falcon_rules/tests/corpus");
        for rule in [
            "always-use-package-imports",
            "implementation-imports",
            "prefer-relative-imports",
            "avoid-renaming-method-parameters",
            "prefer-correct-test-file-name",
            "public-member-api-docs",
        ] {
            let rule_dir = corpus.join(rule);
            let bad_source = fs::read_to_string(rule_dir.join("bad.dart")).ok();
            let directory = project_fixture_directory(&rule_dir, bad_source.as_deref());
            assert!(directory.is_dir(), "{rule} must have project examples");
            assert!(
                recursive_files(&directory).iter().any(|file| {
                    file.extension().is_some_and(|extension| {
                        extension
                            .to_str()
                            .is_some_and(|extension| extension.eq_ignore_ascii_case("dart"))
                    }) && !crate::corpus::parse_expectations(&fs::read_to_string(file).unwrap())
                        .is_empty()
                }),
                "{rule} project fixtures must contain expected diagnostics"
            );
        }
    }

    #[test]
    fn docgen_keeps_positive_bad_fixture_when_project_only_has_support_files() {
        let corpus =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../crates/falcon_rules/tests/corpus");
        for rule in [
            "avoid-non-null-assertion",
            "avoid-duplicate-bloc-event-handlers",
            "check-is-not-closed-after-async-gap",
            "emit-new-bloc-state-instances",
        ] {
            let rule_dir = corpus.join(rule);
            let source = fs::read_to_string(rule_dir.join("bad.dart")).unwrap();
            assert!(
                !crate::corpus::parse_expectations(&source).is_empty(),
                "{rule}"
            );
            assert!(rule_dir.join("project").is_dir(), "{rule}");
            assert!(
                !project_fixture_directory(&rule_dir, Some(&source)).is_dir(),
                "{rule}"
            );
        }
    }

    #[test]
    fn build_context_rule_has_a_description_before_its_provenance() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        let (description, _) = extract_docs(
            &root.join(
                "crates/falcon_rules/src/lint/correctness/use_build_context_synchronously.rs",
            ),
            "use-build-context-synchronously",
        );
        assert_eq!(
            description,
            "Flags `BuildContext` use after an asynchronous gap without a mounted guard."
        );
    }

    #[test]
    fn docgen_check_accepts_current_data_without_writing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rules.json");
        fs::write(&path, "[]\n").unwrap();
        write_or_check(&path, "[]", true).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "[]\n");
    }

    #[test]
    fn docgen_check_rejects_drift_and_missing_files_without_writing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("rules.json");
        assert!(write_or_check(&path, "[]", true).is_err());
        assert!(!path.exists());
        fs::write(&path, "stale").unwrap();
        let error = write_or_check(&path, "[]", true).unwrap_err();
        assert!(error.contains("out of date"), "{error}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "stale");
    }

    #[test]
    fn docgen_write_failure_is_an_error() {
        let directory = tempfile::tempdir().unwrap();
        assert!(
            write_or_check(directory.path(), "[]", false)
                .unwrap_err()
                .contains("cannot write")
        );
    }

    #[test]
    fn docgen_rejects_empty_descriptions_and_zero_diagnostic_bad_fixtures() {
        assert!(
            validate_record("example", " \n", false, 0)
                .unwrap_err()
                .contains("empty rule description")
        );
        assert!(
            validate_record("example", "Description.", true, 0)
                .unwrap_err()
                .contains("zero diagnostics")
        );
        assert!(validate_record("example", "Description.", true, 1).is_ok());
        assert!(validate_record("example", "Description.", false, 0).is_ok());
    }

    #[test]
    fn docgen_columns_count_characters_not_utf16_units() {
        assert_eq!(super::character_column("x😀é target\n", 1, 6), 5);
        assert_eq!(super::character_column("first\n😀 target\n", 2, 4), 3);
        assert_eq!(super::character_column("plain target\n", 1, 7), 7);
        assert_eq!(
            super::character_column("/* expect: rule */ // ignore: unused_import", 1, 20),
            2
        );
        assert_eq!(
            super::character_column("é😀 /* expect: 😀 */ x /* expect: rule */ target", 1, 43),
            8
        );
        assert_eq!(
            super::character_column("x /* expect: rule */ target", 1, 7),
            3
        );
        assert_eq!(super::character_column("x /* expect: rule", 1, 18), 2);
        assert_eq!(
            super::character_column("target /* expect: rule */  ", 1, 27),
            7
        );
    }
    fn run(input: &[&str]) -> Vec<String> {
        let mut lines: Vec<String> = input.iter().map(ToString::to_string).collect();
        strip_provenance(&mut lines);
        lines
    }
    #[test]
    fn cuts_a_provenance_sentence_that_wraps_onto_the_next_line() {
        assert_eq!(
            run(&[
                "those have no literal form. Adopted from package:lints",
                "`prefer_collection_literals`.",
                "",
                "Two further facts."
            ]),
            vec!["those have no literal form.", "", "Two further facts."]
        );
    }

    #[test]
    fn keeps_content_following_the_closing_period() {
        assert_eq!(
            run(&[
                "intro. Adopted from package:lints",
                "`prefer_is_empty`. Only a plain access matches."
            ]),
            vec!["intro.", "Only a plain access matches."]
        );
    }

    #[test]
    fn keeps_description_after_provenance_on_the_same_line() {
        assert_eq!(
            run(&[
                "Summary. Adopted from the official `example` lint. Only plain accesses",
                "match; calls do not.",
            ]),
            vec!["Summary. Only plain accesses", "match; calls do not."]
        );
        assert_eq!(
            run(&[
                "Adopted from the official `example` lint. Only plain accesses",
                "match; calls do not.",
            ]),
            vec!["Only plain accesses", "match; calls do not."]
        );
    }

    #[test]
    fn removing_standalone_provenance_does_not_add_a_paragraph_break() {
        assert_eq!(
            run(&[
                "Summary.",
                "",
                "Adopted from the official",
                "`example` lint. Only plain accesses",
                "match; calls do not.",
            ]),
            vec![
                "Summary.",
                "",
                "Only plain accesses",
                "match; calls do not."
            ]
        );
        assert_eq!(
            run(&[
                "Summary.",
                "",
                "Adopted from the official `example` lint.",
                "",
                "Description.",
            ]),
            vec!["Summary.", "", "Description."]
        );
    }

    #[test]
    fn leaves_a_single_line_sentence_and_unrelated_text_alone() {
        assert_eq!(
            run(&[
                "Flags a thing. Adopted from package:lints `prefer_iterable_whereType`.",
                "Body stays."
            ]),
            vec!["Flags a thing.", "Body stays."]
        );
    }

    #[test]
    fn a_period_inside_an_identifier_is_not_a_sentence_boundary() {
        assert_eq!(
            run(&[
                "intro. Adopted from package:lints",
                "`List.filled` and `prefer_collection_literals`. Body stays."
            ]),
            vec!["intro.", "Body stays."]
        );
    }

    #[test]
    fn stops_at_a_paragraph_break_rather_than_eating_the_next_block() {
        assert_eq!(
            run(&["intro. Ported from dart_code_linter", "", "Next paragraph."]),
            vec!["intro.", "", "Next paragraph."]
        );
    }
}
