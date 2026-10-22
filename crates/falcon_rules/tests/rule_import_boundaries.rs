//! Production rule modules must depend on concepts, never on other rules.
//!
//! Every Rust file in a lint group must be registered as a rule. Importing a
//! rule-only namespace (including by alias or glob) is forbidden. This source
//! scan sees macro bodies; trailing unit-test modules are excluded.

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use regex::Regex;

const GROUPS: &[&str] = &[
    "a11y",
    "complexity",
    "correctness",
    "performance",
    "security",
    "style",
    "suspicious",
];

fn paths(tokens: &[&str], cursor: &mut usize, prefix: &[String], found: &mut Vec<Vec<String>>) {
    loop {
        let mut path = prefix.to_vec();
        while *cursor < tokens.len() {
            let token = tokens[*cursor];
            *cursor += 1;
            match token {
                "::" => continue,
                "{" => {
                    paths(tokens, cursor, &path, found);
                    break;
                }
                "," | ";" | "}" => {
                    if path.len() > prefix.len() {
                        found.push(path);
                    }
                    if token == "}" || token == ";" {
                        return;
                    }
                    break;
                }
                _ => path.push(token.to_owned()),
            }
            if tokens.get(*cursor) != Some(&"::") {
                if tokens.get(*cursor) == Some(&"as") {
                    *cursor += 1;
                    *cursor += usize::from(tokens.get(*cursor).is_some());
                }
                found.push(path);
                break;
            }
        }
        match tokens.get(*cursor) {
            Some(&",") => *cursor += 1,
            Some(&"}") => {
                *cursor += 1;
                return;
            }
            _ => return,
        }
    }
}

