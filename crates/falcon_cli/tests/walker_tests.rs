use falcon_cli::file_walker::walk_project;
use falcon_config::PathFilter;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;

#[test]
fn one_walk_prunes_excluded_directories_and_collects_manifests_once() {
    let temp = tempdir().unwrap();
    let excluded = temp.path().join("build");
    fs::create_dir(&excluded).unwrap();
    for name in ["oversized.dart", "pubspec.yaml"] {
        fs::File::create(excluded.join(name))
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
    }
    let source = temp.path().join("main.dart");
    fs::write(&source, "void main() {}").unwrap();
    let manifest = temp.path().join("pubspec.yaml");
    fs::write(&manifest, "name: package\n").unwrap();
    let mut filter = PathFilter::from_includes(&["**/*.dart".into(), "!build".into()]);
    filter.set_base_dir(temp.path());
    let output = walk_project(&[temp.path().to_path_buf(), source.clone()], &filter, true).unwrap();
    assert_eq!(output.files, [(source, "void main() {}".into())]);
    assert_eq!(output.manifests, [(manifest, "name: package\n".into())]);
    assert!(output.missing_paths.is_empty());
}

#[test]
fn test_walk_single_dart_file() {
    let temp = tempdir().unwrap();
    let dart_file = temp.path().join("test.dart");
    fs::write(&dart_file, "void main() {}").unwrap();

    let results = walk_project(
        std::slice::from_ref(&dart_file),
        &PathFilter::default(),
        false,
    )
    .unwrap()
    .files;
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, dart_file);
    assert_eq!(results[0].1, "void main() {}");
}

#[test]
fn oversized_sources_and_manifests_abort_collection() {
    for name in ["oversized.dart", "pubspec.yaml"] {
        let temp = tempdir().unwrap();
        fs::File::create(temp.path().join(name))
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
        fs::write(temp.path().join("small.dart"), "void main() {}").unwrap();
        let error =
            walk_project(&[temp.path().to_path_buf()], &PathFilter::default(), true).unwrap_err();
        assert!(error.contains(name));
        assert!(error.contains("maximum supported source size"));
    }
}

#[test]
fn ordinary_read_errors_skip_sources_and_ancestor_manifests() {
    let temp = tempdir().unwrap();
    let lib = temp.path().join("lib");
    fs::create_dir(&lib).unwrap();
    let valid = lib.join("main.dart");
    fs::write(&valid, "void main() {}").unwrap();
    fs::write(lib.join("invalid.dart"), [0xff]).unwrap();
    fs::create_dir(lib.join("directory.dart")).unwrap();
    fs::write(temp.path().join("pubspec.yaml"), [0xff]).unwrap();
    let manifest = lib.join("pubspec.yaml");
    fs::write(&manifest, "name: local\n").unwrap();
    let paths = [lib];
    assert_eq!(
        walk_project(&paths, &PathFilter::default(), false)
            .unwrap()
            .files,
        [(valid, "void main() {}".to_string())]
    );
    let manifests = walk_project(&paths, &PathFilter::default(), true)
        .unwrap()
        .manifests;
    assert!(manifests.contains(&(manifest, "name: local\n".to_string())));
    assert!(
        manifests
            .iter()
            .all(|(path, _)| path != &temp.path().join("pubspec.yaml"))
    );
}

#[test]
fn excluded_oversized_files_do_not_abort_collection() {
    let temp = tempdir().unwrap();
    let excluded = temp.path().join("excluded");
    fs::create_dir(&excluded).unwrap();
    for name in ["oversized.dart", "pubspec.yaml"] {
        fs::File::create(excluded.join(name))
            .unwrap()
            .set_len(u64::from(u32::MAX) + 1)
            .unwrap();
    }
    let filter = PathFilter::from_includes(&["!**/excluded/**".to_string()]);
    let paths = [temp.path().to_path_buf()];
    assert!(
        walk_project(&paths, &filter, false)
            .unwrap()
            .files
            .is_empty()
    );
    assert!(
        walk_project(&paths, &filter, true)
            .unwrap()
            .manifests
            .iter()
            .all(|(path, _)| !path.starts_with(&excluded))
    );
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

    let results = walk_project(&[temp.path().to_path_buf()], &PathFilter::default(), false)
        .unwrap()
        .files;
    assert_eq!(results.len(), 3);

    let paths: Vec<_> = results.iter().map(|(p, _)| p).collect();
    assert!(paths.contains(&&dart1));
    assert!(paths.contains(&&dart2));
    assert!(paths.contains(&&dart3));
}

