use crate::workspace_root;
use falcon_rules::meta::{RuleGroup, meta_for};
use std::{fs, path::Path};

const FILE_RULE_TEMPLATE: &str =
    include_str!("../../crates/falcon_rules/tests/scaffolds/file_rule.rs");
const FILE_RULE_NAMES: &[&str] = &[
    "AnalyzeContext",
    "Rule",
    "RulePreparation",
    "StructuralRuleVisitor",
    "StructuralTraversal",
    "Diagnostic",
    "Expr",
    "Program",
    "Collector",
    "Default",
    "Box",
    "Vec",
];
const CROSS_FILE_TEMPLATE: &str =
    include_str!("../../crates/falcon_rules/tests/scaffolds/cross_file_rule.rs");
const CROSS_FILE_NAMES: &[&str] = &[
    "CrossFileRule",
    "ProjectFile",
    "SemanticIndexes",
    "FalconConfig",
    "Diagnostic",
    "Default",
    "Option",
    "Vec",
];
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "abstract", "become", "box", "do", "final", "gen", "macro", "override",
    "priv", "try", "typeof", "unsized", "virtual", "yield",
];

pub(crate) fn validate_rule_id(value: &str) -> Result<String, String> {
    if value.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
        && value.split('-').all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
        })
    {
        Ok(value.to_owned())
    } else {
        Err("rule id must be kebab-case: start with a-z, then use lowercase ASCII letters, digits, and single hyphens".into())
    }
}

/// Convert `snake_case` to `PascalCase` for the rule struct name.
fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// Render the Rule impl stub for a new rule.
fn rule_stub(struct_name: &str) -> String {
    FILE_RULE_TEMPLATE.replace("ScaffoldRule", struct_name)
}

fn cross_file_stub(struct_name: &str) -> String {
    CROSS_FILE_TEMPLATE.replace("ScaffoldRule", struct_name)
}

/// Generate a rule implementation stub plus corpus fixture skeletons.
///
/// `cargo xtask codegen rule --group suspicious --name avoid-foo`
///
/// The validated kebab-case id maps to a snake_case module under either
/// `lint/<group>/` or `cross_file/`.
pub(crate) fn run(group: String, rule_id: String, cross_file: bool) -> Result<(), String> {
    generate(&workspace_root(), &group, &rule_id, cross_file)
}

fn generate(root: &Path, group: &str, rule_id: &str, cross_file: bool) -> Result<(), String> {
    validate_rule_id(rule_id)?;
    if falcon_diagnostics::RESERVED_DIAGNOSTIC_IDS.contains(&rule_id) {
        return Err(format!(
            "rule id `{rule_id}` is reserved for internal diagnostics"
        ));
    }
    if meta_for(rule_id).is_some() {
        return Err(format!("rule id `{rule_id}` is already registered"));
    }
    let module = rule_id.replace('-', "_");
    if RUST_KEYWORDS.contains(&module.as_str()) {
        return Err(format!(
            "rule id `{rule_id}` maps to a reserved Rust keyword"
        ));
    }
    let struct_name = pascal_case(&module);
    let template_names = if cross_file {
        CROSS_FILE_NAMES
    } else {
        FILE_RULE_NAMES
    };
    if template_names.contains(&struct_name.as_str()) {
        return Err(format!(
            "rule id `{rule_id}` conflicts with a generated template name"
        ));
    }
    let directory = if cross_file {
        "crates/falcon_rules/src/cross_file".to_owned()
    } else {
        format!("crates/falcon_rules/src/lint/{group}")
    };
    let rule_file = root.join(directory).join(format!("{module}.rs"));
    let corpus_dir = root.join(format!("crates/falcon_rules/tests/corpus/{}", rule_id));

    if rule_file.exists() {
        return Err(format!("{} already exists", rule_file.display()));
    }

    fs::create_dir_all(rule_file.parent().expect("rule directory")).expect("create rule directory");
    let stub = if cross_file {
        cross_file_stub(&struct_name)
    } else {
        rule_stub(&struct_name)
    };
    fs::write(&rule_file, stub).expect("write rule stub");

    let fixtures: &[(&str, &str)] = if cross_file {
        &[
            (
                "cross-file/bad/main.dart",
                "import 'support.dart';\n\n// Placeholder: violation examples with expect annotations will be added\n",
            ),
            (
                "cross-file/bad/support.dart",
                "// Placeholder fixture: filled in when the rule is implemented.\n",
            ),
            (
                "cross-file/good/main.dart",
                "// Placeholder: compliant code, no annotations.\n",
            ),
        ]
    } else {
        &[
            (
                "bad.dart",
                "// Placeholder: violation examples with expect annotations will be added\n",
            ),
            (
                "good.dart",
                "// Placeholder: compliant code, no annotations.\n",
            ),
        ]
    };
    println!("generated:");
    println!("  {}", rule_file.display());
    for &(name, content) in fixtures {
        let fixture = corpus_dir.join(name);
        fs::create_dir_all(fixture.parent().expect("fixture directory"))
            .expect("create corpus directory");
        if !fixture.exists() {
            fs::write(&fixture, content).expect("write corpus fixture");
        }
        println!("  {}", fixture.display());
    }
    println!();
    print_next_steps(group, rule_id, &module, &struct_name, cross_file);
    Ok(())
}

