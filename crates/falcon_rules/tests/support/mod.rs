use std::path::{Path, PathBuf};

pub fn files(root: &Path, max_depth: usize, extension: Option<&str>) -> Vec<PathBuf> {
    let mut files: Vec<_> = walkdir::WalkDir::new(root)
        .follow_links(true)
        .max_depth(max_depth)
        .into_iter()
        .map(|entry| {
            entry.unwrap_or_else(|error| panic!("cannot walk {}: {error}", root.display()))
        })
        .filter(|entry| entry.file_type().is_file())
        .map(walkdir::DirEntry::into_path)
        .filter(|path| {
            extension.is_none_or(|extension| path.extension().is_some_and(|ext| ext == extension))
        })
        .collect();
    files.sort();
    files
}
