use std::{fs, sync::Arc};

use falcon_analyze::{
    AnalysisEnvironment, AnalyzeContext, DeclarationIdentity, ExternalLibraries, ExternalSources,
    IdentityIndex, IdentitySource, LibrarySource, ParsedFile, Rule, RuleAnalysis, SemanticModel,
    SignatureIndex, TypeIndex, group_libraries, library_unit,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use falcon_rules::lint::{
    correctness::diagnostic_describe_all_properties::DiagnosticDescribeAllProperties,
    style::{
        avoid_catches_without_on_clauses::AvoidCatchesWithoutOnClauses,
        use_late_for_private_fields_and_variables::UseLateForPrivateFieldsAndVariables,
    },
};

fn parsed(source: &str) -> Arc<ParsedFile> {
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    Arc::new(ParsedFile {
        source: source.into(),
        tree: result.tree,
    })
}

#[test]
fn colliding_package_names_do_not_gain_project_or_sdk_semantics() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".dart_tool")).unwrap();
    fs::create_dir_all(dir.path().join("dep/lib")).unwrap();
    fs::write(
        dir.path().join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../dep","packageUri":"lib/"}]}"#,
    )
    .unwrap();
    for file in ["a.dart", "b.dart"] {
        fs::write(
            dir.path().join("dep/lib").join(file),
            "class Widget {} class Future { static dynamic error(Object error) {} }",
        )
        .unwrap();
    }
    let source = "import 'package:dep/a.dart'; import 'package:dep/b.dart' as other; class Diagnosticable {} class Owner extends Diagnosticable { Widget get child => throw 0; } void f() { try {} catch (error) { Future.error(error); } }";
    let path = dir.path().join("main.dart");
    fs::write(&path, source).unwrap();
    let parsed = parsed(source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
    let externals = ExternalLibraries::load(
        &environment,
        &[(path.as_path(), parsed.program(), source)],
        |file| file == path,
    );
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        Some(ExternalSources {
            libraries: &externals,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            LibrarySource {
                program: parsed.program(),
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(path.clone(), parsed.clone())],
        Some(&externals),
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    let model = SemanticModel::new(&path, &parsed, &identities, Some(&types));
    let identity = model.resolve_name("Widget").unwrap();
    assert!(matches!(identity, DeclarationIdentity::Library { .. }));
    assert!(!identities.is_project(&identity));
    let diagnostics =
        falcon_rules::lint::correctness::diagnostic_describe_all_properties::DiagnosticDescribeAllProperties.analyze(DiagnosticDescribeAllProperties::NAME, parsed.program(), &ctx);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let diagnostics =
        falcon_rules::lint::style::avoid_catches_without_on_clauses::AvoidCatchesWithoutOnClauses
            .analyze(AvoidCatchesWithoutOnClauses::NAME, parsed.program(), &ctx);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
}

#[test]
fn sibling_context_uses_its_own_file_identity() {
    let paths = [
        std::path::Path::new("/project/owner.dart"),
        std::path::Path::new("/project/part.dart"),
    ];
    let files = [
        parsed("part 'part.dart'; int owner = 1;"),
        parsed("part of 'owner.dart'; String sibling = '';"),
    ];
    let sources = paths
        .iter()
        .zip(&files)
        .map(|(path, parsed)| IdentitySource {
            path,
            parsed,
            has_parse_errors: false,
        })
        .collect::<Vec<_>>();
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        paths.iter().zip(&files).map(|(path, parsed)| {
            (
                *path,
                LibrarySource {
                    program: parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let signatures = SignatureIndex::build(
        &paths
            .iter()
            .zip(&files)
            .map(|(path, parsed)| (path.to_path_buf(), parsed.clone()))
            .collect::<Vec<_>>(),
        None,
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let owner = AnalyzeContext::new(paths[0], &config, &files[0])
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    let sibling = owner.for_file(paths[1], &files[1]);
    assert_eq!(sibling.file_path(), paths[1]);
    assert!(Arc::ptr_eq(sibling.parsed(), &files[1]));
    let direct = AnalyzeContext::new(paths[1], &config, &files[1])
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    assert!(std::ptr::eq(
        sibling.flow_writes(files[1].program()),
        direct.flow_writes(files[1].program()),
    ));
}

#[test]
fn unresolved_error_forwarding_stays_silent() {
    for directive in ["part 'missing.g.dart';", "import 'missing.dart';"] {
        let path = std::path::Path::new("/project/main.dart");
        let source =
            format!("{directive} void f() {{ try {{}} catch (error) {{ Future.error(error); }} }}");
        let parsed = parsed(&source);
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_resolved_files(
            [(
                path,
                LibrarySource {
                    program: parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: directive.starts_with("part"),
                },
            )],
            &identities,
        );
        let signatures = SignatureIndex::build(
            &[(path.to_path_buf(), parsed.clone())],
            None,
            &identities,
            &types,
        );
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        let diagnostics =
            falcon_rules::lint::style::avoid_catches_without_on_clauses::AvoidCatchesWithoutOnClauses.analyze(AvoidCatchesWithoutOnClauses::NAME, parsed.program(), &ctx);
        assert!(diagnostics.is_empty(), "{directive}: {diagnostics:?}");
    }
}

fn assert_dependency_rule(
    name: &'static str,
    dependencies: &[(&str, &str, &str)],
    source: &str,
    rule: &dyn Rule,
    expected: usize,
) {
    assert_dependency_rule_with(name, dependencies, source, rule, expected, |_, _| {});
}

fn assert_dependency_rule_with(
    name: &'static str,
    dependencies: &[(&str, &str, &str)],
    source: &str,
    rule: &dyn Rule,
    expected: usize,
    check: impl FnOnce(&SemanticModel<'_>, &SignatureIndex),
) {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join(".dart_tool")).unwrap();
    let mut packages = Vec::with_capacity(dependencies.len());
    for &(package, file, contents) in dependencies {
        let root = dir.path().join(package).join("lib");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(file), contents).unwrap();
        if !packages
            .iter()
            .any(|entry: &serde_json::Value| entry["name"] == package)
        {
            packages.push(serde_json::json!({
                "name": package,
                "rootUri": format!("../{package}"),
                "packageUri": "lib/",
            }));
        }
    }
    fs::write(
        dir.path().join(".dart_tool/package_config.json"),
        serde_json::json!({"configVersion": 2, "packages": packages}).to_string(),
    )
    .unwrap();
    let path = dir.path().join("main.dart");
    fs::write(&path, source).unwrap();
    let parsed = parsed(source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
    let externals = ExternalLibraries::load(
        &environment,
        &[(path.as_path(), parsed.program(), source)],
        |file| file == path,
    );
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        Some(ExternalSources {
            libraries: &externals,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            LibrarySource {
                program: parsed.program(),
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(path.clone(), parsed.clone())],
        Some(&externals),
        &identities,
        &types,
    );
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(&path, &config, &parsed)
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures);
    let model = SemanticModel::new(&path, &parsed, &identities, Some(&types));
    check(&model, &signatures);
    let diagnostics = rule.analyze(name, parsed.program(), &ctx);
    assert_eq!(diagnostics.len(), expected, "{source}\n{diagnostics:?}");
}

#[test]
fn late_candidates_use_part_loop_writes_not_owner_loop_writes() {
    let paths = [
        std::path::Path::new("/project/owner.dart"),
        std::path::Path::new("/project/part.dart"),
    ];
    let files = [
        parsed("part 'part.dart'; int? _unsafe; int? _safe;"),
        parsed(
            "part of 'owner.dart'; void f(bool again) { int? value = 1; while (again) { _unsafe = value; value = null; _safe = 1; } }",
        ),
    ];
    let identities = IdentityIndex::from_project_files(
        &paths
            .iter()
            .zip(&files)
            .map(|(path, parsed)| IdentitySource {
                path,
                parsed,
                has_parse_errors: false,
            })
            .collect::<Vec<_>>(),
        &[],
    );
    let types = TypeIndex::from_resolved_files(
        paths.iter().zip(&files).map(|(path, parsed)| {
            (
                *path,
                LibrarySource {
                    program: parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let signatures = SignatureIndex::build(
        &paths
            .iter()
            .zip(&files)
            .map(|(path, parsed)| (path.to_path_buf(), parsed.clone()))
            .collect::<Vec<_>>(),
        None,
        &identities,
        &types,
    );
    let grouping = group_libraries(
        &paths
            .iter()
            .zip(&files)
            .map(|(path, parsed)| (path.to_path_buf(), parsed.program()))
            .collect::<Vec<_>>(),
    );
    let unit = library_unit(&grouping, &[&files[0], &files[1]], 0);
    let config = FalconConfig::default();
    let ctx = AnalyzeContext::new(paths[0], &config, &files[0])
        .with_identities(&identities)
        .with_types(&types)
        .with_signatures(&signatures)
        .with_library(&unit);
    let diagnostics =
        falcon_rules::lint::style::use_late_for_private_fields_and_variables::UseLateForPrivateFieldsAndVariables.analyze(UseLateForPrivateFieldsAndVariables::NAME, files[0].program(), &ctx);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(
        diagnostics[0].span.start,
        files[0].source.find("_safe").unwrap(),
        "{diagnostics:?}",
    );
}

#[test]
fn colliding_dependency_bloc_models_are_not_project_roots() {
    use falcon_rules::lint::correctness::{
        prefer_sealed_bloc_events::PreferSealedBlocEvents,
        prefer_sealed_bloc_state::PreferSealedBlocState,
    };

    let dependencies = [
        ("bloc", "bloc.dart", "class Bloc<E, S> {}"),
        ("dep", "a.dart", "class Event {} class State {}"),
        ("dep", "b.dart", "class Event {} class State {}"),
    ];
    let imports = "import 'package:bloc/bloc.dart'; import 'package:dep/a.dart'; import 'package:dep/b.dart' as other;";
    let source = format!(
        "{imports} class ChildEvent extends Event {{}} class ChildState extends State {{}} class Owner extends Bloc<Event, State> {{}}"
    );
    let check = |model: &SemanticModel<'_>, _: &SignatureIndex| {
        for name in ["Event", "State"] {
            let identity = model.resolve_name(name).unwrap();
            assert!(matches!(identity, DeclarationIdentity::Library { .. }));
            assert!(!model.identities().is_project(&identity));
        }
    };
    assert_dependency_rule_with(
        "prefer-sealed-bloc-events",
        &dependencies,
        &source,
        &PreferSealedBlocEvents,
        0,
        check,
    );
    assert_dependency_rule_with(
        "prefer-sealed-bloc-state",
        &dependencies,
        &source,
        &PreferSealedBlocState,
        0,
        check,
    );
    let source = format!(
        "{imports} class LocalEvent {{}} class LocalState {{}} class Owner extends Bloc<LocalEvent, LocalState> {{}}"
    );
    assert_dependency_rule(
        "prefer-sealed-bloc-events",
        &dependencies,
        &source,
        &PreferSealedBlocEvents,
        1,
    );
    assert_dependency_rule(
        "prefer-sealed-bloc-state",
        &dependencies,
        &source,
        &PreferSealedBlocState,
        1,
    );
}

#[test]
fn colliding_dependency_hook_ancestry_remains_unknown() {
    use falcon_rules::lint::correctness::avoid_hooks_outside_build::AvoidHooksOutsideBuild;

    let dependencies = [
        (
            "flutter_hooks",
            "flutter_hooks.dart",
            "int useState(int value) => value;",
        ),
        ("dep", "a.dart", "class Base {}"),
        ("dep", "b.dart", "class Base {}"),
    ];
    let imports = "import 'package:flutter_hooks/flutter_hooks.dart'; import 'package:dep/a.dart'; import 'package:dep/b.dart' as other;";
    assert_dependency_rule_with(
        "avoid-hooks-outside-build",
        &dependencies,
        &format!("{imports} class Owner extends Base {{ void build() {{ useState(0); }} }}"),
        &AvoidHooksOutsideBuild,
        0,
        |model, signatures| {
            let base = model.resolve_name("Base").unwrap();
            assert!(matches!(base, DeclarationIdentity::Library { .. }));
            assert!(!model.identities().is_project(&base));
            assert!(signatures.superclass(&base).is_none());
        },
    );
    assert_dependency_rule(
        "avoid-hooks-outside-build",
        &dependencies,
        &format!("{imports} class Owner {{ void build() {{ useState(0); }} }}"),
        &AvoidHooksOutsideBuild,
        1,
    );
}

#[test]
fn colliding_dependency_blocs_do_not_register_project_events() {
    use falcon_rules::lint::suspicious::avoid_passing_build_context_to_blocs::AvoidPassingBuildContextToBlocs;

    // Only the Bloc owner collides: its event is a genuine project class.
    let dependencies = [
        ("bloc", "bloc.dart", "class Bloc<E, S> {}"),
        ("flutter", "widgets.dart", "class BuildContext {}"),
        (
            "dep",
            "a.dart",
            "import 'package:bloc/bloc.dart'; import '../../main.dart'; class Owner extends Bloc<Event, int> {}",
        ),
        ("dep", "b.dart", "class Owner {}"),
    ];
    let source = "import 'package:flutter/widgets.dart'; import 'package:dep/a.dart'; import 'package:dep/b.dart' as other; class Event { Event(BuildContext context); } void f(BuildContext context) { Event(context); }";
    assert_dependency_rule_with(
        "avoid-passing-build-context-to-blocs",
        &dependencies,
        source,
        &AvoidPassingBuildContextToBlocs,
        0,
        |model, signatures| {
            let owner = model.resolve_name("Owner").unwrap();
            assert!(matches!(owner, DeclarationIdentity::Library { .. }));
            assert!(!model.identities().is_project(&owner));
            assert!(
                signatures
                    .declaration_identities()
                    .any(|identity| identity == &owner)
            );
            let event = model.resolve_name("Event").unwrap();
            assert!(model.identities().is_project(&event));
            let superclass = signatures.superclass(&owner).unwrap();
            assert_eq!(
                superclass.arguments().first(),
                Some(&falcon_analyze::ResolvedType::interface_identity(event)),
                "{superclass:?}",
            );
        },
    );
    let source = format!(
        "import 'package:bloc/bloc.dart'; {source} class LocalBloc extends Bloc<Event, int> {{}}"
    );
    assert_dependency_rule(
        "avoid-passing-build-context-to-blocs",
        &dependencies,
        &source,
        &AvoidPassingBuildContextToBlocs,
        1,
    );
}

#[test]
fn colliding_dependency_events_are_not_registered_by_project_blocs() {
    use falcon_rules::lint::suspicious::avoid_passing_build_context_to_blocs::AvoidPassingBuildContextToBlocs;

    let dependencies = [
        ("bloc", "bloc.dart", "class Bloc<E, S> {}"),
        ("flutter", "widgets.dart", "class BuildContext {}"),
        (
            "dep",
            "a.dart",
            "import 'package:flutter/widgets.dart'; class Event { Event(BuildContext context); }",
        ),
        ("dep", "b.dart", "class Event {}"),
    ];
    let imports = "import 'package:bloc/bloc.dart'; import 'package:flutter/widgets.dart'; import 'package:dep/a.dart'; import 'package:dep/b.dart' as other;";
    assert_dependency_rule_with(
        "avoid-passing-build-context-to-blocs",
        &dependencies,
        &format!(
            "{imports} class Owner extends Bloc<Event, int> {{}} void f(BuildContext context) {{ Event(context); }}"
        ),
        &AvoidPassingBuildContextToBlocs,
        0,
        |model, signatures| {
            let event = model.resolve_name("Event").unwrap();
            assert!(matches!(event, DeclarationIdentity::Library { .. }));
            assert!(!model.identities().is_project(&event));
            let owner = model.resolve_name("Owner").unwrap();
            assert!(model.identities().is_project(&owner));
            let superclass = signatures.superclass(&owner).unwrap();
            assert_eq!(
                superclass.arguments().first(),
                Some(&falcon_analyze::ResolvedType::interface_identity(event)),
                "{superclass:?}",
            );
        },
    );
    assert_dependency_rule(
        "avoid-passing-build-context-to-blocs",
        &dependencies,
        &format!(
            "{imports} class LocalEvent {{ LocalEvent(BuildContext context); }} class Owner extends Bloc<LocalEvent, int> {{}} void f(BuildContext context) {{ LocalEvent(context); }}"
        ),
        &AvoidPassingBuildContextToBlocs,
        1,
    );
}

#[test]
fn unique_dependency_future_does_not_forward_caught_errors() {
    use falcon_rules::lint::style::avoid_catches_without_on_clauses::AvoidCatchesWithoutOnClauses;

    let dependencies = [(
        "dep",
        "future.dart",
        "class Future { static dynamic error(Object error) {} }",
    )];
    assert_dependency_rule_with(
        "avoid-catches-without-on-clauses",
        &dependencies,
        "import 'package:dep/future.dart'; void f() { try {} catch (error) { Future.error(error); } }",
        &AvoidCatchesWithoutOnClauses,
        1,
        |model, _| {
            let future = model.resolve_name("Future").unwrap();
            assert!(future.is_package("dep", "Future"), "{future:?}");
            assert!(!model.identities().is_project(&future));
        },
    );
    assert_dependency_rule(
        "avoid-catches-without-on-clauses",
        &dependencies,
        "void f() { try {} catch (error) { Future.error(error); } }",
        &AvoidCatchesWithoutOnClauses,
        0,
    );
}
