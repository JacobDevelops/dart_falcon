use crate::workspace_root;
use falcon_rules::meta::{RuleGroup, meta_for};
use std::{fs, path::Path};

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
    include_str!("../../crates/falcon_rules/tests/scaffolds/file_rule.rs")
        .replace("ScaffoldRule", struct_name)
}

fn cross_file_stub(struct_name: &str) -> String {
    include_str!("../../crates/falcon_rules/tests/scaffolds/cross_file_rule.rs")
        .replace("ScaffoldRule", struct_name)
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
    if meta_for(rule_id).is_some() {
        return Err(format!("rule id `{rule_id}` is already registered"));
    }
    let module = rule_id.replace('-', "_");
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

    let struct_name = pascal_case(&module);
    fs::create_dir_all(rule_file.parent().expect("rule directory")).expect("create rule directory");
    let stub = if cross_file {
        cross_file_stub(&struct_name)
    } else {
        rule_stub(&struct_name)
    };
    fs::write(&rule_file, stub).expect("write rule stub");

    fs::create_dir_all(&corpus_dir).expect("create corpus dir");
    let bad = corpus_dir.join("bad.dart");
    let good = corpus_dir.join("good.dart");
    if !bad.exists() {
        fs::write(
            &bad,
            "// Placeholder: violation examples with expect annotations will be added\n",
        )
        .expect("write bad.dart");
    }
    if !good.exists() {
        fs::write(&good, "// Placeholder: compliant code, no annotations.\n")
            .expect("write good.dart");
    }

    println!("generated:");
    println!("  {}", rule_file.display());
    println!("  {}", bad.display());
    println!("  {}", good.display());
    if cross_file {
        let project_dir = corpus_dir.join("cross-file");
        fs::create_dir_all(&project_dir).expect("create project fixture directory");
        for (name, content) in [
            (
                "main.dart",
                "import 'support.dart';\n\n// Placeholder: violation examples with expect annotations will be added\n",
            ),
            (
                "support.dart",
                "// Placeholder fixture: filled in when the rule is implemented.\n",
            ),
        ] {
            let fixture = project_dir.join(name);
            if !fixture.exists() {
                fs::write(&fixture, content).expect("write project fixture");
            }
            println!("  {}", fixture.display());
        }
    }
    println!();
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
        println!("     include genuine multi-file cases in the cross-file/ fixture project");
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
    Ok(())
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
            let mut fixtures = vec![corpus.join("bad.dart"), corpus.join("good.dart")];
            if cross_file {
                fixtures.extend([
                    corpus.join("cross-file/main.dart"),
                    corpus.join("cross-file/support.dart"),
                ]);
            }
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
}