#[test]
fn test_walk_empty_directory() {
    let temp = tempdir().unwrap();
    let results = walk_project(&[temp.path().to_path_buf()], &PathFilter::default(), false)
        .unwrap()
        .files;
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

    let filter = PathFilter::from_includes(&["!**/build/**".to_string()]);
    let results = walk_project(&[temp.path().to_path_buf()], &filter, false)
        .unwrap()
        .files;

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

    let results = walk_project(&[temp.path().to_path_buf()], &PathFilter::default(), false)
        .unwrap()
        .files;
    assert_eq!(results.len(), 2);

    let paths: Vec<_> = results.iter().map(|(p, _)| p).collect();
    assert!(paths.contains(&&b_dart));
    assert!(paths.contains(&&c_dart));
}

#[test]
fn test_walk_nonexistent_path_reports_missing() {
    let nonexistent = PathBuf::from("/nonexistent/path/to/file.dart");
    let output = walk_project(
        std::slice::from_ref(&nonexistent),
        &PathFilter::default(),
        true,
    )
    .unwrap();
    assert!(output.files.is_empty());
    assert!(output.manifests.is_empty());
    assert_eq!(output.missing_paths, [nonexistent]);
}

#[test]
fn explicit_filtered_files_are_reported_even_with_an_admitted_input() {
    let temp = tempdir().unwrap();
    let source = temp.path().join("main.dart");
    let filtered = temp.path().join("other.dart");
    fs::write(&source, "void main() {}").unwrap();
    fs::write(&filtered, "void other() {}").unwrap();
    let mut filter = PathFilter::from_includes(&["main.dart".into()]);
    filter.set_base_dir(temp.path());
    let output = walk_project(&[source.clone(), filtered.clone()], &filter, false).unwrap();
    assert_eq!(output.files, [(source, "void main() {}".into())]);
    assert_eq!(output.filtered_paths, [filtered]);
    assert!(output.missing_paths.is_empty());
}

#[test]
fn unsupported_explicit_files_do_not_discover_ancestor_manifests() {
    let temp = tempdir().unwrap();
    fs::write(temp.path().join("pubspec.yaml"), "name: example\n").unwrap();
    let readme = temp.path().join("README.txt");
    fs::write(&readme, "package notes").unwrap();
    let output = walk_project(&[readme], &PathFilter::default(), true).unwrap();
    assert!(output.files.is_empty());
    assert!(output.manifests.is_empty());
}

#[cfg(unix)]
#[test]
fn test_walk_does_not_follow_symlinked_dirs() {
    let outside = tempdir().unwrap();
    fs::write(outside.path().join("external.dart"), "void main() {}").unwrap();

    let temp = tempdir().unwrap();
    fs::write(temp.path().join("local.dart"), "void main() {}").unwrap();
    std::os::unix::fs::symlink(outside.path(), temp.path().join("plugin_link")).unwrap();
    // A self-referential link must not cycle the walk either.
    std::os::unix::fs::symlink(temp.path(), temp.path().join("self_link")).unwrap();

    let results = walk_project(&[temp.path().to_path_buf()], &PathFilter::default(), false)
        .unwrap()
        .files;
    assert_eq!(results.len(), 1);
    assert!(results[0].0.ends_with("local.dart"));
}

#[test]
fn test_walk_pubspec_only_package() {
    let temp = tempdir().unwrap();
    let pubspec = temp.path().join("pubspec.yaml");
    fs::write(&pubspec, "name: package\n").unwrap();
    let results = walk_project(&[temp.path().to_path_buf()], &PathFilter::default(), true)
        .unwrap()
        .manifests;
    assert_eq!(results, [(pubspec, "name: package\n".to_string())]);
}

#[test]
fn dart_includes_do_not_hide_package_manifests() {
    let temp = tempdir().unwrap();
    let lib = temp.path().join("lib");
    fs::create_dir(&lib).unwrap();
    fs::write(lib.join("main.dart"), "void main() {}").unwrap();
    fs::write(temp.path().join("other.dart"), "void other() {}").unwrap();
    let pubspec = temp.path().join("pubspec.yaml");
    fs::write(&pubspec, "name: package\n").unwrap();
    let mut filter = PathFilter::from_includes(&["**/lib/**".to_string()]);
    assert_eq!(
        walk_project(&[temp.path().to_path_buf()], &filter, false)
            .unwrap()
            .files
            .len(),
        1
    );
    assert_eq!(
        walk_project(&[temp.path().to_path_buf()], &filter, true)
            .unwrap()
            .manifests
            .len(),
        1
    );
    filter.extend_excludes(&["**/pubspec.yaml".to_string()]);
    assert!(
        walk_project(&[temp.path().to_path_buf()], &filter, true)
            .unwrap()
            .manifests
            .is_empty()
    );
}
