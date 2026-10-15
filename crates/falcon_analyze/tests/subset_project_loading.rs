//! Subset analysis indexes unopened local libraries and parts without an SDK.

use std::fs;

use falcon_analyze::{
    AnalysisEnvironment, DeclarationIdentity, MemberKind, MemberResult, RuleRegistry,
    SemanticModel, Workspace, analyze_sequential_collecting_resolving,
};
use falcon_config::FalconConfig;

fn assert_subset_visibility(include_library: bool, configured: bool) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    if configured {
        fs::create_dir(root.join(".dart_tool")).unwrap();
        fs::write(
            root.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.9"}]}"#,
        )
        .unwrap();
    }
    let a = root.join("a.dart");
    let b = root.join("b.dart");
    let part = root.join("b_part.dart");
    let a_source = "import 'b.dart'; void f() {}";
    let b_source = "export 'd.dart'; part 'b_part.dart'; class B { void base() {} }";
    fs::write(&a, a_source).unwrap();
    // A requested file's buffer is authoritative for its part directives.
    fs::write(
        &b,
        if include_library {
            "class Stale {}"
        } else {
            b_source
        },
    )
    .unwrap();
    fs::write(
        &part,
        "part of 'b.dart'; class C extends B { void child() {} }",
    )
    .unwrap();
    fs::write(root.join("d.dart"), "class D { void exported() {} }").unwrap();
    let environment = AnalysisEnvironment::discover_with_sdk_locator([a.as_path()], |_| None);
    let mut files = vec![(a.clone(), a_source.to_owned())];
    if include_library {
        files.push((b.clone(), b_source.to_owned()));
    }
    let (diagnostics, retained, indexes) = analyze_sequential_collecting_resolving(
        &RuleRegistry::new(),
        &files,
        &FalconConfig::default(),
        true,
        true,
        Workspace {
            packages: &[],
            environment: Some(&environment),
        },
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        retained.len(),
        files.len(),
        "companions must not become lint targets",
    );
    let indexes = indexes.unwrap();
    let model = SemanticModel::new(&a, &indexes.identities, Some(&indexes.types));
    for (name, method) in [("B", "base"), ("C", "child"), ("D", "exported")] {
        let identity = model
            .resolve_name(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert!(
            matches!(&identity, DeclarationIdentity::Project { .. }),
            "{identity:?}",
        );
        assert_eq!(
            indexes.types.member_lookup_identity(&identity, method),
            MemberResult::Found(MemberKind::Method),
            "missing type facts for {name}",
        );
        assert!(
            indexes.signatures.declaration(&identity).is_some(),
            "missing signature for {name}",
        );
    }
    let c = model.resolve_name("C").unwrap();
    assert_eq!(
        indexes.types.member_lookup_identity(&c, "base"),
        MemberResult::Found(MemberKind::Method),
        "part declarations must share the defining library's visibility",
    );
    assert!(retained.iter().all(|file| file.path != part));
}

#[test]
fn a_only_indexes_unrequested_local_library_and_its_part() {
    for configured in [false, true] {
        assert_subset_visibility(false, configured);
    }
}

#[test]
fn a_and_b_index_unrequested_part_from_buffered_directives() {
    for configured in [false, true] {
        assert_subset_visibility(true, configured);
    }
}

#[test]
fn part_only_indexes_uri_owner_siblings_and_imports() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let owner = root.join("b.dart");
    let part = root.join("b_part.dart");
    let sibling = root.join("b_other.dart");
    let source = "part of 'b.dart'; class Buffered {}";
    fs::write(
        &owner,
        "import 'dep.dart'; part 'b_part.dart'; part 'b_other.dart'; class B {}",
    )
    .unwrap();
    fs::write(&part, "part of 'b.dart'; class Stale {}").unwrap();
    fs::write(&sibling, "part of 'b.dart'; class Sibling {}").unwrap();
    fs::write(root.join("dep.dart"), "class Imported {}").unwrap();
    let environment = AnalysisEnvironment::discover_with_sdk_locator([part.as_path()], |_| None);
    let (program, errors) = falcon_dart_parser::parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let external = falcon_analyze::ExternalLibraries::load(
        &environment,
        &[(part.as_path(), &program, source)],
        |path| path == part,
    );
    let companions: Vec<_> = external.project_companions().collect();
    assert!(!companions.contains(&part.as_path()));
    for path in [&owner, &sibling] {
        assert!(
            companions.contains(&path.as_path()),
            "missing {}",
            path.display()
        );
    }
    let (diagnostics, retained, indexes) = analyze_sequential_collecting_resolving(
        &RuleRegistry::new(),
        &[(part.clone(), source.to_owned())],
        &FalconConfig::default(),
        true,
        true,
        Workspace {
            packages: &[],
            environment: Some(&environment),
        },
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(retained.len(), 1);
    assert_eq!(retained[0].path, part);
    let indexes = indexes.unwrap();
    let model = SemanticModel::new(&part, &indexes.identities, Some(&indexes.types));
    for name in ["B", "Buffered", "Sibling", "Imported"] {
        let identity = model
            .resolve_name(name)
            .unwrap_or_else(|| panic!("missing {name}"));
        assert!(indexes.types.type_kind_identity(&identity).is_some());
        assert!(indexes.signatures.declaration(&identity).is_some());
    }
    assert!(model.resolve_name("Stale").is_none());
}

