//! File walker: discovers `.dart` files recursively, applies glob exclude patterns.

use glob::Pattern;
use std::path::{Path, PathBuf};
use tracing::warn;
use walkdir::WalkDir;

/// Walk `paths` and return all non-excluded `.dart` files with their contents.
///
/// Directories are walked recursively. Invalid glob patterns, unreadable files,
/// broken symlinks, and permission errors are logged as warnings and skipped.
pub fn walk_files(paths: &[PathBuf], exclude_patterns: &[String]) -> Vec<(PathBuf, String)> {
    let compiled: Vec<Pattern> = exclude_patterns
        .iter()
        .filter_map(|p| match Pattern::new(p) {
            Ok(pat) => Some(pat),
            Err(_) => {
                warn!("invalid exclude pattern: {}", p);
                None
            }
        })
        .collect();

    let mut results = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if path.is_file() {
            if let Some(entry) = try_read_dart(path, &compiled) {
                results.push(entry);
            }
        } else if path.is_dir() {
            // Never follow symlinks: Flutter's ephemeral/.plugin_symlinks point
            // into the pub cache, turning a project walk into a multi-minute
            // crawl of every dependency (and risking cycles).
            for item in WalkDir::new(path).follow_links(false) {
                match item {
                    Ok(e) => {
                        if let Some(entry) = try_read_dart(e.path(), &compiled) {
                            results.push(entry);
                        }
                    }
                    Err(e) => warn!("error walking directory: {}", e),
                }
            }
        }
    }
    results
}

fn is_dart(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("dart")
}

fn is_excluded(path: &Path, patterns: &[Pattern]) -> bool {
    let s = path.to_string_lossy();
    patterns.iter().any(|p| p.matches(&s))
}

fn try_read_dart(path: &Path, patterns: &[Pattern]) -> Option<(PathBuf, String)> {
    if !is_dart(path) || is_excluded(path, patterns) {
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(contents) => Some((path.to_path_buf(), contents)),
        Err(_) => {
            warn!("failed to read file: {}", path.display());
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_walk_single_dart_file() {
        let temp = tempdir().unwrap();
        let dart_file = temp.path().join("test.dart");
        fs::write(&dart_file, "void main() {}").unwrap();

        let results = walk_files(std::slice::from_ref(&dart_file), &[]);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, dart_file);
        assert_eq!(results[0].1, "void main() {}");
    }

    #[test]
    fn test_walk_directory_finds_dart_files() {
        let temp = tempdir().unwrap();
        let dart1 = temp.path().join("file1.dart");
        let dart2 = temp.path().join("file2.dart");
        let dart3 = temp.path().join("file3.dart");
        let rust_file = temp.path().join("notdart.rs");

        fs::write(&dart1, "dart1").unwrap();
        fs::write(&dart2, "dart2").unwrap();
        fs::write(&dart3, "dart3").unwrap();
        fs::write(&rust_file, "rust").unwrap();

        let results = walk_files(&[temp.path().to_path_buf()], &[]);
        assert_eq!(results.len(), 3);

        let paths: Vec<_> = results.iter().map(|(p, _)| p).collect();
        assert!(paths.contains(&&dart1));
        assert!(paths.contains(&&dart2));
        assert!(paths.contains(&&dart3));
    }

    #[test]
    fn test_walk_empty_directory() {
        let temp = tempdir().unwrap();
        let results = walk_files(&[temp.path().to_path_buf()], &[]);
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_walk_exclude_pattern() {
        let temp = tempdir().unwrap();
        let lib_dir = temp.path().join("lib");
        let build_dir = temp.path().join("build");
        fs::create_dir(&lib_dir).unwrap();
        fs::create_dir(&build_dir).unwrap();

        let lib_main = lib_dir.join("main.dart");
        let build_output = build_dir.join("output.dart");

        fs::write(&lib_main, "lib").unwrap();
        fs::write(&build_output, "build").unwrap();

        let exclude_patterns = vec!["**/build/**".to_string()];
        let results = walk_files(&[temp.path().to_path_buf()], &exclude_patterns);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, lib_main);
    }

    #[test]
    fn test_walk_nested_directory() {
        let temp = tempdir().unwrap();
        let src_dir = temp.path().join("src");
        let a_dir = src_dir.join("a");
        fs::create_dir(&src_dir).unwrap();
        fs::create_dir(&a_dir).unwrap();

        let b_dart = a_dir.join("b.dart");
        let c_dart = src_dir.join("c.dart");

        fs::write(&b_dart, "nested").unwrap();
        fs::write(&c_dart, "shallow").unwrap();

        let results = walk_files(&[temp.path().to_path_buf()], &[]);
        assert_eq!(results.len(), 2);

        let paths: Vec<_> = results.iter().map(|(p, _)| p).collect();
        assert!(paths.contains(&&b_dart));
        assert!(paths.contains(&&c_dart));
    }

    #[test]
    fn test_walk_nonexistent_path_returns_empty() {
        let nonexistent = PathBuf::from("/nonexistent/path/to/file.dart");
        let results = walk_files(&[nonexistent], &[]);
        assert_eq!(results.len(), 0);
    }
}