fn references(source: &str) -> Vec<Vec<String>> {
    let token = Regex::new(r"[A-Za-z_][A-Za-z0-9_]*|::|[{},;*]").unwrap();
    // Like the existing bespoke-walker ratchet, this is textual rather than a
    // Rust parser. Keep comments and literals conservative; they cannot hide a
    // production dependency. Standard trailing unit-test modules are excluded.
    let test_module =
        Regex::new(r"(?m)^#\[cfg\(test\)\]\s*mod\s+[A-Za-z_][A-Za-z0-9_]*\s*\{").unwrap();
    let production = test_module
        .find(source)
        .map_or(source, |module| &source[..module.start()]);
    let tokens: Vec<_> = token.find_iter(production).map(|m| m.as_str()).collect();
    let mut found = Vec::new();
    for (index, root) in tokens.iter().enumerate() {
        if *root == "use" {
            paths(&tokens, &mut (index + 1), &[], &mut found);
        } else if matches!(*root, "crate" | "super" | "self")
            && tokens.get(index + 1) == Some(&"::")
        {
            paths(&tokens, &mut (index + 1), &[(*root).to_owned()], &mut found);
        }
    }
    found
        .into_iter()
        .filter(|path| {
            path.first()
                .is_some_and(|root| matches!(root.as_str(), "crate" | "super" | "self"))
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn resolved(path: &[String], module: &[String]) -> Vec<String> {
    let mut result = module.to_vec();
    for part in path {
        match part.as_str() {
            "crate" => result.clear(),
            "super" => {
                result.pop();
            }
            "self" => {}
            _ => result.push(part.clone()),
        }
    }
    result
}

fn violations(src: &Path) -> BTreeSet<String> {
    let mut rules = BTreeSet::new();
    let mut sources = Vec::new();
    let mut violations = BTreeSet::new();
    let registration =
        Regex::new(r"(?m)^\s*([A-Za-z_][A-Za-z0-9_]*)::[A-Za-z_][A-Za-z0-9_]*\s*=>").unwrap();
    for dir in GROUPS
        .iter()
        .map(|group| src.join("lint").join(group))
        .chain([src.join("cross_file")])
    {
        let registered = (!dir.ends_with("cross_file")).then(|| {
            let declarations = fs::read_to_string(dir.with_extension("rs"))
                .expect("readable rule group declarations");
            registration
                .captures_iter(&declarations)
                .map(|entry| entry[1].to_owned())
                .collect::<BTreeSet<_>>()
        });
        for file in support::files(&dir, usize::MAX, Some("rs")) {
            let source = fs::read_to_string(&file).expect("readable rule source");
            // Group registration is authoritative, not impl-like text in a file.
            if let Some(registered) = &registered {
                let name = file.strip_prefix(&dir).unwrap().with_extension("");
                if !registered.contains(name.to_str().unwrap()) {
                    violations.insert(format!(
                        "{} is not a rule file",
                        file.strip_prefix(src).unwrap().display()
                    ));
                }
            } else if !source.contains("impl Rule for ")
                && !source.contains("impl CrossFileRule for ")
            {
                // Preserve the existing cross-file test-helper exclusion.
                continue;
            }
            let relative = file.strip_prefix(src).unwrap().with_extension("");
            let module: Vec<String> = relative
                .iter()
                .map(|part| part.to_string_lossy().into_owned())
                .collect();
            rules.insert(module.clone());
            sources.push((file, module, source));
        }
    }
    for (file, module, source) in sources {
        for path in references(&source) {
            let mut target = resolved(&path, &module);
            if target.last().is_some_and(|part| part == "*") {
                target.pop();
            }
            for rule in &rules {
                if rule != &module && target.starts_with(rule) {
                    violations.insert(format!(
                        "{} -> {}",
                        file.strip_prefix(src).unwrap().display(),
                        rule.join("::")
                    ));
                }
                if target != module && rule.starts_with(&target) && target.len() < rule.len() {
                    violations.insert(format!(
                        "{} -> {} (rule namespace)",
                        file.strip_prefix(src).unwrap().display(),
                        target.join("::")
                    ));
                }
            }
        }
    }
    violations
}

#[test]
fn production_rules_do_not_import_other_rules() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let violations = violations(&src);
    assert!(
        violations.is_empty(),
        "Move shared behavior into a concept helper:\n{}",
        violations.into_iter().collect::<Vec<_>>().join("\n")
    );
}

#[test]
fn source_scan_covers_grouped_imports_and_macro_paths() {
    let module = vec!["lint".into(), "style".into(), "consumer".into()];
    let paths: Vec<_> = references("use super::{first::helper, second::{a, b}}; macro_call! { crate::lint::style::third::helper(); }")
        .iter().map(|path| resolved(path, &module).join("::")).collect();
    for expected in [
        "lint::style::first::helper",
        "lint::style::second::a",
        "lint::style::second::b",
        "lint::style::third::helper",
    ] {
        assert!(
            paths.iter().any(|path| path == expected),
            "missing {expected}: {paths:?}"
        );
    }
}

#[test]
fn test_only_import_does_not_hide_later_production_dependencies() {
    let source = "#[cfg(test)]\nuse std::path::Path;\nuse super::other_rule::helper;\n#[cfg(test)]\nmod tests {\nuse super::test_rule::TestRule;\n}\n";
    let paths = references(source);
    assert!(
        paths
            .iter()
            .any(|path| path == &["super", "other_rule", "helper"])
    );
    assert!(
        !paths
            .iter()
            .any(|path| path.iter().any(|part| part == "test_rule"))
    );
}

#[test]
fn group_files_cannot_hide_dependencies_without_rule_impls() {
    let root = tempfile::tempdir().unwrap();
    for group in GROUPS {
        fs::create_dir_all(root.path().join("lint").join(group)).unwrap();
        fs::write(
            root.path().join("lint").join(group).with_extension("rs"),
            "crate::declare_rules! {\nGroup;\nconsumer::Consumer => ();\n}",
        )
        .unwrap();
    }
    fs::create_dir(root.path().join("cross_file")).unwrap();
    let group = root.path().join("lint/style");
    for source in [
        "pub fn helper() {}",
        "// impl Rule for Fake {}",
        r#"const EXAMPLE: &str = "impl Rule for Fake {}";"#,
        "#[cfg(test)]\nmod tests { impl Rule for Fake {} }",
    ] {
        fs::write(group.join("other.rs"), source).unwrap();
        assert_eq!(
            violations(root.path()),
            BTreeSet::from(["lint/style/other.rs is not a rule file".to_owned()])
        );
    }
    for source in [
        "use super::other::helper;",
        "use crate::lint::style as rules; fn check() { rules::other::helper(); }",
        "use crate::lint::{style as rules}; fn check() { rules::other::helper(); }",
        "use crate::lint::style::{self as rules}; fn check() { rules::other::helper(); }",
        "use crate::lint::style; fn check() { style::other::helper(); }",
        "use super::*; fn check() { other::helper(); }",
        "use super::{*}; fn check() { other::helper(); }",
        "use super as rules; fn check() { rules::other::helper(); }",
        "use crate::lint::style as group; use group as rules; rules::other::helper();",
        "use group as rules; use crate::lint::style as group; rules::other::helper();",
        "use crate::lint::style as rules; macro_call! { rules::other::helper(); }",
        "fn a() { use crate::lint::style as r; r::other::helper(); } fn b() { use crate::lint::support as r; }",
        "use super::*; use other as o; fn check() { o::helper(); }",
        "use super::*; fn check() { self::other::helper(); }",
        "use super::*; use falcon_syntax::cst::*; use std::collections::*; other::helper();",
    ] {
        fs::write(group.join("consumer.rs"), source).unwrap();
        assert_eq!(
            violations(root.path()),
            BTreeSet::from([
                if source.starts_with("use super::other") {
                    "lint/style/consumer.rs -> lint::style::other".to_owned()
                } else {
                    "lint/style/consumer.rs -> lint::style (rule namespace)".to_owned()
                },
                "lint/style/other.rs is not a rule file".to_owned(),
            ]),
            "{source}"
        );
    }
    // Registered rules are accepted regardless of impl formatting or qualification.
    fs::write(
        group.with_extension("rs"),
        "crate::declare_rules! {\nGroup;\nconsumer::Consumer => ();\nother::Other => ();\n}",
    )
    .unwrap();
    fs::write(
        group.join("other.rs"),
        "impl<'a> falcon_analyze::Rule for Other<'a> {}",
    )
    .unwrap();
    assert_eq!(
        violations(root.path()),
        BTreeSet::from(["lint/style/consumer.rs -> lint::style (rule namespace)".to_owned()])
    );
}

#[test]
fn group_aliases_and_globs_preserve_allowed_dependencies() {
    let root = tempfile::tempdir().unwrap();
    for group in GROUPS {
        fs::create_dir_all(root.path().join("lint").join(group)).unwrap();
        fs::write(
            root.path().join("lint").join(group).with_extension("rs"),
            "crate::declare_rules! {\nGroup;\nconsumer::Consumer => ();\nother::Other => ();\n}",
        )
        .unwrap();
    }
    fs::create_dir(root.path().join("cross_file")).unwrap();
    fs::write(
        root.path().join("cross_file/test_helpers.rs"),
        "use crate::lint::style::other::helper;",
    )
    .unwrap();
    for source in [
        "use crate::lint::style::consumer; fn check() { consumer::helper(); }",
        "use self::helper; fn check() { helper(); }",
        "use crate::lint::support as helpers; fn check() { helpers::other::helper(); }",
        "use super::{\n helper,\n};",
        "use crate::lint::support::{\n a,\n b,\n};",
        "use crate::{\n meta::RuleSource,\n};",
        "#[cfg(test)]\nmod tests { use super::*; other::helper(); }",
    ] {
        fs::write(root.path().join("lint/style/consumer.rs"), source).unwrap();
        fs::write(
            root.path().join("lint/style/other.rs"),
            "pub fn helper() {}",
        )
        .unwrap();
        assert!(violations(root.path()).is_empty(), "{source}");
    }
}

#[test]
fn external_globs_do_not_compound_relative_globs() {
    let paths = references("use super::*; use falcon_syntax::cst::*; use std::collections::*;");
    assert_eq!(paths, vec![vec!["super".to_owned(), "*".to_owned()]]);
}
