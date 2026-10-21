use falcon_config::{FalconConfig, PathFilter};
use std::path::Path;

#[test]
fn rooted_globs_select_the_same_absolute_and_walked_paths() {
    let root = std::env::current_dir().unwrap();
    let mut filter = PathFilter::from_includes(&["**".into(), "!build/**".into()]);
    filter.set_base_dir(&root);
    assert!(!filter.admits("./build/generated.dart"));
    assert!(!filter.admits(&root.join("build/generated.dart").to_string_lossy()));
    assert!(filter.admits("./lib/main.dart"));
    assert!(filter.excludes_directory(&root.join("build")));
}

#[test]
fn single_star_stops_at_directory_separator() {
    let mut filter = PathFilter::from_includes(&["lib/*.dart".into()]);
    filter.set_base_dir(Path::new("/project"));
    assert!(filter.admits("/project/lib/main.dart"));
    assert!(!filter.admits("/project/lib/nested/main.dart"));
    let mut recursive = PathFilter::from_includes(&["lib/**/*.dart".into()]);
    recursive.set_base_dir(Path::new("/project"));
    assert!(recursive.admits("/project/lib/nested/main.dart"));
    assert!(recursive.admits("/project/lib/main.dart"));
    let mut exclusions = PathFilter::from_includes(&["!lib/*.dart".into()]);
    exclusions.set_base_dir(Path::new("/project"));
    assert!(!exclusions.admits("/project/lib/main.dart"));
    assert!(exclusions.admits("/project/lib/nested/main.dart"));
    assert!(!exclusions.excludes_directory(Path::new("/project/lib/nested")));
}

#[test]
fn override_and_parent_globs_are_relative_to_configuration() {
    let mut config: FalconConfig = serde_json::from_str(
        r#"{"files":{"includes":["../shared/**"]},"overrides":[{"includes":["lib/**"]}]}"#,
    )
    .unwrap();
    config.resolve_paths(Path::new("/project/app"));
    assert!(config.files.includes.admits("/project/shared/main.dart"));
    assert!(config.overrides[0].matches("/project/app/lib/main.dart"));
    assert!(!config.overrides[0].matches("/project/other/lib/main.dart"));
}

#[test]
fn excluding_a_directory_also_excludes_its_descendants() {
    let mut filter = PathFilter::from_includes(&["!build".into()]);
    filter.set_base_dir(Path::new("/project"));
    assert!(filter.excludes_directory(Path::new("/project/build")));
    assert!(!filter.admits("/project/build/nested/main.dart"));
    assert!(filter.admits("/project/lib/main.dart"));
    assert!(filter.excludes_directory(Path::new("/project/build/")));
}

#[test]
fn literal_config_directory_does_not_become_a_glob() {
    let mut filter = PathFilter::from_includes(&["lib/**".into()]);
    filter.set_base_dir(Path::new("/project/[app]"));
    assert!(filter.admits("/project/[app]/lib/main.dart"));
    assert!(!filter.admits("/project/a/lib/main.dart"));
}

#[cfg(unix)]
#[test]
fn configuration_symlink_and_document_paths_share_a_root() {
    let root = tempfile::tempdir().unwrap();
    let real = root.path().join("real");
    std::fs::create_dir_all(real.join("lib")).unwrap();
    std::fs::write(real.join("lib/main.dart"), "void main() {}").unwrap();
    let alias = root.path().join("alias[app]");
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let mut filter = PathFilter::from_includes(&["lib/**".into()]);
    filter.set_base_dir(&alias);
    assert!(filter.admits(&alias.join("lib/main.dart").to_string_lossy()));
    assert!(filter.admits(&real.join("lib/main.dart").to_string_lossy()));
    assert!(filter.admits(&alias.join("lib/unsaved.dart").to_string_lossy()));
    let mut parent = PathFilter::from_includes(&["../shared/**".into()]);
    parent.set_base_dir(&alias);
    assert!(parent.admits(&root.path().join("shared/main.dart").to_string_lossy()));
    let mut parent_exclusion = PathFilter::from_includes(&["!../**/build/**".into()]);
    parent_exclusion.set_base_dir(&alias);
    assert!(!parent_exclusion.admits(&real.join("build/main.dart").to_string_lossy()));
    for base in [&alias, &real] {
        let mut absolute = PathFilter::from_includes(&[format!("!{}/build/**", base.display())]);
        absolute.set_base_dir(&alias);
        assert!(!absolute.admits(&real.join("build/main.dart").to_string_lossy()));
        assert!(absolute.excludes_directory(&alias.join("build")));
    }
}

#[test]
fn absolute_paths_with_dot_components_and_repeated_separators_match() {
    let mut filter = PathFilter::from_includes(&["lib/**".into()]);
    filter.set_base_dir(Path::new("/project"));
    assert!(filter.admits("/project/./lib/main.dart"));
    assert!(filter.admits("/project//lib/main.dart"));
    assert!(filter.admits("/project/lib/../lib/main.dart"));
}

#[test]
fn relative_candidates_use_cwd_not_the_configuration_directory() {
    let cwd = std::env::current_dir().unwrap();
    let directory = tempfile::tempdir_in(&cwd).unwrap();
    let mut filter = PathFilter::from_includes(&["lib/*.dart".into()]);
    filter.set_base_dir(directory.path());
    let candidate = directory
        .path()
        .strip_prefix(&cwd)
        .unwrap()
        .join("lib/main.dart");
    assert!(filter.admits(&candidate.to_string_lossy()));
    assert!(!filter.admits("lib/main.dart"));
}

#[test]
fn rooted_bare_catchalls_match_omitted_includes_and_keep_exclusions_rooted() {
    let base = Path::new("/project/elsewhere");
    let mut omitted: FalconConfig =
        serde_json::from_str(r#"{"files":{"includes":["!build/**"]}}"#).unwrap();
    omitted.resolve_paths(base);
    for catchall in ["**", "**/*"] {
        let mut config: FalconConfig = serde_json::from_value(serde_json::json!({
            "files": { "includes": [catchall, "!build/**"] }
        }))
        .unwrap();
        config.resolve_paths(base);
        for path in [
            "/project/a.dart",
            "/project/build/generated.dart",
            "/project/elsewhere/lib/main.dart",
        ] {
            assert!(omitted.files.includes.admits(path));
            assert!(config.files.includes.admits(path), "{catchall}: {path}");
        }
        let excluded = "/project/elsewhere/build/generated.dart";
        assert!(!omitted.files.includes.admits(excluded));
        assert!(!config.files.includes.admits(excluded));
    }
}

#[test]
fn invalid_rooted_pattern_warning_names_original_pattern() {
    let mut filter = PathFilter::from_includes(&["lib/[".into()]);
    filter.set_base_dir(Path::new("/project/[app]"));
    assert!(filter.warnings()[0].contains("`lib/[`"));
}
