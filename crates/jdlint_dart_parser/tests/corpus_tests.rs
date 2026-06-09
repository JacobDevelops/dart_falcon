/// Corpus tests: parse every .dart file in the jfit project and assert no panics.
///
/// The jfit corpus path is resolved via the `JFIT_PATH` environment variable
/// first, then falls back to the known local development path. The tests are
/// silently skipped when the corpus is not accessible (e.g. in CI without the
/// jfit repo checked out).
use jdlint_dart_parser::parser::parse;
use std::path::{Path, PathBuf};

fn corpus_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("JFIT_PATH") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let local = PathBuf::from("/home/jacob/Documents/Developer/jfit");
    if local.exists() {
        return Some(local);
    }
    None
}

/// Directories that are not project-owned Dart source — skip them entirely.
const SKIP_DIRS: &[&str] = &[
    ".direnv",   // nix/direnv cached inputs
    ".dart_tool",
    ".pub-cache",
    "build",
    ".flutter-plugins",
    ".flutter-plugins-dependencies",
];

fn is_skip_dir(name: &str) -> bool {
    SKIP_DIRS.iter().any(|s| name == *s)
        || name.starts_with("result")  // nix result, result-1, result-11, …
}

fn collect_dart_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !is_skip_dir(dir_name) {
                files.extend(collect_dart_files(&path));
            }
        } else if path.extension().is_some_and(|e| e == "dart") {
            files.push(path);
        }
    }
    files
}

#[test]
fn corpus_parses_without_panic() {
    let Some(root) = corpus_root() else {
        eprintln!("jfit corpus not found — skipping corpus_parses_without_panic");
        return;
    };

    // Target: jfit mobile app lib — the primary corpus per the M1 plan.
    let mobile_lib = root.join("apps/mobile/lib");
    let search_root = if mobile_lib.exists() { mobile_lib } else { root.clone() };

    let files = collect_dart_files(&search_root);
    assert!(
        !files.is_empty(),
        "corpus root exists but contains no .dart files: {}",
        search_root.display()
    );

    let total = files.len();
    let mut error_files: Vec<(PathBuf, usize)> = Vec::new();

    for path in &files {
        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("could not read {}: {e}", path.display());
                continue;
            }
        };
        // parse() must not panic — if it does the test aborts with a clear message.
        let (_, errors) = parse(&source);
        if !errors.is_empty() {
            error_files.push((path.clone(), errors.len()));
        }
    }

    let error_count = error_files.len();

    // Report files with parse errors (informational — not a hard failure yet).
    if !error_files.is_empty() {
        eprintln!(
            "\nParse errors in {error_count}/{total} corpus files (informational):"
        );
        for (path, n) in &error_files {
            eprintln!("  {n} error(s): {}", path.display());
        }
    }

    println!("Corpus: {total} files parsed, {error_count} had parse errors, {} clean", total - error_count);

    // Hard limit: at most 20 % of files may have parse errors.
    // This threshold will tighten as the parser matures.
    let allowed = total / 5;
    assert!(
        error_count <= allowed,
        "Too many corpus files with parse errors: {error_count}/{total} \
         (limit {allowed}). Run with RUST_LOG=debug for details."
    );
}

#[test]
fn corpus_no_panics_on_generated_files() {
    let Some(root) = corpus_root() else {
        eprintln!("jfit corpus not found — skipping corpus_no_panics_on_generated_files");
        return;
    };

    // Focus on the mobile app lib directory which is the primary target.
    let mobile_lib = root.join("apps/mobile/lib");
    let search_root = if mobile_lib.exists() { mobile_lib } else { root };

    let files = collect_dart_files(&search_root);
    let total = files.len();

    if total == 0 {
        eprintln!("no .dart files found under {}", search_root.display());
        return;
    }

    // This test's sole assertion is that parse() never panics.
    // Any panic would abort the test process with a clear stack trace.
    for path in &files {
        let Ok(source) = std::fs::read_to_string(path) else {
            continue;
        };
        let _ = parse(&source);
    }

    println!("no-panic check: parsed {total} files under {}", search_root.display());
}

#[test]
fn diag_show_errors() {
    let path = "/home/jacob/Documents/Developer/jfit/apps/mobile/lib/firebase_options.dart";
    let source = std::fs::read_to_string(path).unwrap_or_default();
    if source.is_empty() { return; }
    let lines: Vec<&str> = source.lines().collect();
    let (_, errors) = parse(&source);
    for e in errors.iter().take(15) {
        let before = &source[..e.offset.min(source.len())];
        let line_no = before.lines().count();
        let line_text = lines.get(line_no.saturating_sub(1)).unwrap_or(&"");
        eprintln!("L{line_no}: {} | snippet: {}", e.message, &line_text.trim()[..line_text.trim().len().min(60)]);
    }
}

#[test]
fn diag_error_distribution() {
    let Some(root) = corpus_root() else { return; };
    let mobile_lib = root.join("apps/mobile/lib");
    let search_root = if mobile_lib.exists() { mobile_lib } else { root };
    let files = collect_dart_files(&search_root);
    let mut msg_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for path in &files {
        let Ok(source) = std::fs::read_to_string(path) else { continue; };
        let (_, errors) = parse(&source);
        for e in &errors {
            // Normalize message to first 60 chars
            let key = e.message.chars().take(60).collect::<String>();
            *msg_counts.entry(key).or_insert(0) += 1;
        }
    }
    let mut sorted: Vec<_> = msg_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1));
    eprintln!("\nTop 15 parse error messages:");
    for (msg, count) in sorted.iter().take(15) {
        eprintln!("  {count:4}x  {msg}");
    }
}

#[test]
fn diag_onboarding_errors() {
    let path = "/home/jacob/Documents/Developer/jfit/apps/mobile/lib/features/onboarding/data/onboarding_repository.dart";
    let source = std::fs::read_to_string(path).unwrap_or_default();
    if source.is_empty() { return; }
    let lines: Vec<&str> = source.lines().collect();
    let (_, errors) = parse(&source);
    for e in &errors {
        let before = &source[..e.offset.min(source.len())];
        let line_no = before.lines().count();
        let line_text = lines.get(line_no.saturating_sub(1)).unwrap_or(&"");
        eprintln!("L{line_no}: {} | {}", e.message, line_text.trim());
    }
}

#[test]
fn diag_app_providers_errors() {
    let path = "/home/jacob/Documents/Developer/jfit/apps/mobile/lib/core/app_providers.dart";
    let source = std::fs::read_to_string(path).unwrap_or_default();
    if source.is_empty() { return; }
    let lines: Vec<&str> = source.lines().collect();
    let (_, errors) = parse(&source);
    for e in &errors {
        let before = &source[..e.offset.min(source.len())];
        let line_no = before.lines().count();
        let line_text = lines.get(line_no.saturating_sub(1)).unwrap_or(&"");
        eprintln!("L{line_no}: {} | {}", e.message, line_text.trim());
    }
}
