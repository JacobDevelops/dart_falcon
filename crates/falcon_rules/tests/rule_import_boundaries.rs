//! Production rule modules must depend on concepts, never on other rules.
//!
//! This deliberately conservative source scan also sees paths inside macro
//! bodies, unlike a Rust AST traversal without macro expansion. Unit-test
//! modules may import rules to exercise their public behavior.

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
                    found.push(path);
                    if token == "}" || token == ";" {
                        return;
                    }
                    break;
                }
                _ => path.push(token.to_owned()),
            }
            if tokens.get(*cursor) != Some(&"::") {
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
    let token = Regex::new(r"[A-Za-z_][A-Za-z0-9_]*|::|[{},;]").unwrap();
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
        if matches!(*root, "crate" | "super" | "self") && tokens.get(index + 1) == Some(&"::") {
            paths(&tokens, &mut (index + 1), &[(*root).to_owned()], &mut found);
        }
    }
    found
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

#[test]
fn production_rules_do_not_import_other_rules() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut rules = BTreeSet::new();
    let mut sources = Vec::new();
    for dir in GROUPS
        .iter()
        .map(|group| src.join("lint").join(group))
        .chain([src.join("cross_file")])
    {
        for file in support::files(&dir, usize::MAX, Some("rs")) {
            let source = fs::read_to_string(&file).expect("readable rule source");
            if !source.contains("impl Rule for ") && !source.contains("impl CrossFileRule for ") {
                continue;
            }
            let relative = file.strip_prefix(&src).unwrap().with_extension("");
            let module: Vec<String> = relative
                .iter()
                .map(|part| part.to_string_lossy().into_owned())
                .collect();
            rules.insert(module.clone());
            sources.push((file, module, source));
        }
    }
    let mut violations = BTreeSet::new();
    for (file, module, source) in sources {
        for path in references(&source) {
            let target = resolved(&path, &module);
            for rule in &rules {
                if rule != &module && target.starts_with(rule) {
                    violations.insert(format!(
                        "{} -> {}",
                        file.strip_prefix(&src).unwrap().display(),
                        rule.join("::")
                    ));
                }
            }
        }
    }
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