fn print_next_steps(group: &str, rule_id: &str, module: &str, struct_name: &str, cross_file: bool) {
    println!("next steps:");
    let declaration_file = if cross_file {
        "crates/falcon_rules/src/cross_file.rs".to_owned()
    } else {
        format!("crates/falcon_rules/src/lint/{group}.rs")
    };
    let category = RuleGroup::ALL
        .iter()
        .find(|category| category.as_str() == group)
        .unwrap();
    println!("  1. add this entry inside crate::declare_rules! in {declaration_file}:");
    if cross_file {
        println!(
            "     {module}::{struct_name} => (\"{rule_id}\", cross_file, {category:?}, false, RuleSource::Falcon);"
        );
    } else {
        println!(
            "     {module}::{struct_name} => (\"{rule_id}\", file, None, false, RuleSource::Falcon);"
        );
    }
    println!(
        "     choose resolved/resolved_cross_file if resolution is required; set recommended and source (and domain for file rules)"
    );
    let section = if cross_file {
        "\"cross-file\""
    } else {
        "linter"
    };
    println!("  2. add `{rule_id}` under {section}.rules.{group} in the root falcon.json showcase");
    println!(
        "  3. implement the rule and replace all fixture placeholders with positive and negative cases"
    );
    if cross_file {
        println!(
            "     include genuine multi-file cases in the cross-file/bad/ and cross-file/good/ fixture projects"
        );
    }
    println!(
        "  4. run `cargo xtask validate-rules --rule {rule_id}`, `makers test`, and `makers validate_rules`"
    );
    println!(
        "  5. run `cargo xtask schema` and `cargo xtask docgen`; commit the generated outputs"
    );
    println!(
        "  6. add a user-facing changeset with `makers changeset`, then run the CONTRIBUTING gates"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_ids_require_canonical_kebab_case() {
        for valid in ["avoid-foo", "rule", "avoid-2d-values"] {
            assert_eq!(validate_rule_id(valid).unwrap(), valid);
        }
        for invalid in [
            "", "-foo", "foo-", "foo--bar", "Foo", "foo_bar", "foo/bar", "../foo", "foo.bar",
            "foo bar", "1foo", "fóo",
        ] {
            assert!(validate_rule_id(invalid).is_err(), "{invalid:?}");
        }
    }

    #[test]
    fn generated_fixtures_are_rejected_as_placeholders() {
        for cross_file in [false, true] {
            let directory = tempfile::tempdir().unwrap();
            generate(
                directory.path(),
                "suspicious",
                "avoid-scaffold-foo",
                cross_file,
            )
            .unwrap();
            let corpus = directory
                .path()
                .join("crates/falcon_rules/tests/corpus/avoid-scaffold-foo");
            let fixtures = if cross_file {
                assert!(!corpus.join("bad.dart").exists());
                assert!(!corpus.join("good.dart").exists());
                assert_eq!(
                    crate::corpus::cross_file_project_roots(&corpus.join("cross-file")),
                    [
                        corpus.join("cross-file/bad"),
                        corpus.join("cross-file/good")
                    ]
                );
                vec![
                    corpus.join("cross-file/bad/main.dart"),
                    corpus.join("cross-file/bad/support.dart"),
                    corpus.join("cross-file/good/main.dart"),
                ]
            } else {
                vec![corpus.join("bad.dart"), corpus.join("good.dart")]
            };
            for fixture in fixtures {
                let source = fs::read_to_string(&fixture).unwrap();
                assert!(
                    crate::corpus::contains_placeholder_marker(&source),
                    "{}",
                    fixture.display()
                );
            }
        }
    }

    #[test]
    fn registered_and_aliased_ids_are_rejected_before_writes() {
        let alias = falcon_rules::meta::RULE_ALIASES[0].0;
        for name in ["no-empty-block", alias] {
            for cross_file in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let error = generate(directory.path(), "style", name, cross_file).unwrap_err();
                assert!(error.contains("already registered"), "{error}");
                assert!(fs::read_dir(directory.path()).unwrap().next().is_none());
            }
        }
    }

    #[test]
    fn reserved_and_template_names_are_rejected_before_writes() {
        for cross_file in [false, true] {
            let template_names = if cross_file {
                CROSS_FILE_NAMES
            } else {
                FILE_RULE_NAMES
            };
            let collisions = template_names.iter().map(|name| {
                let mut id = String::with_capacity(name.len() * 2);
                for (index, character) in name.chars().enumerate() {
                    if index > 0 && character.is_ascii_uppercase() {
                        id.push('-');
                    }
                    id.push(character.to_ascii_lowercase());
                }
                id
            });
            for name in RUST_KEYWORDS
                .iter()
                .copied()
                .chain(falcon_diagnostics::RESERVED_DIAGNOSTIC_IDS.iter().copied())
                .map(str::to_owned)
                .chain(collisions)
            {
                let directory = tempfile::tempdir().unwrap();
                assert!(
                    generate(directory.path(), "style", &name, cross_file).is_err(),
                    "{name}"
                );
                assert!(
                    fs::read_dir(directory.path()).unwrap().next().is_none(),
                    "{name}"
                );
            }
        }
    }
}