#[test]
fn requested_parts_are_not_disk_companions_but_their_unopened_parts_are() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let owner = root.join("owner.dart");
    let part = root.join("part.dart");
    let nested = root.join("nested.dart");
    let owner_source = "part 'part.dart';";
    let part_source = "part of 'owner.dart'; part 'nested.dart';";
    fs::write(&owner, owner_source).unwrap();
    fs::write(&part, "part of 'owner.dart';").unwrap();
    fs::write(&nested, "part of 'owner.dart'; class Nested {}").unwrap();
    let environment = AnalysisEnvironment::discover_with_sdk_locator([owner.as_path()], |_| None);
    let (owner_program, errors) = falcon_dart_parser::parse(owner_source);
    assert!(errors.is_empty(), "{errors:?}");
    let (part_program, errors) = falcon_dart_parser::parse(part_source);
    assert!(errors.is_empty(), "{errors:?}");
    let external = falcon_analyze::ExternalLibraries::load(
        &environment,
        &[
            (owner.as_path(), &owner_program, owner_source),
            (part.as_path(), &part_program, part_source),
        ],
        |path| path == owner || path == part,
    );
    assert_eq!(
        external.project_companions().collect::<Vec<_>>(),
        [nested.as_path()],
    );
}

#[test]
fn closing_an_unimported_root_removes_its_disk_companions() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let owner = root.join("owner.dart");
    let part = root.join("part.dart");
    let remaining = root.join("remaining.dart");
    let source = "part 'part.dart';";
    fs::write(&owner, source).unwrap();
    fs::write(&part, "part of 'owner.dart'; class Part {}").unwrap();
    fs::write(&remaining, "").unwrap();
    let environment = AnalysisEnvironment::discover_with_sdk_locator([owner.as_path()], |_| None);
    let (program, errors) = falcon_dart_parser::parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let (empty, errors) = falcon_dart_parser::parse("");
    assert!(errors.is_empty(), "{errors:?}");
    let mut external = falcon_analyze::ExternalLibraries::load(
        &environment,
        &[(owner.as_path(), &program, source)],
        |path| path == owner,
    );
    assert_eq!(
        external.project_companions().collect::<Vec<_>>(),
        [part.as_path()],
    );
    external.extend(&environment, &[(remaining.as_path(), &empty, "")], |path| {
        path == remaining
    });
    assert_eq!(external.project_companions().count(), 0);
    external.extend(&environment, &[], |_| false);
    assert_eq!(external.project_companions().count(), 0);
}

#[test]
fn failed_local_import_targets_remain_available_for_file_watches() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    fs::create_dir(root.join("app")).unwrap();
    fs::create_dir(root.join("shared")).unwrap();
    let app = root.join("app/main.dart");
    let bridge = root.join("shared/bridge.dart");
    let direct = root.join("shared/direct.dart");
    let transitive = root.join("shared/transitive.dart");
    let source = "import '../shared/direct.dart'; import '../shared/bridge.dart';";
    fs::write(&app, source).unwrap();
    fs::write(&bridge, "export 'transitive.dart';").unwrap();
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let (program, errors) = falcon_dart_parser::parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let roots = [(app.as_path(), &program, source)];
    let external =
        falcon_analyze::ExternalLibraries::load(&environment, &roots, |path| path == app);
    let targets: Vec<_> = external.attempted_project_paths().collect();
    for path in [&bridge, &direct, &transitive] {
        assert!(
            targets.contains(&path.as_path()),
            "missing {}",
            path.display()
        );
    }
    assert!(!targets.contains(&app.as_path()));
    fs::write(&direct, "class Direct {}").unwrap();
    fs::write(&transitive, "class Transitive {}").unwrap();
    let refreshed =
        falcon_analyze::ExternalLibraries::load(&environment, &roots, |path| path == app);
    for path in [&direct, &transitive] {
        assert!(
            refreshed
                .libraries()
                .iter()
                .any(|library| library.files[0].path == *path),
            "new target not loaded: {}",
            path.display(),
        );
    }
}
