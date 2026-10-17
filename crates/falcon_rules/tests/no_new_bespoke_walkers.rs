//! Ratchet against hand-rolled syntax-tree traversal in rules.
//!
//! Catch-all arms in bespoke walkers silently miss new syntax. Prefer the
//! exhaustive walker in `falcon_syntax::cst::visit` and its `for_each_expr` /
//! `bound_names` helpers.
//!
//! New walkers fail this ratchet; migrated walkers must leave the baseline.
//!
//! Run with `UPDATE_BESPOKE_WALKER_BASELINE=1` to rewrite the baseline after a
//! deliberate migration.

mod support;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const BASELINE: &str = "tests/bespoke_walkers.txt";

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn baseline_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(BASELINE)
}

/// The body of the block that starts at the first `{` at or after `from`.
fn block_body(src: &str, from: usize) -> Option<&str> {
    let bytes = src.as_bytes();
    let start = src[from..].find('{')? + from;
    let mut depth = 0usize;
    for (i, b) in bytes.iter().enumerate().skip(start) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..=i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// A hand-rolled walker: a self-recursive function that matches on syntax nodes and
/// has a catch-all arm. Recursive *predicates* (`is_literal`, `base_type_name`)
/// trip this too; that is deliberate — a false positive costs one baseline line,
/// whereas a missed walker costs a silent rule.
///
/// This is a textual scan, not a parse. `fn `, `Stmt::`, `Expr::` and braces are
/// matched wherever they appear, including inside comments and string literals,
/// and the recursion check is a substring match on the function name. So a
/// baseline entry may reflect a textual match rather than a real walker, and
/// prose alone can move the list. That is accepted: the ratchet only has to make
/// the set impossible to grow silently, and erring toward matching keeps a real
/// walker from slipping through.
fn walkers_in(src: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut cursor = 0usize;
    while let Some(rel) = src[cursor..].find("fn ") {
        let at = cursor + rel;
        cursor = at + 3;
        let rest = &src[cursor..];
        let name_len = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(0);
        if name_len == 0 {
            continue;
        }
        let name = &rest[..name_len];
        let Some(body) = block_body(src, cursor + name_len) else {
            continue;
        };
        let self_recursive = body.contains(&format!("{name}("));
        let walks_syntax = body.contains("Stmt::") || body.contains("Expr::");
        // A catch-all arm: `_` not starting a longer identifier, reaching a `=>`
        // on the same line — with or without a `_ if guard =>` in between.
        let has_catch_all = body.lines().any(|l| {
            matches!(l.trim_start().strip_prefix('_'), Some(r)
                if !r.starts_with(|c: char| c.is_alphanumeric() || c == '_')
                    && (r.contains("=>") || r.starts_with(" |")))
        });
        if self_recursive && walks_syntax && has_catch_all {
            found.push(name.to_string());
        }
    }
    found
}

fn current_set() -> BTreeSet<String> {
    let src = src_dir();
    let files = support::files(&src, usize::MAX, Some("rs"));
    let mut set = BTreeSet::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("readable rule source");
        let rel = file
            .strip_prefix(&src)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        for name in walkers_in(&text) {
            set.insert(format!("{rel}::{name}"));
        }
    }
    set
}

#[test]
fn no_new_hand_rolled_syntax_walkers() {
    let current = current_set();

    if std::env::var("UPDATE_BESPOKE_WALKER_BASELINE").is_ok() {
        let mut out = String::from(
            "# Hand-rolled syntax-tree walkers, pinned by tests/no_new_bespoke_walkers.rs.\n\
             # This list may only shrink. Build new rules on falcon_syntax::cst::visit.\n",
        );
        for entry in &current {
            out.push_str(entry);
            out.push('\n');
        }
        fs::write(baseline_path(), out).expect("baseline is writable");
        return;
    }

    let baseline: BTreeSet<String> = fs::read_to_string(baseline_path())
        .expect("baseline file exists")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

    let added: Vec<_> = current.difference(&baseline).collect();
    let removed: Vec<_> = baseline.difference(&current).collect();

    let mut problems = String::new();
    if !added.is_empty() {
        problems.push_str(
            "\nNew hand-rolled syntax-tree walker(s). These go blind on new Dart syntax —\n\
             build on falcon_syntax::cst::visit instead (walk_expr / for_each_expr /\n\
             bound_names), which is exhaustive:\n",
        );
        for entry in added {
            problems.push_str(&format!("  + {entry}\n"));
        }
    }
    if !removed.is_empty() {
        problems.push_str(
            "\nBaseline entries no longer present — migrated, so drop these lines\n\
             (re-run with UPDATE_BESPOKE_WALKER_BASELINE=1):\n",
        );
        for entry in removed {
            problems.push_str(&format!("  - {entry}\n"));
        }
    }

    assert!(problems.is_empty(), "{problems}");
}
