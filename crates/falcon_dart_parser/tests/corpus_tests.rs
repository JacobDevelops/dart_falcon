/// Corpus tests: parse every .dart file in the jfit project and assert no panics.
///
/// The jfit corpus path is resolved via the `JFIT_PATH` environment variable
/// first, then falls back to the known local development path. The tests are
/// silently skipped when the corpus is not accessible (e.g. in CI without the
/// jfit repo checked out).
use falcon_dart_parser::parser::parse;
use std::path::PathBuf;
#[path = "common/collect_dart_files.rs"]
mod common_files;
use common_files::collect_dart_files;

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
    ".direnv",
    ".dart_tool",
    ".pub-cache",
    "build",
    ".flutter-plugins",
    ".flutter-plugins-dependencies",
];

#[test]
fn corpus_parses_without_panic() {
    let Some(root) = corpus_root() else {
        eprintln!("jfit corpus not found — skipping corpus_parses_without_panic");
        return;
    };

    // Target: jfit mobile app lib — the primary corpus per the M1 plan.
    let mobile_lib = root.join("apps/mobile/lib");
    let search_root = if mobile_lib.exists() {
        mobile_lib
    } else {
        root.clone()
    };

    let files = collect_dart_files(&search_root, SKIP_DIRS);
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
        let falcon_dart_parser::ParseResult { errors, .. } = parse(&source);
        if !errors.is_empty() {
            error_files.push((path.clone(), errors.len()));
        }
    }

    let error_count = error_files.len();

    // Report files with parse errors (informational — not a hard failure yet).
    if !error_files.is_empty() {
        eprintln!("\nParse errors in {error_count}/{total} corpus files (informational):");
        for (path, n) in &error_files {
            eprintln!("  {n} error(s): {}", path.display());
        }
    }

    println!(
        "Corpus: {total} files parsed, {error_count} had parse errors, {} clean",
        total - error_count
    );

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
    let search_root = if mobile_lib.exists() {
        mobile_lib
    } else {
        root
    };

    let files = collect_dart_files(&search_root, SKIP_DIRS);
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

    println!(
        "no-panic check: parsed {total} files under {}",
        search_root.display()
    );
}
