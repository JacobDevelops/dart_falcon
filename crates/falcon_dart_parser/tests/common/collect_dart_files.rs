use std::path::{Path, PathBuf};

pub fn collect_dart_files(root: &Path, skip_dirs: &[&str]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return files;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if !skip_dirs.contains(&name) {
                files.extend(collect_dart_files(&path, skip_dirs));
            }
        } else if path.extension().is_some_and(|e| e == "dart") {
            files.push(path);
        }
    }
    files
}
