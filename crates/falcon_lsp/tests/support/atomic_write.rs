pub fn write_file_atomically(path: impl AsRef<std::path::Path>, contents: impl AsRef<[u8]>) {
    use std::io::Write as _;

    // Idle revalidation must not observe truncated rewrites of files the server already tracks.
    let path = path.as_ref();
    let mut file = tempfile::NamedTempFile::new_in(path.parent().unwrap()).unwrap();
    file.write_all(contents.as_ref()).unwrap();
    file.persist(path).unwrap();
}
