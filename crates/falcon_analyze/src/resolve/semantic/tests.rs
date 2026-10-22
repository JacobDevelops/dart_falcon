use std::path::{Path, PathBuf};

use falcon_dart_parser::parse;

use super::*;
use crate::resolve::{IdentitySource, LibrarySource};

#[test]
fn fallback_generic_arities_live_in_the_signature_index() {
    let mut signatures = SignatureIndex::default();
    let list = DeclarationIdentity::sdk("dart:core", "List");
    signatures
        .type_facts
        .entry(list.clone())
        .or_default()
        .supertypes = Some((
        (0..3)
            .map(|ordinal| sdk_parameter("List", ordinal))
            .collect(),
        Vec::new(),
    ));
    signatures.add_fallback_signatures();
    assert_eq!(signatures.type_parameter_count(&list), Some(3));
    for (identity, arity) in [
        (DeclarationIdentity::sdk("dart:core", "String"), 0),
        (DeclarationIdentity::sdk("dart:core", "Iterable"), 1),
        (DeclarationIdentity::sdk("dart:core", "Iterator"), 1),
        (DeclarationIdentity::sdk("dart:core", "Comparable"), 1),
        (DeclarationIdentity::sdk("dart:core", "MapEntry"), 2),
        (DeclarationIdentity::sdk("dart:core", "Expando"), 1),
        (DeclarationIdentity::sdk("dart:core", "WeakReference"), 1),
        (DeclarationIdentity::sdk("dart:core", "Finalizer"), 1),
        (DeclarationIdentity::sdk("dart:core", "Map"), 2),
        (DeclarationIdentity::sdk("dart:async", "Future"), 1),
        (DeclarationIdentity::sdk("dart:async", "Completer"), 1),
        (DeclarationIdentity::sdk("dart:async", "Stream"), 1),
        (DeclarationIdentity::sdk("dart:async", "FutureOr"), 1),
        (DeclarationIdentity::sdk("dart:collection", "HashMap"), 2),
        (
            DeclarationIdentity::sdk("dart:collection", "LinkedHashMap"),
            2,
        ),
        (
            DeclarationIdentity::sdk("dart:collection", "SplayTreeMap"),
            2,
        ),
        (DeclarationIdentity::sdk("dart:collection", "HashSet"), 1),
        (
            DeclarationIdentity::sdk("dart:collection", "LinkedHashSet"),
            1,
        ),
        (
            DeclarationIdentity::sdk("dart:collection", "SplayTreeSet"),
            1,
        ),
        (DeclarationIdentity::sdk("dart:collection", "ListQueue"), 1),
        (
            DeclarationIdentity::sdk("dart:collection", "DoubleLinkedQueue"),
            1,
        ),
        (DeclarationIdentity::sdk("dart:js_interop", "JSArray"), 1),
        (DeclarationIdentity::sdk("dart:js_interop", "JSPromise"), 1),
        (DeclarationIdentity::package("flutter", "State"), 1),
        (DeclarationIdentity::package("flutter", "BuildContext"), 0),
        (DeclarationIdentity::sdk("dart:core", "Object"), 0),
        (DeclarationIdentity::sdk("dart:math", "Random"), 0),
    ] {
        assert_eq!(signatures.type_parameter_count(&identity), Some(arity));
    }
    for identity in [
        DeclarationIdentity::package("flutter", "ValueNotifier"),
        DeclarationIdentity::package("provider", "Provider"),
        DeclarationIdentity::package("unknown", "Box"),
        DeclarationIdentity::sdk("dart:unknown", "Box"),
    ] {
        assert_eq!(signatures.type_parameter_count(&identity), None);
    }
}

#[test]
fn fallback_arities_do_not_invent_supertype_facts() {
    let mut signatures = SignatureIndex::default();
    signatures.add_fallback_signatures();
    for identity in [
        DeclarationIdentity::sdk("dart:collection", "HashMap"),
        DeclarationIdentity::sdk("dart:collection", "ListQueue"),
        DeclarationIdentity::sdk("dart:js_interop", "JSArray"),
        DeclarationIdentity::package("flutter", "BuildContext"),
    ] {
        assert_eq!(
            signatures.implements_sdk(
                &ResolvedType::interface_identity(identity),
                "dart:core",
                "Iterable",
            ),
            Truth::Unknown,
        );
    }
    for (name, supertype) in [("int", "num"), ("FormatException", "Exception")] {
        assert_eq!(
            signatures.implements_sdk(
                &ResolvedType::interface_identity(DeclarationIdentity::sdk("dart:core", name)),
                "dart:core",
                supertype,
            ),
            Truth::Yes,
        );
    }
}

#[test]
fn fallback_sdk_records_fill_gaps_without_merging_overrides() {
    let mut signatures = SignatureIndex::default();
    let object = DeclarationIdentity::sdk("dart:core", "Object");
    let string = DeclarationIdentity::sdk("dart:core", "String");
    signatures.member_entries.insert(
        object.clone(),
        FxHashMap::from_iter([(
            "custom".to_string(),
            MemberEntry {
                read: Some(ResolvedType::VOID),
                ..MemberEntry::default()
            },
        )]),
    );
    signatures
        .type_facts
        .insert(string.clone(), TypeFacts::default());

    signatures.add_fallback_signatures();

    let members = signatures.member_entries.get(&object).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members["custom"].read, Some(ResolvedType::VOID));
    let facts = signatures.type_facts.get(&string).unwrap();
    assert!(facts.fallback_type_parameters.is_none());
    assert!(facts.supertypes.is_none());
    let list = DeclarationIdentity::sdk("dart:core", "List");
    assert_eq!(
        signatures.member_entries.get(&list).unwrap()["add"]
            .signature
            .as_ref()
            .unwrap()
            .return_type,
        ResolvedType::VOID,
    );
    assert_eq!(
        signatures.implements_sdk(
            &ResolvedType::interface_identity(DeclarationIdentity::sdk("dart:core", "int")),
            "dart:core",
            "num",
        ),
        Truth::Yes,
    );
}

#[test]
fn fallback_merge_covers_shared_maps_with_each_key_type() {
    with_model("", |_, model| {
        let mut signatures = SignatureIndex::default();
        let mut fallback = SignatureIndex::default();
        let print = DeclarationIdentity::sdk("dart:core", "print");
        let identical = DeclarationIdentity::sdk("dart:core", "identical");
        signatures.functions.insert(
            print.clone(),
            ResolvedSignature::returning(ResolvedType::UNKNOWN),
        );
        for identity in [print.clone(), identical.clone()] {
            fallback
                .functions
                .insert(identity, ResolvedSignature::returning(ResolvedType::VOID));
        }
        let library = model.library.unwrap();
        fallback
            .blocked_field_promotions
            .insert(library, FxHashSet::from_iter(["_field".to_string()]));
        let existing = sdk_parameter("List", 0);
        let missing = sdk_parameter("Map", 0);
        signatures
            .owner_defaults
            .insert(existing.clone(), ResolvedType::UNKNOWN);
        signatures.bound_declared.insert(existing.clone(), false);
        for parameter in [existing.clone(), missing.clone()] {
            fallback
                .owner_defaults
                .insert(parameter.clone(), ResolvedType::DYNAMIC);
            fallback.bound_declared.insert(parameter, true);
        }

        signatures.merge_fallback_signatures(fallback);

        assert_eq!(
            signatures.functions.get(&print).unwrap().return_type,
            ResolvedType::UNKNOWN,
        );
        assert_eq!(
            signatures.functions.get(&identical).unwrap().return_type,
            ResolvedType::VOID,
        );
        assert!(
            signatures
                .blocked_field_promotions
                .get(&library)
                .unwrap()
                .contains("_field"),
        );
        assert_eq!(
            signatures.owner_defaults.get(&existing),
            Some(&ResolvedType::UNKNOWN),
        );
        assert_eq!(
            signatures.owner_defaults.get(&missing),
            Some(&ResolvedType::DYNAMIC),
        );
        assert_eq!(signatures.bound_declared.get(&existing), Some(&false));
        assert_eq!(signatures.bound_declared.get(&missing), Some(&true));
    });
}

#[test]
fn sdk_identities_remain_canonical_across_plain_and_embedded_contexts() {
    let directory = tempfile::tempdir().unwrap();
    let root = &directory.path().canonicalize().unwrap();
    let sdk = root.join("flutter/bin/cache/dart-sdk/lib");
    let embedded = root.join("embedder/lib");
    std::fs::create_dir_all(&sdk).unwrap();
    std::fs::create_dir_all(&embedded).unwrap();
    std::fs::write(
        sdk.join("libraries.json"),
        r#"{"vm":{"libraries":{"core":{"uri":"core.dart"},"async":{"uri":"async.dart"}}}}"#,
    )
    .unwrap();
    std::fs::write(
        embedded.join("_embedder.yaml"),
        "embedded_libs:\n  dart:core: core.dart\n  dart:async: async.dart\n",
    )
    .unwrap();
    for (lib, parameter) in [(&sdk, "int"), (&embedded, "String")] {
        let operator = if parameter == "int" {
            "bool operator ==(Object other) => true;"
        } else {
            "bool operator ==(Object other);"
        };
        std::fs::write(
            lib.join("core.dart"),
            format!("class Object {{}} class num {{}} class int extends num {{}} \
             class String {{}} class Type {{}} class Error {{}} class Exception {{}} \
             class ArgumentError extends Error {{ ArgumentError(String message); }} \
             class StateError extends Error {{ StateError(String message); }} \
             class Box<T> {{ Box(T value); }} \
             abstract class Canonical extends Box<{parameter}> {{ Canonical(super.value); {operator} }}"),
        )
        .unwrap();
        std::fs::write(
            lib.join("async.dart"),
            "class Future<T> {} class Completer<T> { Completer(); }",
        )
        .unwrap();
    }
    let paths = [
        root.join("plain/lib/main.dart"),
        root.join("embedded/lib/main.dart"),
    ];
    let source = "import 'dart:async'; import 'package:dependency/dependency.dart'; final error = ArgumentError('x'); \
                  final state = StateError('x'); int number = 0; String text = ''; \
                  final done = Completer<void>(); class LocalError extends Error {} \
                  class NotAnError {}";
    for (index, path) in paths.iter().enumerate() {
        let app = path.parent().unwrap().parent().unwrap();
        std::fs::create_dir_all(app.join(".dart_tool")).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::create_dir_all(app.join("dependency/lib")).unwrap();
        let parameter = if index == 0 { "int" } else { "String" };
        std::fs::write(app.join("dependency/lib/dependency.dart"), format!(
            "class PackageBox<T> {{ PackageBox(T value); }} \
             class PackageCanonical extends PackageBox<{parameter}> {{ PackageCanonical(super.value); }}"
        )).unwrap();
        let packages = if index == 0 {
            serde_json::json!([{"name":"dependency","rootUri":"../dependency","packageUri":"lib/"}])
        } else {
            serde_json::json!([
                {"name":"sky_engine","rootUri":"../../embedder","packageUri":"lib/"},
                {"name":"dependency","rootUri":"../dependency","packageUri":"lib/"}
            ])
        };
        std::fs::write(
            app.join(".dart_tool/package_config.json"),
            serde_json::json!({
                "configVersion": 2,
                "flutterRoot": format!("file://{}", root.join("flutter").display()),
                "packages": packages
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(path, source).unwrap();
    }
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let parsed = Arc::new(crate::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    assert!(errors.is_empty(), "{errors:?}");
    let environment = crate::AnalysisEnvironment::discover_with_sdk_locator(
        paths.iter().map(PathBuf::as_path),
        |config| {
            crate::environment::DartSdk::load(
                &config?.flutter_root.as_ref()?.join("bin/cache/dart-sdk"),
            )
        },
    );
    let project: Vec<_> = paths
        .iter()
        .map(|path| (path.as_path(), program, source))
        .collect();
    let external = ExternalLibraries::load(&environment, &project, |path| {
        paths.iter().any(|project| project == path)
    });
    assert_eq!(
        external.libraries().iter().filter(|library| {
            matches!(&library.origin, crate::environment::ExternalOrigin::Sdk(uri) if uri.as_str() == "dart:core")
        }).count(),
        2,
    );
    let files: Vec<_> = paths
        .iter()
        .map(|path| (path.clone(), parsed.clone()))
        .collect();
    let sources: Vec<_> = paths
        .iter()
        .map(|path| IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        })
        .collect();
    let external_sources = crate::ExternalSources {
        libraries: &external,
        environment: &environment,
    };
    let monolithic = IdentityIndex::build(&sources, &[source, source], &[], Some(external_sources));
    let base = Arc::new(IdentityIndex::build_external(external_sources));
    let overlay = IdentityIndex::with_external(
        &sources,
        &[source, source],
        &[],
        external_sources,
        base.clone(),
    );
    for (identities, external_identities) in [(&monolithic, None), (&overlay, Some(base.as_ref()))]
    {
        let mut types = TypeIndex::from_resolved_files(
            paths.iter().map(|path| {
                (
                    path.as_path(),
                    LibrarySource {
                        program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            identities,
        );
        types.add_external(&external, identities);
        let signatures = if let Some(external_identities) = external_identities {
            let external_signatures = Arc::new(SignatureIndex::build(
                &[],
                Some(&external),
                external_identities,
                &types,
            ));
            SignatureIndex::with_external(&files, identities, &types, external_signatures)
        } else {
            SignatureIndex::build(&files, Some(&external), identities, &types)
        };
        for path in &paths {
            let model = SemanticModel::new(path, &parsed, identities, Some(&types));
            for name in ["Canonical", "PackageCanonical"] {
                let identity = model.resolve_name(name).unwrap();
                let library = identities.declaration_library(&identity).unwrap();
                let file = identities.declaration_file(library, name).unwrap();
                let source = identities.source_path(file).unwrap();
                let parameter =
                    if source.starts_with(&embedded) || source.starts_with(root.join("embedded")) {
                        "String"
                    } else {
                        "int"
                    };
                assert_eq!(
                    signatures.constructor(&identity, "new").unwrap().positional,
                    [model.core_type(parameter, false)],
                    "{identity:?} from {source:?}",
                );
                if name == "Canonical" {
                    let facts = signatures.declared_member_facts(&identity, "==");
                    assert_eq!(facts.len(), 1);
                    assert_eq!(facts[0].is_abstract, parameter == "String");
                }
            }
            for (name, library) in [
                ("int", "dart:core"),
                ("String", "dart:core"),
                ("Type", "dart:core"),
                ("Completer", "dart:async"),
            ] {
                assert_eq!(
                    model.resolve_name(name),
                    Some(DeclarationIdentity::sdk(library, name))
                );
            }
            for name in ["error", "state"] {
                let ty = signatures
                    .value_type(&model.resolve_value(name).unwrap())
                    .unwrap();
                assert_eq!(
                    signatures.interface_check_sdk(ty, "dart:core", "Error", &model),
                    Truth::Yes
                );
            }
            for name in ["number", "text", "done", "NotAnError"] {
                let ty = model
                    .resolve_value(name)
                    .and_then(|id| signatures.value_type(&id).cloned())
                    .unwrap_or_else(|| model.declared_type(&model.resolve_name(name).unwrap()));
                assert_eq!(
                    signatures.interface_check_sdk(&ty, "dart:core", "Error", &model),
                    Truth::No
                );
            }
            let number = signatures
                .value_type(&model.resolve_value("number").unwrap())
                .unwrap();
            let text = signatures
                .value_type(&model.resolve_value("text").unwrap())
                .unwrap();
            assert_eq!(
                signatures.relation(
                    number,
                    crate::TypeRelation::RuntimeUnrelatedTo(&model.core_type("int", false)),
                    &model
                ),
                Truth::No
            );
            assert_eq!(
                signatures.relation(
                    text,
                    crate::TypeRelation::RuntimeUnrelatedTo(&model.core_type("String", false)),
                    &model
                ),
                Truth::No
            );
            assert_eq!(
                signatures.relation(
                    number,
                    crate::TypeRelation::RuntimeUnrelatedTo(text),
                    &model
                ),
                Truth::Yes
            );
            let done = signatures
                .value_type(&model.resolve_value("done").unwrap())
                .unwrap();
            assert_eq!(
                signatures.interface_check_sdk(done, "dart:async", "Completer", &model),
                Truth::Yes
            );
            let local = model.declared_type(&model.resolve_name("LocalError").unwrap());
            assert_eq!(
                signatures.interface_check_sdk(&local, "dart:core", "Error", &model),
                Truth::Yes
            );
        }
    }
}

#[test]
fn single_file_and_project_inference_share_converged_headers_and_values() {
    with_model(
        r#"
class Root { final int value; Root(this.value); }
class Middle extends Root { Middle(super.value); }
class Leaf extends Middle { Leaf(super.value); }
class Base<T> { T get value => throw 0; T method(T input) => input; }
class Child extends Base<int> {
  get value => 1;
  method(input) => input;
}
final first = second;
final second = third;
final third = Child().value;
final partial = [first, unresolved];
"#,
        |program, model| {
            let single = SignatureIndex::from_program(program, model);
            let project = SignatureIndex::from_project_files(
                &[(
                    PathBuf::from("/project/lib/main.dart"),
                    model.node_ref(program.syntax()).file,
                )],
                model.identities,
                model.types.unwrap(),
            );
            for name in ["first", "second", "third", "partial"] {
                let identity = model.resolve_value(name).unwrap();
                let actual = single.value_type(&identity).unwrap();
                assert_eq!(Some(actual), project.value_type(&identity));
                if name == "partial" {
                    assert!(actual.interface("dart:core", "List"));
                    assert!(has_inference_holes(actual));
                } else {
                    assert!(actual.interface("dart:core", "int"));
                }
            }
            for name in ["Root", "Middle", "Leaf"] {
                let identity = model.resolve_name(name).unwrap();
                let actual = single.constructor(&identity, "new").unwrap();
                assert_eq!(
                    actual.function_type(),
                    project
                        .constructor(&identity, "new")
                        .unwrap()
                        .function_type(),
                );
                assert!(actual.positional[0].interface("dart:core", "int"));
            }
        },
    );
}

#[test]
fn extension_setter_lookup_borrows_indexed_spelling_through_overlays() {
    let mut setters = FxHashMap::default();
    setters.insert(ExtensionMemberName("value=".to_owned()), vec![0]);
    let mut members = ExtensionMembers::default();
    members[1] = setters;
    let base = SignatureIndex {
        extension_members: Some(members),
        ..SignatureIndex::default()
    };
    let overlay = SignatureIndex {
        extension_members: Some(ExtensionMembers::default()),
        external: Some(Arc::new(base)),
        ..SignatureIndex::default()
    };
    let spelling = overlay.extension_access_name("value", true).unwrap();
    assert_eq!(spelling, "value=");
    let stored = &overlay
        .external
        .as_ref()
        .unwrap()
        .extension_members
        .as_ref()
        .unwrap()[1]
        .get_key_value("value")
        .unwrap()
        .0
        .0;
    assert_eq!(spelling.as_ptr(), stored.as_ptr());
    assert_eq!(overlay.extension_access_name("value", false), Some("value"));
    assert_eq!(overlay.extension_access_name("missing", true), None);
}

#[test]
fn extension_setter_lookup_borrows_unindexed_stored_spelling() {
    with_model(
        "class C {} extension E on C { set value(int next) {} }",
        |program, model| {
            let mut signatures = SignatureIndex::from_program(program, model);
            signatures.extension_members = None;
            let spelling = signatures.extension_access_name("value", true).unwrap();
            let stored = signatures.extensions[0]
                .instance_members
                .get("value=")
                .unwrap();
            assert_eq!(spelling, "value=");
            assert_eq!(spelling.as_ptr(), stored.as_ptr());
            assert_eq!(
                signatures.extension_access_name("value", false),
                Some("value")
            );
            assert_eq!(signatures.extension_access_name("missing", true), None);
        },
    );
}

#[test]
fn external_overlays_share_facts_and_replace_project_snapshots() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("app/.dart_tool")).unwrap();
    std::fs::create_dir_all(root.path().join("app/lib")).unwrap();
    std::fs::create_dir_all(root.path().join("dep/lib")).unwrap();
    std::fs::write(
        root.path().join("app/.dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[
            {"name":"app","rootUri":"../","packageUri":"lib/"},
            {"name":"dep","rootUri":"../../dep","packageUri":"lib/"}
        ]}"#,
    )
    .unwrap();
    let path = root.path().join("app/lib/main.dart");
    let external_path = root.path().join("dep/lib/external.dart");
    std::fs::write(
        &external_path,
        "class Base<T> { T field; Base(this.field); T method() => field; } \
         int get answer => 1; int imported(int x) => x; \
         extension Tools on Base<int> { int tool() => 1; int implicitTool() => 1; } \
         extension Other on Base<int> { String tool() => ''; } \
         extension GenericTools<T> on Base<T> { T chosen() => field; R choose<R>(R value) => value; } \
         enum Choice { first, second }",
    )
    .unwrap();
    let external_path = std::fs::canonicalize(&external_path).unwrap();
    let old_source = "import 'package:dep/external.dart'; \
        class Child extends Base<int> { Child(): super(1); } \
        final oldValue = answer; final result = Child().method(); \
        final viaExtension = Base<int>(1).implicitTool(); \
        final viaExplicit = Tools(Base<int>(1)).tool(); \
        final viaGeneric = GenericTools<int>(Base<int>(1)).chosen(); \
        final viaGenericMethod = GenericTools(Base<int>(1)).choose<String>('value'); \
        Base<int>? maybeBase; \
        final viaNullable = GenericTools(maybeBase)?.chosen(); \
        final viaNonNull = GenericTools(Base<int>(1))?.chosen(); \
        final viaInferredMethod = GenericTools(Base<int>(1)).choose('value');";
    let new_source = "import 'package:dep/external.dart'; \
        class Child extends Base<String> { Child(): super('x'); } \
        final freshValue = answer; final result = Child().method(); \
        final viaExplicit = Tools(Base<int>(1)).tool(); \
        final viaGeneric = GenericTools<int>(Base<int>(1)).chosen(); \
        final viaGenericMethod = GenericTools(Base<int>(1)).choose<String>('value'); \
        Base<int>? maybeBase; \
        final viaNullable = GenericTools(maybeBase)?.chosen(); \
        final viaNonNull = GenericTools(Base<int>(1))?.chosen(); \
        final viaInferredMethod = GenericTools(Base<int>(1)).choose('value');";
    let falcon_dart_parser::ParseResult {
        tree: old_program_tree,
        errors,
    } = parse(old_source);
    let old_program_parsed = Arc::new(crate::ParsedFile {
        source: old_source.into(),
        tree: old_program_tree,
    });
    let old_program = old_program_parsed.program();
    assert!(errors.is_empty(), "{errors:?}");
    let falcon_dart_parser::ParseResult {
        tree: new_program_tree,
        errors,
    } = parse(new_source);
    let new_program_parsed = Arc::new(crate::ParsedFile {
        source: new_source.into(),
        tree: new_program_tree,
    });
    assert!(errors.is_empty(), "{errors:?}");
    std::fs::write(&path, old_source).unwrap();
    let environment =
        crate::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
    let external = ExternalLibraries::load(
        &environment,
        &[(path.as_path(), old_program, old_source)],
        // Only this package file belongs to the fixture's external graph.
        |candidate| candidate != external_path,
    );
    assert_eq!(external.libraries().len(), 1);
    let external_sources = || crate::ExternalSources {
        libraries: &external,
        environment: &environment,
    };
    let base_ids = Arc::new(IdentityIndex::build_external(external_sources()));
    let mut base_types = TypeIndex::from_resolved_files([], &base_ids);
    base_types.add_external(&external, &base_ids);
    let base_types = Arc::new(base_types);
    let base_signatures = Arc::new(SignatureIndex::build(
        &[],
        Some(&external),
        &base_ids,
        &base_types,
    ));
    let mut old_identity = None;
    for (source, parsed, expected) in [
        (old_source, &old_program_parsed, "int"),
        (new_source, &new_program_parsed, "String"),
    ] {
        let program = parsed.program();
        let identities = IdentityIndex::with_external(
            &[IdentitySource {
                path: &path,
                parsed,
                has_parse_errors: false,
            }],
            &[source],
            &[],
            external_sources(),
            Arc::clone(&base_ids),
        );
        let types = TypeIndex::with_external(
            [(
                path.as_path(),
                LibrarySource {
                    program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
            Arc::clone(&base_types),
        );
        let signatures = SignatureIndex::with_external(
            &[(path.clone(), parsed.clone())],
            &identities,
            &types,
            Arc::clone(&base_signatures),
        );
        let model = SemanticModel::new(&path, parsed, &identities, Some(&types));
        let base = model.resolve_name("Base").unwrap();
        let child = model.resolve_name("Child").unwrap();
        assert_eq!(types.type_kind_identity(&base), Some(TypeKind::Class));
        assert_eq!(
            types.supertype_matches_identity(&child, |identity| identity.matches(&base)),
            Truth::Yes
        );
        assert_eq!(
            types.member_lookup_identity(&child, "method"),
            crate::resolve::MemberResult::Found(MemberKind::Method),
        );
        let imported = model.resolve_value("imported").unwrap();
        assert!(std::ptr::eq(
            signatures.function(&imported).unwrap(),
            base_signatures.function(&imported).unwrap(),
        ));
        assert!(std::ptr::eq(
            signatures.constructor(&base, "new").unwrap(),
            base_signatures.constructor(&base, "new").unwrap(),
        ));
        assert!(signatures.is_value_getter(&model.resolve_value("answer").unwrap()));
        assert!(
            signatures
                .value_type(&model.resolve_value("result").unwrap())
                .unwrap()
                .interface("dart:core", expected)
        );
        let choice = model.resolve_name("Choice").unwrap();
        assert!(signatures.is_enum_value(&choice, "first"));
        let tools = model.resolve_extension("Tools").unwrap();
        assert!(signatures.extension_on_type(&tools).is_some());
        assert!(
            signatures
                .extension_override_method(&tools, "tool")
                .is_some()
        );
        let (resolved, _) = signatures
            .resolved_extension_override_method(
                &tools,
                signatures.extension_on_type(&tools).unwrap(),
                &[],
                "tool",
                &model,
            )
            .unwrap();
        assert!(resolved.return_type.interface("dart:core", "int"));
        assert!(
            signatures
                .value_type(&model.resolve_value("viaExplicit").unwrap())
                .unwrap()
                .interface("dart:core", "int")
        );
        assert!(
            signatures
                .value_type(&model.resolve_value("viaGeneric").unwrap())
                .unwrap()
                .interface("dart:core", "int")
        );
        assert!(
            signatures
                .value_type(&model.resolve_value("viaGenericMethod").unwrap())
                .unwrap()
                .interface("dart:core", "String")
        );
        for name in ["viaNullable", "viaNonNull"] {
            let ty = signatures
                .value_type(&model.resolve_value(name).unwrap())
                .unwrap();
            assert!(ty.interface("dart:core", "int"), "{name}: {ty:?}");
            assert!(ty.nullable(), "{name}: {ty:?}");
        }
        let inferred = signatures
            .value_type(&model.resolve_value("viaInferredMethod").unwrap())
            .unwrap();
        assert!(inferred.interface("dart:core", "String"));
        assert!(!inferred.nullable());
        if let Some(identity) = &old_identity {
            assert!(signatures.value_type(identity).is_none());
            assert!(
                signatures
                    .value_type(&model.resolve_value("freshValue").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
        } else {
            old_identity = Some(model.resolve_value("oldValue").unwrap());
            assert!(
                signatures
                    .value_type(&model.resolve_value("viaExtension").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
        }
        let empty_types = TypeIndex::with_external([], &identities, Arc::clone(&base_types));
        assert_eq!(empty_types.type_kind_identity(&base), Some(TypeKind::Class));
        assert!(Arc::ptr_eq(
            signatures.external.as_ref().unwrap(),
            &base_signatures
        ));
    }
}

#[test]
fn type_facts_overlays_shadow_whole_records_without_mutating_the_base() {
    let identity = DeclarationIdentity::sdk("dart:core", "Choice");
    let mut external = SignatureIndex::default();
    external.type_facts.insert(
        identity.clone(),
        TypeFacts {
            supertypes: Some((
                Vec::new(),
                vec![ResolvedType::interface_identity(
                    DeclarationIdentity::OBJECT,
                )],
            )),
            enum_values: Some(vec!["first".to_string()]),
            kind: Some(TypeKind::Enum),
            immutable: Some(Truth::Yes),
            constructors: FxHashMap::from_iter([(
                "new".to_string(),
                ResolvedSignature::returning(ResolvedType::DYNAMIC),
            )]),
            ..TypeFacts::default()
        },
    );
    external.finish();
    let external = Arc::new(external);
    let untouched = SignatureIndex::borrowing_external(Arc::clone(&external));
    let mut overlay = SignatureIndex::borrowing_external(Arc::clone(&external));
    assert!(std::ptr::eq(
        overlay.type_facts.get(&identity).unwrap(),
        external.type_facts.get(&identity).unwrap(),
    ));
    overlay.type_facts.insert(
        identity.clone(),
        TypeFacts {
            kind: Some(TypeKind::Class),
            ..TypeFacts::default()
        },
    );
    assert_eq!(overlay.type_kind(&identity), Some(TypeKind::Class));
    assert_eq!(overlay.enum_value_names(&identity), None);
    assert_eq!(overlay.type_parameters(&identity), None);
    assert!(overlay.constructor(&identity, "new").is_none());
    assert!(!overlay.has_cyclic_supertypes(&identity));
    assert_eq!(untouched.enum_value_names(&identity).unwrap(), ["first"]);
    assert_eq!(external.type_kind(&identity), Some(TypeKind::Enum));
    assert!(external.constructor(&identity, "new").is_some());
    assert_eq!(overlay.type_facts.iter().count(), 1);
}

#[test]
fn signature_map_iteration_preserves_base_and_project_shadowing() {
    let shared = DeclarationIdentity::sdk("dart:core", "shared");
    let external_only = DeclarationIdentity::sdk("dart:core", "externalOnly");
    let project_only = DeclarationIdentity::sdk("dart:core", "projectOnly");
    let mut external = SignatureIndex::default();
    external
        .value_types
        .insert(shared.clone(), ResolvedType::DYNAMIC);
    external
        .value_types
        .insert(external_only.clone(), ResolvedType::VOID);
    let external = Arc::new(external);
    let mut overlay = SignatureMap {
        external: Some((Arc::clone(&external), |base| &base.value_types)),
        ..SignatureMap::default()
    };
    overlay.insert(shared.clone(), ResolvedType::NEVER);
    overlay.insert(project_only.clone(), ResolvedType::NULL);
    assert_eq!(overlay.get(&shared), Some(&ResolvedType::NEVER));
    assert_eq!(overlay.get(&external_only), Some(&ResolvedType::VOID));
    assert!(overlay.contains_key(&external_only));
    assert_eq!(overlay.get(&project_only), Some(&ResolvedType::NULL));
    assert_eq!(overlay.local.len(), 2);
    let entries: FxHashMap<_, _> = overlay
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries.get(&shared), Some(&ResolvedType::NEVER));
    assert_eq!(entries.get(&external_only), Some(&ResolvedType::VOID));
    assert_eq!(entries.get(&project_only), Some(&ResolvedType::NULL));
    assert_eq!(external.value_type(&shared), Some(&ResolvedType::DYNAMIC));
}

#[test]
fn typedef_provenance_preserves_resolved_type_size() {
    // The semantic type layout before function typedef metadata was added.
    #[allow(dead_code)]
    enum BeforeTypedefProvenance {
        Unknown,
        Dynamic,
        Void,
        Never,
        Null,
        Interface {
            identity: DeclarationIdentity,
            arguments: Vec<BeforeTypedefProvenance>,
            nullable: bool,
            extension_type: bool,
        },
        TypeParameter {
            id: TypeParameterId,
            bound: Box<BeforeTypedefProvenance>,
            nullable: bool,
        },
        NonNullTypeParameter {
            id: TypeParameterId,
            bound: Box<BeforeTypedefProvenance>,
        },
        Function {
            return_type: Box<BeforeTypedefProvenance>,
            positional: Vec<BeforeTypedefProvenance>,
            named: Vec<(String, BeforeTypedefProvenance)>,
            nullable: bool,
        },
        Record {
            positional: Vec<BeforeTypedefProvenance>,
            named: Vec<(String, BeforeTypedefProvenance)>,
            nullable: bool,
        },
    }

    assert_eq!(
        std::mem::size_of::<FunctionTypeAlias>(),
        std::mem::size_of::<usize>(),
    );
    assert!(std::mem::size_of::<ResolvedType>() <= std::mem::size_of::<BeforeTypedefProvenance>(),);
}

#[test]
fn no_op_flow_mutations_preserve_shared_scopes() {
    let mut environment = TypeEnvironment::new();
    environment.declare_receiver(ResolvedType::DYNAMIC);
    environment.declare("plain", ResolvedType::DYNAMIC);
    environment.declare("value", ResolvedType::DYNAMIC);
    environment.declare_unpromotable("blocked", ResolvedType::DYNAMIC);
    environment.promote("value", ResolvedType::NULL);
    environment.promote_this_field("synthetic", ResolvedType::NULL);
    environment.enter_callable();
    environment.record_write("blocked");
    environment.mark_captured("blocked");
    let snapshot = environment.clone();

    environment.promote("plain", ResolvedType::UNKNOWN);
    environment.promote("value", ResolvedType::NULL);
    environment.promote("blocked", ResolvedType::NULL);
    environment.promote("missing", ResolvedType::NULL);
    environment.forget("plain");
    environment.forget("missing");
    environment.promote_this_field("blocked", ResolvedType::NULL);
    environment.promote_this_field("value", ResolvedType::NULL);
    environment.promote_this_field("synthetic", ResolvedType::NULL);
    environment.record_write("blocked");
    environment.mark_captured("blocked");
    environment.attach_finally(&snapshot, &snapshot);
    assert!(Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
    assert_eq!(environment.lookup("value"), ResolvedType::NULL);
    assert_eq!(environment.lookup("plain"), ResolvedType::DYNAMIC);
}

#[test]
fn flow_snapshots_copy_only_written_scopes() {
    let mut environment = TypeEnvironment::new();
    environment.declare("outer", ResolvedType::DYNAMIC);
    environment.push_scope();
    environment.declare("inner", ResolvedType::DYNAMIC);
    let snapshot = environment.clone();
    assert!(Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
    assert!(Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));

    environment.promote("outer", ResolvedType::NULL);
    assert_eq!(snapshot.lookup("outer"), ResolvedType::DYNAMIC);
    assert_eq!(environment.lookup("outer"), ResolvedType::NULL);
    assert!(!Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
    assert!(Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));

    environment.declare("inner", ResolvedType::NULL);
    assert_eq!(snapshot.lookup("inner"), ResolvedType::DYNAMIC);
    assert!(!Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));
    let joined = environment.join(&snapshot);
    assert_eq!(joined.lookup("outer"), ResolvedType::DYNAMIC);
    assert_eq!(environment.lookup("outer"), ResolvedType::NULL);
}

#[test]
fn nearest_flow_binding_preserves_declaration_identity_and_snapshots() {
    let root = Span::new(1, 6);
    let inner = Span::new(10, 15);
    let mut environment = TypeEnvironment::new();
    environment.declare_at("value", ResolvedType::DYNAMIC, root);
    assert!(environment.is_bound_in_root_scope("value"));
    environment.push_scope();
    environment.declare_at("value", ResolvedType::DYNAMIC, inner);
    assert_eq!(environment.binding_depth("value"), Some(1));
    assert_eq!(environment.binding_declaration("value"), Some(inner));
    assert!(!environment.is_bound_in_root_scope("value"));
    let snapshot = environment.clone();

    environment.promote("value", ResolvedType::NULL);
    assert!(Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
    assert!(!Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));
    assert_eq!(snapshot.lookup("value"), ResolvedType::DYNAMIC);
    assert_eq!(snapshot.binding_declaration("value"), Some(inner));
    let joined = environment.join(&snapshot);
    assert_eq!(joined.lookup("value"), ResolvedType::DYNAMIC);
    assert_eq!(joined.binding_declaration("value"), Some(inner));
    assert!(!joined.is_bound_in_root_scope("value"));

    let mut target = snapshot.clone();
    target.attach_finally(&snapshot, &environment);
    assert_eq!(target.lookup("value"), ResolvedType::NULL);
    assert_eq!(target.binding_declaration("value"), Some(inner));
    assert!(Arc::ptr_eq(&target.scopes[0], &snapshot.scopes[0]));
    target.pop_scope();
    assert_eq!(target.lookup("value"), ResolvedType::DYNAMIC);
    assert_eq!(target.binding_declaration("value"), Some(root));
    assert!(target.is_bound_in_root_scope("value"));
}

#[test]
fn missing_flow_mutations_preserve_every_shared_scope() {
    let mut environment = TypeEnvironment::new();
    environment.declare("outer", ResolvedType::DYNAMIC);
    environment.enter_callable();
    environment.push_scope();
    environment.declare("inner", ResolvedType::DYNAMIC);
    let snapshot = environment.clone();

    environment.assign("missing", Vec::new());
    environment.promote("missing", ResolvedType::NULL);
    environment.forget("missing");
    environment.mark_captured("missing");
    environment.record_write("missing");
    environment.promote_this_field("missing", ResolvedType::NULL);
    assert!(environment.same_flow_state(&snapshot));
    for (scope, original) in environment.scopes.iter().zip(snapshot.scopes.iter()) {
        assert!(Arc::ptr_eq(scope, original));
    }
}

#[test]
fn this_promotions_copy_only_the_nearest_receiver_scope() {
    let mut environment = TypeEnvironment::new();
    environment.declare_receiver(ResolvedType::DYNAMIC);
    environment.promote_this_field("field", ResolvedType::DYNAMIC);
    environment.push_scope();
    environment.declare_receiver(ResolvedType::NULL);
    assert_eq!(environment.receiver(), ResolvedType::NULL);
    environment.push_scope();
    environment.declare("field", ResolvedType::DYNAMIC);
    let snapshot = environment.clone();

    environment.promote_this_field("field", ResolvedType::NULL);
    assert_eq!(
        environment.promoted_this_field("field"),
        Some(ResolvedType::NULL)
    );
    assert_eq!(snapshot.promoted_this_field("field"), None);
    assert_eq!(environment.lookup("field"), ResolvedType::DYNAMIC);
    assert!(Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
    assert!(!Arc::ptr_eq(&environment.scopes[1], &snapshot.scopes[1]));
    assert!(Arc::ptr_eq(&environment.scopes[2], &snapshot.scopes[2]));
    environment.pop_scope();
    environment.pop_scope();
    assert_eq!(environment.receiver(), ResolvedType::DYNAMIC);
    assert_eq!(
        environment.promoted_this_field("field"),
        Some(ResolvedType::DYNAMIC)
    );
}

#[test]
fn receiver_slot_is_not_a_named_binding() {
    let mut environment = TypeEnvironment::new();
    environment.declare_receiver(ResolvedType::DYNAMIC);
    environment.promote_this_field("field", ResolvedType::NULL);
    assert!(!environment.is_bound("this"));
    assert_eq!(environment.binding_depth("this"), None);
    assert_eq!(environment.binding_declaration("this"), None);
    assert_eq!(environment.lookup("this"), ResolvedType::UNKNOWN);
    assert_eq!(environment.declared_type("this"), ResolvedType::UNKNOWN);
    let snapshot = environment.clone();
    environment.enter_callable();
    assert!(!environment.assign("this", vec![ResolvedType::NULL]));
    environment.promote("this", ResolvedType::NULL);
    environment.demote("this");
    environment.mark_captured("this");
    environment.demote_captured();
    assert!(Arc::ptr_eq(&environment.scopes[0], &snapshot.scopes[0]));
    assert_eq!(environment.receiver(), ResolvedType::DYNAMIC);
    assert_eq!(
        environment.promoted_this_field("field"),
        Some(ResolvedType::NULL)
    );

    environment.declare("this", ResolvedType::NULL);
    environment.mark_captured("this");
    assert_eq!(environment.lookup("this"), ResolvedType::NULL);
    assert_eq!(environment.receiver(), ResolvedType::DYNAMIC);
    assert_eq!(
        environment.promoted_this_field("field"),
        Some(ResolvedType::NULL)
    );
}

#[test]
fn this_field_promotions_prefer_bindings_in_the_receiver_scope() {
    let mut environment = TypeEnvironment::new();
    environment.declare_receiver(ResolvedType::DYNAMIC);
    environment.promote_this_field("field", ResolvedType::DYNAMIC);
    environment.declare("field", ResolvedType::DYNAMIC);
    assert_eq!(environment.promoted_this_field("field"), None);
    environment.push_scope();
    environment.declare("field", ResolvedType::DYNAMIC);
    environment.promote_this_field("field", ResolvedType::NULL);
    assert_eq!(environment.lookup("field"), ResolvedType::DYNAMIC);
    assert_eq!(
        environment.promoted_this_field("field"),
        Some(ResolvedType::NULL)
    );
    environment.pop_scope();
    assert_eq!(environment.lookup("field"), ResolvedType::NULL);
}

#[test]
fn receiver_promotions_join_and_finally_transfer_independently() {
    let mut entry = TypeEnvironment::new();
    entry.declare_receiver(ResolvedType::DYNAMIC);
    entry.promote_this_field("unchanged", ResolvedType::DYNAMIC);
    entry.push_scope();
    entry.declare("local", ResolvedType::DYNAMIC);
    let mut left = entry.clone();
    let mut right = entry.clone();
    left.promote_this_field("shared", ResolvedType::NULL);
    right.promote_this_field("shared", ResolvedType::NULL);
    left.promote_this_field("left", ResolvedType::NULL);
    right.promote_this_field("right", ResolvedType::NULL);
    let joined = left.join(&right);
    assert_eq!(joined.receiver(), ResolvedType::DYNAMIC);
    assert_eq!(
        joined.promoted_this_field("shared"),
        Some(ResolvedType::NULL)
    );
    assert_eq!(joined.promoted_this_field("left"), None);
    assert_eq!(joined.promoted_this_field("right"), None);
    assert!(Arc::ptr_eq(&joined.scopes[1], &entry.scopes[1]));

    let mut target = left.clone();
    target.attach_finally(&entry, &right);
    assert_eq!(
        target.promoted_this_field("shared"),
        Some(ResolvedType::NULL)
    );
    assert_eq!(target.promoted_this_field("left"), None);
    assert_eq!(
        target.promoted_this_field("right"),
        Some(ResolvedType::NULL)
    );
    assert_eq!(
        target.promoted_this_field("unchanged"),
        Some(ResolvedType::DYNAMIC)
    );
    assert!(Arc::ptr_eq(&target.scopes[1], &entry.scopes[1]));
    assert_eq!(entry.promoted_this_field("shared"), None);
}

#[test]
fn receiver_no_ops_preserve_scope_snapshots() {
    let mut entry = TypeEnvironment::new();
    entry.declare_receiver(ResolvedType::DYNAMIC);
    let snapshot = entry.clone();
    entry.declare_receiver(ResolvedType::DYNAMIC);
    assert!(entry.same_flow_state(&snapshot));

    let mut left = entry.clone();
    let mut right = entry.clone();
    left.promote_this_field("field", ResolvedType::NULL);
    right.promote_this_field("field", ResolvedType::NULL);
    let snapshot = left.clone();
    left.promote_this_field("field", ResolvedType::NULL);
    left.attach_finally(&entry, &right);
    assert!(left.same_flow_state(&snapshot));
    assert!(left.join(&right).same_flow_state(&snapshot));
    assert!(left.join(&left).same_flow_state(&snapshot));
}

#[test]
fn receiver_join_with_missing_peer_preserves_type_without_promotions() {
    let mut environment = TypeEnvironment::new();
    environment.declare_receiver(ResolvedType::DYNAMIC);
    environment.promote_this_field("field", ResolvedType::NULL);
    let joined = environment.join(&TypeEnvironment::new());
    assert_eq!(joined.receiver(), ResolvedType::DYNAMIC);
    assert_eq!(joined.promoted_this_field("field"), None);
    assert_eq!(
        environment.promoted_this_field("field"),
        Some(ResolvedType::NULL)
    );
}

#[test]
fn finally_transfer_preserves_receiver_presence_and_snapshot_sharing() {
    let missing = TypeEnvironment::new();
    let mut present = TypeEnvironment::new();
    present.declare_receiver(ResolvedType::DYNAMIC);
    present.promote_this_field("field", ResolvedType::NULL);

    let mut target = present.clone();
    target.attach_finally(&present, &missing);
    assert!(target.same_flow_state(&present));
    assert_eq!(target.receiver(), ResolvedType::DYNAMIC);
    assert_eq!(
        target.promoted_this_field("field"),
        Some(ResolvedType::NULL)
    );

    let mut target = missing.clone();
    target.attach_finally(&missing, &present);
    assert!(target.same_flow_state(&missing));
    assert_eq!(target.receiver(), ResolvedType::UNKNOWN);
    assert_eq!(target.promoted_this_field("field"), None);
}

#[test]
fn shared_flow_joins_preserve_capture_demotion() {
    let mut environment = TypeEnvironment::new();
    environment.declare("value", ResolvedType::DYNAMIC);
    let unchanged = environment.join(&environment);
    assert!(Arc::ptr_eq(&environment.scopes[0], &unchanged.scopes[0]));

    environment.enter_callable();
    environment.assign("value", Vec::new());
    assert!(environment.binding("value").unwrap().captured);
    assert!(environment.binding("value").unwrap().promotable);
    let joined = environment.join(&environment);
    assert!(!joined.binding("value").unwrap().promotable);
    assert!(environment.binding("value").unwrap().promotable);
}

#[test]
fn unknown_generic_arguments_poison_constraints_in_either_order_and_nested() {
    with_model(
        "T pick<T>(T a, T b) => a; T nested<T>(List<T> a, T b) => b; \
         dynamic first(dynamic missing) => pick(missing, 1); \
         dynamic last(dynamic missing) => pick(1, missing); \
         dynamic inside(List<dynamic> missing) => nested(missing, 1); \
         T consume<T>(void Function(T) a, void Function(T) b) => throw 0; \
         void integer(int value) {} \
         dynamic contraFirst(void Function(dynamic) missing) => consume(missing, integer); \
         dynamic contraLast(void Function(dynamic) missing) => consume(integer, missing);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if !matches!(
                    function.name().name(),
                    "first" | "last" | "inside" | "contraFirst" | "contraLast"
                ) {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let hole = if function.name().name() == "inside" {
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface {
                            occurrence: Default::default(),
                            identity: DeclarationIdentity::sdk("dart:core", "List"),
                            arguments: vec![ResolvedType::UNKNOWN].into(),
                            nullable: false,
                            extension_type: false,
                        },
                    }
                } else if function.name().name().starts_with("contra") {
                    ResolvedType {
                        kind: ResolvedTypeKind::Function {
                            shape: None,
                            alias: FunctionTypeAlias::default(),
                            optional_positional: 0,
                            return_type: Arc::new(ResolvedType::VOID),
                            positional: vec![ResolvedType::UNKNOWN].into(),
                            named: Arc::from([]),
                            nullable: false,
                        },
                    }
                } else {
                    ResolvedType::UNKNOWN
                };
                let mut environment = TypeEnvironment::new();
                environment.declare("missing", hole);
                assert_eq!(
                    environment.infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default()
                    ),
                    ResolvedType::UNKNOWN,
                    "{}",
                    function.name().name(),
                );
            }
            assert_eq!(
                inference_join(
                    &ResolvedType::DYNAMIC,
                    &ResolvedType::VOID,
                    &signatures,
                    model
                ),
                ResolvedType::VOID
            );
            assert_eq!(
                inference_join(
                    &ResolvedType::VOID,
                    &ResolvedType::DYNAMIC,
                    &signatures,
                    model
                ),
                ResolvedType::VOID
            );
        },
    );
}

#[test]
fn unknown_downward_schema_does_not_poison_argument_inference() {
    with_model(
        "T pick<T>(T a, T b) => a; int use() => pick(1, 2);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap() else {
                panic!()
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!()
            };
            let expression = expression.expr();
            assert_eq!(
                TypeEnvironment::new().infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                    Some(&ResolvedType::UNKNOWN),
                    &|_| None,
                ),
                model.core_type("int", false),
            );
        },
    );
}

#[test]
fn partial_downward_schemas_leave_whole_variables_for_upward_inference() {
    with_model(
        "import 'dart:async'; \
         T identity<T>(T value) => value; \
         T boundedIdentity<T extends num>(T value) => value; \
         T empty<T>() => throw 0; \
         class StateProvider<T> {} \
         class Ref { T watch<T>(StateProvider<T> provider) => throw 0; } \
         class FutureProvider<T> { FutureProvider(FutureOr<T> Function(Ref) create); } \
         class Pair<A, B> { Pair(A first, B second); } \
         int partial() => identity(1); \
         num complete() => identity(1); \
         int bounded() => boundedIdentity(1); \
         dynamic defaulted() => empty(); \
         Pair<num, int> nested() => Pair(1, 2); \
         FutureProvider<int> provider(StateProvider<int> dep) => \
             FutureProvider((ref) async => ref.watch(dep));",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let partial = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:async", "FutureOr"),
                    arguments: vec![ResolvedType::UNKNOWN].into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            let partial_pair = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Pair").unwrap(),
                    arguments: vec![model.core_type("num", false), ResolvedType::UNKNOWN].into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if !function.type_params().is_empty() {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                let context = match function.name().name() {
                    "partial" | "bounded" => Some(&partial),
                    "complete" => Some(&expected),
                    "nested" => Some(&partial_pair),
                    _ => None,
                };
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        context,
                        &|_| None,
                    ),
                    expected,
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn partial_contexts_remain_fallbacks_for_calls_and_tearoffs_without_evidence() {
    with_model(
        "T empty<T>() => throw 0; T identity<T>(T value) => value; \
         dynamic call() => empty(); dynamic tearoff() => identity;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let partial = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:core", "List"),
                    arguments: vec![ResolvedType::UNKNOWN].into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            let mut signature = ResolvedSignature::returning(partial.clone());
            signature.positional.push(partial.clone());
            let function_context = signature.function_type();
            for declaration in program.declarations().skip(2) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let is_call = function.name().name() == "call";
                let inferred = TypeEnvironment::new().infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                    Some(if is_call { &partial } else { &function_context }),
                    &|_| None,
                );
                if is_call {
                    assert_eq!(inferred, partial);
                } else {
                    let ResolvedType {
                        kind: ResolvedTypeKind::Function { positional, .. },
                    } = &inferred
                    else {
                        panic!("expected an instantiated tear-off")
                    };
                    assert_eq!(inferred.function_return_type(), Some(partial.clone()));
                    assert_eq!(positional.as_ref(), std::slice::from_ref(&partial));
                }
            }
        },
    );
}

#[test]
fn downward_future_or_contexts_constrain_returned_interface_supertypes() {
    with_model(
        "import 'dart:async'; \
         class Sequence<T> {} class Items<T> extends Sequence<T> {} \
         Items<T> create<T>(T value) => throw 0; \
         Future<Items<T>> pending<T>(T value) => throw 0; \
         FutureOr<Sequence<num>> direct() => create(1); \
         FutureOr<Sequence<num>> future() => pending(1); \
         Future<Sequence<num>> Function() closure() => () async => create(1);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations().skip(4) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                let inferred = TypeEnvironment::new().infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                    Some(&expected),
                    &|_| None,
                );
                let value = match &inferred {
                    ResolvedType {
                        kind: ResolvedTypeKind::Function { return_type, .. },
                    } => return_type.as_ref(),
                    other => other,
                };
                let value = if value.interface("dart:async", "Future") {
                    &value.arguments()[0]
                } else {
                    value
                };
                assert!(
                    value.arguments()[0].interface("dart:core", "num"),
                    "{}: {inferred:?}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn downward_constraints_follow_formal_supertypes() {
    with_model(
        "class Sequence<T> {} class Items<T> extends Sequence<T> {} \
         Items<T> create<T>(T value) => throw 0; \
         Sequence<num> probe() => create(1);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().last().unwrap() else {
                panic!()
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!()
            };
            let expression = expression.expr();
            let expected = model.resolve_type(function.return_type().unwrap());
            let inferred = TypeEnvironment::new().infer_with_context(
                expression,
                model,
                &signatures,
                &TypeParameterScope::default(),
                Some(&expected),
                &|_| None,
            );
            assert!(inferred.arguments()[0].interface("dart:core", "num"));
        },
    );
}

#[test]
fn async_closure_flattens_returns_before_joining() {
    with_model(
        "import 'dart:async'; \
         class Pending implements Future<int> {} \
         dynamic mixed(bool choose, Future<int> future) => () async { if (choose) return 1; return future; }; \
         dynamic subclass(bool choose, Pending future) => () async { if (choose) return 1; return future; }; \
         dynamic arrow(Pending future) => () async => future;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let ResolvedType {
                    kind: ResolvedTypeKind::Function { return_type, .. },
                } = environment.infer_with_signatures(expression, model, &signatures, &scope)
                else {
                    panic!()
                };
                assert!(return_type.interface("dart:async", "Future"));
                let argument = &return_type.arguments()[0];
                assert!(argument.interface("dart:core", "int"));
            }
        },
    );
}

#[test]
fn closure_loop_writes_are_cached_per_file() {
    let paths = [
        PathBuf::from("/project/lib/first.dart"),
        PathBuf::from("/project/lib/second.dart"),
    ];
    let sources = [
        "dynamic f() => () { int? x = 1; while (true) { x = null; } return x; };",
        "dynamic f() => () { int? y = 1; while (true) { y = null; } return y; };",
    ];
    let parsed = sources.map(|source| {
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        Arc::new(crate::ParsedFile {
            source: source.into(),
            tree: parsed.tree,
        })
    });
    let programs = parsed.each_ref().map(|parsed| parsed.program());
    let identities = IdentityIndex::from_project_files(
        &[
            IdentitySource {
                path: &paths[0],
                parsed: &parsed[0],
                has_parse_errors: false,
            },
            IdentitySource {
                path: &paths[1],
                parsed: &parsed[1],
                has_parse_errors: false,
            },
        ],
        &[],
    );
    let types = TypeIndex::from_program(programs[0]);
    let files = vec![
        (paths[0].clone(), parsed[0].clone()),
        (paths[1].clone(), parsed[1].clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    let config = falcon_config::FalconConfig::default();
    for (index, name) in ["x", "y"].into_iter().enumerate() {
        let model = SemanticModel::new(&paths[index], &parsed[index], &identities, Some(&types));
        let start = sources[index].find("while").unwrap();
        assert_eq!(
            signatures.flow_writes[&model.file].region_writes[&start],
            HashSet::from([name.to_owned()]),
        );
        let context = crate::AnalyzeContext::new(&paths[index], &config, &parsed[index])
            .with_identities(&identities)
            .with_signatures(&signatures);
        assert!(std::ptr::eq(
            context.flow_writes(programs[index]),
            signatures
                .flow_writes(model.file.unwrap())
                .unwrap()
                .as_ref(),
        ));
    }
}

#[test]
fn nullable_receiver_selects_unconstrained_generic_extension() {
    with_model(
        "extension Wide on Object? { int get value => 0; } \
         extension Narrow<T> on T { String get value => ''; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let (ty, substitutions) = signatures
                .resolved_extension_getter(&model.core_type("int", true), "value", model)
                .unwrap();
            let ty = model.substitute(&ty, &substitutions);
            assert!(ty.interface("dart:core", "String"));
        },
    );
}

#[test]
fn nullable_extensions_shadow_members_of_the_non_null_branch() {
    with_model(
        "class String { bool get isEmpty => true; bool check() => true; } \
         extension Nullable on String? { int get isEmpty => 0; int check() => 0; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver = ResolvedType::interface_identity(model.resolve_name("String").unwrap());
            let nullable = receiver.with_nullable(true);
            let (field, substitutions) = signatures
                .resolved_extension_getter(&nullable, "isEmpty", model)
                .unwrap();
            assert!(
                model
                    .substitute(&field, &substitutions)
                    .interface("dart:core", "int")
            );
            assert!(
                signatures
                    .resolved_field(&nullable, "isEmpty", model)
                    .is_none()
            );
            let (method, substitutions) = signatures
                .resolved_member(&nullable, "check", model)
                .unwrap();
            assert!(
                model
                    .substitute(&method.return_type, &substitutions)
                    .interface("dart:core", "int")
            );
            assert_eq!(
                signatures.resolved_extension_owner(&nullable, "isEmpty", model),
                model.resolve_extension("Nullable"),
            );
            let (field, substitutions) = signatures
                .resolved_field(&receiver, "isEmpty", model)
                .unwrap();
            assert!(
                model
                    .substitute(&field, &substitutions)
                    .interface("dart:core", "bool")
            );
            let (method, substitutions) = signatures
                .resolved_member(&receiver, "check", model)
                .unwrap();
            assert!(
                model
                    .substitute(&method.return_type, &substitutions)
                    .interface("dart:core", "bool")
            );
            assert!(
                signatures
                    .resolved_extension_owner(&receiver, "isEmpty", model)
                    .is_none()
            );
        },
    );
}

#[test]
fn null_shorting_preserves_declared_nullable_member_receivers() {
    with_model(
        r#"
class Value {}
class Items { Value? operator [](int index) => throw 0; }
class Holder {
  Value? get maybe => throw 0;
  Value get present => throw 0;
  Value? maybeCall() => throw 0;
  Value presentCall() => throw 0;
  Items get items => throw 0;
}
extension Nullable on Value? {
  int get size => 0;
  int measure() => 0;
}
extension NonNullable on Value {
  String get size => '';
  String measure() => '';
}
Holder? holder = throw 0;
final nullableField = holder?.maybe.size;
final nonNullableField = holder?.present.size;
final ownShorting = holder?.maybe?.size;
final parenthesizedField = (holder?.present).size;
final nullableCall = holder?.maybeCall().measure();
final parenthesizedCall = (holder?.presentCall()).measure();
final nullableIndex = holder?.items[0].size;
"#,
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (name, expected, nullable) in [
                ("nullableField", "int", true),
                ("nonNullableField", "String", true),
                ("ownShorting", "String", true),
                ("parenthesizedField", "int", false),
                ("nullableCall", "int", true),
                ("parenthesizedCall", "int", false),
                ("nullableIndex", "int", true),
            ] {
                let identity = model.resolve_value(name).unwrap();
                let ty = signatures.value_type(&identity).unwrap();
                assert!(ty.interface("dart:core", expected), "{name}: {ty:?}");
                assert_eq!(ty.nullable(), nullable, "{name}: {ty:?}");
            }
        },
    );
}

#[test]
fn inherited_interface_owner_follows_nested_type_parameter_bounds() {
    with_model(
        "class I { void m() {} } abstract class A implements I {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let a = ResolvedType::interface_identity(model.resolve_name("A").unwrap());
            let s = ResolvedType {
                kind: ResolvedTypeKind::TypeParameter {
                    id: sdk_parameter("bounds", 0),
                    bound: Arc::new(a),
                    nullable: false,
                },
            };
            let t = ResolvedType {
                kind: ResolvedTypeKind::TypeParameter {
                    id: sdk_parameter("bounds", 1),
                    bound: Arc::new(s.clone()),
                    nullable: false,
                },
            };
            for receiver in [s, t] {
                assert_eq!(
                    signatures.resolved_member_owner(&receiver, "m", model),
                    model.resolve_name("I"),
                );
            }
        },
    );
}

#[test]
fn finally_and_capture_mutations_leave_snapshots_unchanged() {
    let mut entry = TypeEnvironment::new();
    entry.declare("value", ResolvedType::DYNAMIC);
    let mut end = entry.clone();
    end.promote("value", ResolvedType::NULL);
    let mut target = entry.clone();
    target.attach_finally(&entry, &end);
    assert_eq!(target.lookup("value"), ResolvedType::NULL);
    assert_eq!(entry.lookup("value"), ResolvedType::DYNAMIC);

    let snapshot = target.clone();
    target.mark_captured("value");
    assert_eq!(target.lookup("value"), ResolvedType::DYNAMIC);
    assert_eq!(snapshot.lookup("value"), ResolvedType::NULL);
    assert!(!snapshot.binding("value").unwrap().captured);
}

#[test]
fn inferred_intersections_demote_before_later_writes() {
    let parameter = ResolvedType {
        kind: ResolvedTypeKind::TypeParameter {
            id: sdk_parameter("test", 0),
            bound: Arc::new(ResolvedType::DYNAMIC),
            nullable: false,
        },
    };
    let intersection = parameter.non_null();
    let mut environment = TypeEnvironment::new();
    environment.declare("value", intersection.clone());
    assert_eq!(environment.declared_type("value"), parameter);
    assert_eq!(environment.lookup("value"), intersection);
    environment.forget("value");
    assert_eq!(environment.lookup("value"), parameter);
}

#[test]
fn unresolved_property_chains_infer_in_linear_work() {
    let source = format!("final value = unknown{};", ".x".repeat(40));
    with_model(&source, |program, model| {
        let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap() else {
            panic!("expected variable");
        };
        let expression = variable
            .declarators()
            .next()
            .unwrap()
            .initializer()
            .unwrap();
        let signatures = SignatureIndex::from_program(program, model);
        let lookups = std::cell::Cell::new(0);
        let lookup = |_: &str| {
            lookups.set(lookups.get() + 1);
            None
        };
        let result = TypeEnvironment::new().infer_with_signatures_using_lookup(
            expression,
            model,
            &signatures,
            &TypeParameterScope::default(),
            &lookup,
        );
        assert_eq!(result, ResolvedType::UNKNOWN);
        assert!(
            lookups.get() <= 200,
            "{} lookups for 40 fields",
            lookups.get()
        );
    });
}

#[test]
fn member_parameter_covariance_includes_overridden_interfaces() {
    with_model(
        r#"
class Leaf extends Middle {
  void run(Object renamed, [Object? extra]) {}
}
class Middle implements Contract {
  void run(Object different, [Object? extra]) {}
}
abstract class Contract {
  void run(covariant Object value, [Object? extra]);
  void named({covariant Object? selected, Object? untouched});
}
class Named implements Contract {
  void run(Object value, [Object? extra]) {}
  void named({Object? untouched, Object? selected, Object? added}) {}
}
mixin Mixed {
  void apply(covariant Object value) {}
}
class Applied with Mixed {
  void apply(Object renamed) {}
}
class Generic<T> {
  void accept(T value) {}
}
class GenericChild extends Generic<int> {
  void accept(int value) {}
}
class StaticBase {
  static void accept(covariant Object value) {}
}
class StaticChild extends StaticBase {
  void accept(Object value) {}
}
class Explicit {
  void accept(covariant Object value) {}
}
"#,
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (owner, name, expected) in [
                ("Leaf", "run", vec![true, false]),
                ("Middle", "run", vec![true, false]),
                ("Named", "named", vec![false, true, false]),
                ("Applied", "apply", vec![true]),
                ("GenericChild", "accept", vec![false]),
                ("StaticChild", "accept", vec![false]),
                ("Explicit", "accept", vec![true]),
            ] {
                let identity = model.resolve_name(owner).unwrap();
                let receiver = ResolvedType::interface_identity(identity);
                let contracts = signatures
                    .effective_parameter_contracts(&receiver, name, false, model)
                    .unwrap();
                assert_eq!(
                    contracts
                        .iter()
                        .map(|parameter| parameter.is_covariant)
                        .collect::<Vec<_>>(),
                    expected,
                    "{owner}.{name}",
                );
                if owner == "Leaf" {
                    let inherited = signatures
                        .super_parameter_contracts(&receiver, name, false, model)
                        .unwrap();
                    assert_eq!(
                        inherited
                            .iter()
                            .map(|parameter| parameter.is_covariant)
                            .collect::<Vec<_>>(),
                        expected,
                    );
                }
            }
        },
    );
}

#[test]
fn setter_parameter_covariance_stays_accessor_specific() {
    with_model(
        r#"
abstract class Setters {
  set value(covariant Object input);
  covariant Object field = 0;
}
class Override implements Setters {
  Object get value => 0;
  set value(Object renamed) {}
  set field(Object renamed) {}
}
class FieldOverride implements Setters {
  Object value = 0;
  Object field = 0;
}
class Plain {
  set value(Object input) {}
}
class Explicit {
  set value(covariant Object input) {}
}
class OtherKind {
  void value(covariant Object input) {}
}
class DifferentKind extends OtherKind {
  set value(Object input) {}
}
"#,
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (owner, name, expected) in [
                ("Override", "value", true),
                ("Override", "field", true),
                ("FieldOverride", "value", true),
                ("FieldOverride", "field", true),
                ("Plain", "value", false),
                ("Explicit", "value", true),
                ("DifferentKind", "value", false),
            ] {
                let identity = model.resolve_name(owner).unwrap();
                let facts = signatures.declared_member_facts(&identity, name);
                let receiver = ResolvedType::interface_identity(identity);
                let contracts = signatures
                    .effective_parameter_contracts(&receiver, name, true, model)
                    .unwrap();
                assert_eq!(contracts[0].is_covariant, expected, "{owner}.{name}",);
                for getter in facts
                    .iter()
                    .filter(|facts| facts.kind == MemberKind::Getter)
                {
                    assert!(getter.parameter_contracts.as_ref().unwrap().is_empty());
                }
            }
        },
    );
}

#[test]
fn covariance_distinguishes_interface_signatures_from_contextual_implementations() {
    with_model(
        r#"
class CovariantBase {
  void run(covariant num value) {}
}
mixin PlainMixin {
  void run(num value) {}
}
class Shadowed extends CovariantBase with PlainMixin {
  void run(num value) {}
}
abstract class CovariantInterface {
  void run(covariant num value);
  set value(covariant num input);
}
abstract class PlainInterface {
  void run(num value);
}
abstract class PlainFirst implements PlainInterface, CovariantInterface {}
abstract class CovariantFirst implements CovariantInterface, PlainInterface {}
class PlainChild extends PlainFirst {
  void run(num value) {}
}
class CovariantChild extends CovariantFirst {
  void run(num value) {}
}
class Concrete {
  void run(num value) {}
  set value(num input) {}
}
class Inherited extends Concrete implements CovariantInterface {}
class Deeper extends Inherited {}
mixin ConstraintMixin on Inherited {
  void run(num value) => super.run(value);
}
class PairBase {
  void pair(num left, num right) {}
}
abstract class LeftCovariance {
  void pair(covariant num left, num right);
}
abstract class RightCovariance {
  void pair(num left, covariant num right);
}
class FirstPair extends PairBase implements LeftCovariance {}
class ThroughPair extends FirstPair {}
class ChangedPair extends ThroughPair implements RightCovariance {}
class ContractBase {
  void defaults([int value = 2]) {}
  void covariance(num value) {}
}
abstract class AbstractContract extends ContractBase {
  void defaults([int value = 1]);
  void covariance(covariant num value);
}
class ConcreteContract extends AbstractContract {
  void defaults([int value = 1]) => super.defaults(value);
  void covariance(covariant num value) => super.covariance(value);
}
class ContextWithUnknown extends Concrete implements PlainInterface, CovariantInterface {}
class UnknownOnly extends Concrete implements PlainInterface {}
mixin InheritedMixin implements CovariantInterface {}
class InheritedMixinChild extends Concrete with InheritedMixin {
  void run(num value) {}
}
class Redeclared extends Inherited {
  void run(num value) {}
  set value(num input) {}
}
class Conflict implements CovariantInterface {
  num get run => 0;
}
class ConflictChild extends Conflict {
  void run(num value) {}
}
"#,
        |program, model| {
            let mut signatures = SignatureIndex::from_program(program, model);
            for (owner, expected) in [
                ("Shadowed", false),
                ("PlainChild", false),
                ("CovariantChild", true),
                ("Concrete", false),
                ("Redeclared", false),
                ("InheritedMixinChild", false),
                ("ConflictChild", false),
            ] {
                let identity = model.resolve_name(owner).unwrap();
                let receiver = ResolvedType::interface_identity(identity);
                let contracts = signatures
                    .effective_parameter_contracts(&receiver, "run", false, model)
                    .unwrap();
                assert_eq!(contracts[0].is_covariant, expected, "{owner}.run");
            }
            for (owner, name, write, expected) in [
                ("Concrete", "run", false, false),
                ("Inherited", "run", false, true),
                ("Deeper", "run", false, true),
                ("Concrete", "value", true, false),
                ("Inherited", "value", true, true),
                ("Deeper", "value", true, true),
                ("Redeclared", "value", true, false),
            ] {
                let receiver = ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                let contracts = signatures
                    .effective_parameter_contracts(&receiver, name, write, model)
                    .unwrap();
                assert_eq!(contracts[0].is_covariant, expected, "{owner}.{name}");
            }
            for (owner, expected) in [("Redeclared", true), ("ConstraintMixin", false)] {
                let receiver = ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                let contracts = signatures
                    .super_parameter_contracts(&receiver, "run", false, model)
                    .unwrap();
                assert_eq!(contracts[0].is_covariant, expected, "{owner}.super.run");
            }
            for (owner, expected) in [
                ("FirstPair", [true, false]),
                ("ThroughPair", [true, false]),
                ("ChangedPair", [false, true]),
            ] {
                let receiver = ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                let contracts = signatures
                    .effective_parameter_contracts(&receiver, "pair", false, model)
                    .unwrap();
                assert_eq!(
                    [contracts[0].is_covariant, contracts[1].is_covariant],
                    expected,
                    "{owner}.pair",
                );
            }
            let receiver =
                ResolvedType::interface_identity(model.resolve_name("ConcreteContract").unwrap());
            for name in ["defaults", "covariance"] {
                let own = signatures
                    .effective_parameter_contracts(&receiver, name, false, model)
                    .unwrap();
                let inherited = signatures
                    .super_parameter_contracts(&receiver, name, false, model)
                    .unwrap();
                let base = signatures
                    .declared_member_facts(&model.resolve_name("ContractBase").unwrap(), name)[0]
                    .parameter_contracts
                    .as_ref()
                    .unwrap();
                assert_eq!(&inherited, base, "{name} concrete super");
                assert_ne!(own, inherited, "{name} abstract redeclaration");
            }
            let plain = model.resolve_name("PlainInterface").unwrap();
            signatures
                .member_entries
                .get_mut(&plain)
                .unwrap()
                .get_mut("run")
                .unwrap()
                .facts[0]
                .parameter_contracts = None;
            for (owner, expected) in [("ContextWithUnknown", Some(true)), ("UnknownOnly", None)] {
                let receiver = ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                let contracts =
                    signatures.effective_parameter_contracts(&receiver, "run", false, model);
                assert_eq!(
                    contracts
                        .as_ref()
                        .map(|parameters| parameters[0].is_covariant),
                    expected,
                    "{owner}.run",
                );
            }
        },
    );
}

#[test]
fn super_contracts_skip_abstract_mixin_declarations() {
    with_model(
        r#"
class ConstraintBase { void run([int value = 1]) {} }
class ActualBase extends ConstraintBase { void run([int value = 2]) {} }
mixin AbstractConstraint on ConstraintBase { void run([int value = 1]); }
class ConstraintApplication extends ActualBase with AbstractConstraint {}
class ConstraintChild extends ConstraintApplication {
  void run([int value = 1]) => super.run(value);
}
abstract class Interface { void run([int value = 2]); }
mixin AbstractInterface implements Interface { void run([int value = 2]); }
class InterfaceApplication extends ActualBase with AbstractInterface {}
class InterfaceChild extends InterfaceApplication {
  void run([int value = 2]) => super.run(value);
}
class SetterBase { set value(num input) {} }
mixin AbstractSetter { set value(num input); }
class SetterApplication extends SetterBase with AbstractSetter {}
class SetterChild extends SetterApplication {
  set value(num input) => super.value = input;
}
"#,
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (owner, base, name, write, equal) in [
                ("ConstraintChild", "ActualBase", "run", false, false),
                ("InterfaceChild", "ActualBase", "run", false, true),
                ("SetterChild", "SetterBase", "value", true, true),
            ] {
                let receiver = ResolvedType::interface_identity(model.resolve_name(owner).unwrap());
                let inherited = signatures
                    .super_parameter_contracts(&receiver, name, write, model)
                    .unwrap();
                let base = signatures
                    .declared_member_facts(&model.resolve_name(base).unwrap(), name)[0]
                    .parameter_contracts
                    .as_ref()
                    .unwrap();
                assert_eq!(&inherited, base, "{owner} concrete super");
                let own = signatures
                    .effective_parameter_contracts(&receiver, name, write, model)
                    .unwrap();
                assert_eq!(own == inherited, equal, "{owner} forwarding contract");
            }
        },
    );
}

pub(crate) fn with_model(source: &str, test: impl FnOnce(Program<'_>, &SemanticModel<'_>)) {
    with_model_at(Path::new("/project/lib/main.dart"), source, test);
}

pub(crate) fn with_model_at(
    path: &Path,
    source: &str,
    test: impl FnOnce(Program<'_>, &SemanticModel<'_>),
) {
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let parsed = Arc::new(crate::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let sources = [IdentitySource {
        path,
        parsed: &parsed,
        has_parse_errors: false,
    }];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_program(program);
    let model = SemanticModel::new(path, &parsed, &identities, Some(&types));
    test(program, &model);
}

#[test]
fn sdk_annotation_constants_follow_const_aliases_only() {
    with_model(
        "import 'dart:core' as core; import 'dart:core' hide deprecated, override; \
         const deprecated = core.deprecated; const override = core.override; \
         class Marks { static const old = core.deprecated; static const replacement = core.override; } \
         class Shadowed { static const deprecated = 0; static const old = deprecated; } \
         final mutable = core.deprecated; const fake = 0; \
         @deprecated void old() {} @Marks.old void older() {}",
        |program, model| {
            for (name, sdk_name) in [
                ("deprecated".into(), "deprecated"),
                ("override".into(), "override"),
                (QualifiedName::prefixed("Marks", "old"), "deprecated"),
                (QualifiedName::prefixed("Marks", "replacement"), "override"),
            ] {
                assert_eq!(
                    model.resolve_sdk_annotation_constant(name),
                    Some(DeclarationIdentity::sdk("dart:core", sdk_name)),
                );
            }
            for name in ["mutable", "fake"] {
                assert_eq!(model.resolve_sdk_annotation_constant(name), None);
            }
            assert_eq!(
                model.resolve_sdk_annotation_constant(QualifiedName::prefixed("Shadowed", "old")),
                None,
            );
            for declaration in program.declarations() {
                if let TopLevelDecl::Function(function) = declaration {
                    assert!(model.is_deprecated_constant(function.annotations().next().unwrap()));
                }
            }
        },
    );
}

#[test]
fn sdk_annotation_constant_aliases_respect_static_scope() {
    with_model(
        "class Marks { static const deprecated = 0; static const old = deprecated; } \
         enum E { deprecated; static const old = deprecated; }",
        |_, model| {
            for owner in ["Marks", "E"] {
                assert_eq!(
                    model.resolve_sdk_annotation_constant(QualifiedName::prefixed(owner, "old")),
                    None,
                );
            }
        },
    );
}

#[test]
fn substitution_removes_instantiated_function_binders_and_maps_retained_bounds() {
    with_model(
        "T f<T, U extends T>(T input, {required int value}) => throw 0;",
        |program, model| {
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!("expected function")
            };
            let mut scope = TypeParameterScope::default();
            scope.push(function.type_params().take(1), model);
            assert!(
                FunctionTypeShape::from_scope(Vec::new(), function.type_params(), &scope,)
                    .is_none()
            );
            scope.push(function.type_params(), model);
            let shape = FunctionTypeShape::from_scope(
                vec!["value".to_owned()],
                function.type_params(),
                &scope,
            )
            .unwrap();
            assert_eq!(shape.type_parameters.len(), 2);
            let ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { id, .. },
            } = &shape.type_parameters[0]
            else {
                panic!("expected binder")
            };
            let substitutions = FxHashMap::from_iter([(id.clone(), ResolvedType::VOID)]);
            let signatures = SignatureIndex::from_program(program, model);
            let source = signatures
                .function(&model.resolve_value("f").unwrap())
                .unwrap()
                .function_type();
            let result = model.substitute(&source, &substitutions);
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        shape: Some(shape),
                        return_type,
                        positional,
                        ..
                    },
            } = &result
            else {
                panic!("expected exact function shape")
            };
            assert_eq!(**return_type, ResolvedType::VOID);
            assert_eq!(positional.as_ref(), &[ResolvedType::VOID]);
            assert_eq!(shape.required_named.as_ref(), &["value".to_owned()]);
            assert_eq!(shape.type_parameters.len(), 1);
            let ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { id, bound, .. },
            } = &shape.type_parameters[0]
            else {
                panic!("expected retained binder")
            };
            assert_eq!(id.name.as_ref(), "U");
            assert_eq!(**bound, ResolvedType::VOID);
            let substitutions = FxHashMap::from_iter([(id.clone(), ResolvedType::DYNAMIC)]);
            // An unused binder still changes the generic arity when supplied.
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        shape: Some(shape), ..
                    },
            } = model.substitute(&result, &substitutions)
            else {
                panic!("expected instantiated function")
            };
            assert!(shape.type_parameters.is_empty());
        },
    );
}

#[test]
fn extension_erasure_distinguishes_nested_arguments_from_representation_cycles() {
    with_model(
        "extension type Id<T>(T value) {} \
         extension type Cycle<T>(Cycle<T> value) {} \
         Id<Id<int>> nested() => throw 0; \
         Cycle<int> cyclic() => throw 0;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations().skip(2) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!("expected function")
                };
                let ty = model.resolve_type(function.return_type().unwrap());
                let erased = signatures.extension_type_erasure(&ty, model);
                if function.name().name() == "nested" {
                    assert_eq!(erased, Some(model.core_type("int", false)));
                } else {
                    assert_eq!(erased, None);
                }
            }
        },
    );
}

#[test]
fn non_nullable_respects_future_or_bounds_and_extension_interfaces() {
    with_model(
        "import 'dart:async'; \
         extension type NullableBox(Object? value) {} \
         extension type IntBox(int value) { int next() => value + 1; } \
         extension type ObjectBox(Object value) implements Object {} \
         extension type GenericBox<T extends Object>(T value) implements Object {} \
         extension type Meters(int value) implements int {} \
         FutureOr<int?> nullableFuture() => throw 0; \
         FutureOr<T> unboundedFuture<T>() => throw 0; \
         FutureOr<T> boundedFuture<T extends Object>() => throw 0; \
         NullableBox nullableExtension() => throw 0; \
         IntBox intExtension() => throw 0; \
         T boundedExtension<T extends IntBox>() => throw 0; \
         Meters sdkExtension() => throw 0; \
         ObjectBox objectExtension() => throw 0; \
         GenericBox<int> genericExtension() => throw 0; \
         ObjectBox? nullableObjectExtension() => throw 0;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let name = function.name().name();
                let identity = model.resolve_value(name).unwrap();
                let ty = &signatures.function(&identity).unwrap().return_type;
                let expected = matches!(
                    name,
                    "boundedFuture" | "objectExtension" | "genericExtension" | "sdkExtension"
                );
                assert_eq!(signatures.is_non_nullable(ty), expected, "{name}: {ty:?}");
                if name.ends_with("Future") {
                    assert_eq!(ty.is_non_nullable(), expected, "{name}: {ty:?}");
                } else {
                    assert!(!ty.is_non_nullable(), "{name}: {ty:?}");
                }
                if matches!(name, "intExtension" | "boundedExtension") {
                    assert!(signatures.resolved_member(ty, "next", model).is_some());
                }
            }
        },
    );
}

#[test]
fn nested_alias_projections_retain_occurrence_provenance_without_copying_types() {
    let source = r#"
typedef G<T> = Future<T> Function();
typedef F<T> = G<T> Function();
class Items<T> { T get first => throw 0; }
typedef R<T> = (G<T>,) Function();
typedef I<T> = Items<G<T>> Function();
typedef L<T> = Items<G<T>>;
typedef P<T> = (G<T>,);
typedef H<T> = G<T> Function() Function();
class Box<T> {
  final (F<T>, F<T>) pair;
  final (G<T>, G<T>) callbacks;
  final Items<G<T>> items;
  final F<T> f;
  final (R<T>, R<T>) records;
  final (I<T>, I<T>) lists;
  final (L<T>, L<T>) aliases;
  final (P<T>, P<T>) recordAliases;
  final (H<T>, H<T>) nested;
  Box(this.pair, this.callbacks, this.items, this.f, this.records, this.lists, this.aliases, this.recordAliases, this.nested);
}
Box<int> box = throw 0;
bool condition = throw 0;
F<int> f = throw 0;
R<int> r = throw 0;
I<int> i = throw 0;
(G<int>, G<int>) directPair = throw 0;
Items<G<int>> directItems = throw 0;
final pair = box.pair;
final left = pair.$1();
final right = pair.$2();
final separateReturns = condition ? left : right;
final sharedReturn = condition ? left : left;
final separateCalls = condition ? pair.$1() : pair.$2();
final separateMemberCalls = condition ? box.pair.$1() : box.pair.$2();
final namedCallbacks = (first: pair.$1, second: pair.$2);
final namedCalls = condition ? namedCallbacks.first() : namedCallbacks.second();
final sharedNamedCalls = condition ? namedCallbacks.first() : namedCallbacks.first();
final callbacks = box.callbacks;
final sharedProjection = condition ? callbacks.$1 : callbacks.$1;
final siblingProjection = condition ? callbacks.$1 : callbacks.$2;
final repeatedMemberProjection = condition ? box.callbacks.$1 : box.callbacks.$1;
final sharedDirectProjection = condition ? directPair.$1 : directPair.$1;
final separateDirectProjection = condition ? directPair.$1 : directPair.$2;
final repeatedMemberArgument = condition ? box.items.first : box.items.first;
final sharedDirectArgument = condition ? directItems.first : directItems.first;
final sharedDirectCalls = condition ? f() : f();
final separateMemberFunctionCalls = condition ? box.f() : box.f();
final records = box.records;
final a = records.$1();
final b = records.$2();
final siblingRecordReturns = condition ? a.$1 : b.$1;
final sharedRecordReturn = condition ? a.$1 : a.$1;
final siblingRecordCalls = condition ? records.$1().$1 : records.$2().$1;
final sharedDirectRecordCalls = condition ? r().$1 : r().$1;
final joinedRecordReturn = (condition ? a : b).$1;
final sharedJoinedRecordReturn = (condition ? a : a).$1;
final lists = box.lists;
final siblingInterfaceCalls = condition ? lists.$1().first : lists.$2().first;
final sharedDirectInterfaceCalls = condition ? i().first : i().first;
final joinedCallbacks = (condition ? pair.$1 : pair.$2)();
final joinedRecordCallbacks = (condition ? records.$1 : records.$2)().$1;
final sharedJoinedCallbacks = (condition ? pair.$1 : pair.$1)();
final sharedJoinedRecordCallbacks = (condition ? records.$1 : records.$1)().$1;
final aliases = box.aliases;
final siblingAliases = condition ? aliases.$1.first : aliases.$2.first;
final sharedAliases = condition ? aliases.$1.first : aliases.$1.first;
final recordAliases = box.recordAliases;
final siblingRecordAliases = condition ? recordAliases.$1.$1 : recordAliases.$2.$1;
final sharedRecordAliases = condition ? recordAliases.$1.$1 : recordAliases.$1.$1;
final nested = box.nested;
final siblingNested = condition ? nested.$1()() : nested.$2()();
final sharedNested = condition ? nested.$1()() : nested.$1()();
final joinedNested = (condition ? nested.$1() : nested.$2())();
final sharedJoinedNested = (condition ? nested.$1() : nested.$1())();
final intermediate = nested.$1();
"#;
    with_model(source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let g = model.resolve_name("G").unwrap();
        for (name, retains_alias) in [
            ("separateReturns", false),
            ("sharedReturn", true),
            ("separateCalls", false),
            ("separateMemberCalls", false),
            ("namedCalls", false),
            ("sharedNamedCalls", true),
            ("sharedProjection", true),
            ("siblingProjection", false),
            ("repeatedMemberProjection", false),
            ("sharedDirectProjection", true),
            ("separateDirectProjection", false),
            ("repeatedMemberArgument", false),
            ("sharedDirectArgument", true),
            ("sharedDirectCalls", true),
            ("separateMemberFunctionCalls", false),
            ("siblingRecordReturns", false),
            ("sharedRecordReturn", true),
            ("siblingRecordCalls", false),
            ("sharedDirectRecordCalls", true),
            ("joinedRecordReturn", false),
            ("sharedJoinedRecordReturn", true),
            ("siblingInterfaceCalls", false),
            ("sharedDirectInterfaceCalls", true),
            ("joinedCallbacks", false),
            ("joinedRecordCallbacks", false),
            ("sharedJoinedCallbacks", true),
            ("sharedJoinedRecordCallbacks", true),
            ("siblingAliases", false),
            ("sharedAliases", true),
            ("siblingRecordAliases", false),
            ("sharedRecordAliases", true),
            ("siblingNested", false),
            ("sharedNested", true),
            ("joinedNested", false),
            ("sharedJoinedNested", true),
        ] {
            let identity = model.resolve_value(name).unwrap();
            let ResolvedType {
                kind: ResolvedTypeKind::Function { alias, .. },
            } = signatures.value_type(&identity).unwrap()
            else {
                panic!("lost function type for {name}")
            };
            assert_eq!(alias.identity(), retains_alias.then_some(&g), "{name}");
        }
        let ResolvedType {
            kind: ResolvedTypeKind::Function { alias, .. },
        } = signatures
            .value_type(&model.resolve_value("intermediate").unwrap())
            .unwrap()
        else {
            panic!("lost unaliased intermediate function")
        };
        assert_eq!(alias.identity(), None);
    });
}

#[test]
fn callable_record_fields_precede_extension_methods() {
    with_model(
        r#"
class Token {}
class Wrong {}
Token callback() => Token();
extension PositionalCollision on (Token Function(),) {
  Wrong $1() => Wrong();
  Token extra() => Token();
}
extension NamedCollision on ({Token Function() action}) {
  Wrong action() => Wrong();
  Token extra() => Token();
}
final positionalRecord = (callback,);
final namedRecord = (action: callback);
final positionalResult = positionalRecord.$1();
final namedResult = namedRecord.action();
final positionalExtension = positionalRecord.extra();
final namedExtension = namedRecord.extra();
"#,
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let token = model.resolve_name("Token").unwrap();
            for name in [
                "positionalResult",
                "namedResult",
                "positionalExtension",
                "namedExtension",
            ] {
                let identity = model.resolve_value(name).unwrap();
                let Some(ResolvedType {
                    kind: ResolvedTypeKind::Interface { identity, .. },
                }) = signatures.value_type(&identity)
                else {
                    panic!("lost interface type for {name}")
                };
                assert_eq!(identity, &token, "{name}");
            }
        },
    );
}

#[test]
fn sdk_iterable_getters_preserve_projected_alias_origins() {
    with_parsed_core(
        r#"
class Result<T> {}
typedef G<T> = Result<T> Function();
typedef I<T> = List<G<T>> Function();
typedef L<T> = List<G<T>>;
class Box<T> {
  List<G<T>> get list => throw 0;
  (I<T>, I<T>) get functions => throw 0;
  (L<T>, L<T>) get aliases => throw 0;
}
Box<int> box = throw 0;
bool condition = throw 0;
List<G<int>> direct = throw 0;
List<int> integers = throw 0;
final functions = box.functions;
final aliases = box.aliases;
final repeatedMember = condition ? box.list.first : box.list.first;
final returnedSiblings = condition ? functions.$1().first : functions.$2().first;
final aliasedSiblings = condition ? aliases.$1.first : aliases.$2.first;
final sharedAlias = condition ? aliases.$1.first : aliases.$1.first;
final sharedDirect = condition ? direct.first : direct.first;
final integer = integers.first;
"#,
        "library dart.core; class Object {} abstract class int {} \
         abstract class bool {} abstract class Iterable<E> { E get first; } \
         abstract class List<E> implements Iterable<E> {}",
        |_program, model, signatures| {
            let g = model.resolve_name("G").unwrap();
            for (name, retains_alias) in [
                ("repeatedMember", false),
                ("returnedSiblings", false),
                ("aliasedSiblings", false),
                ("sharedAlias", true),
                ("sharedDirect", true),
            ] {
                let identity = model.resolve_value(name).unwrap();
                let Some(ResolvedType {
                    kind: ResolvedTypeKind::Function { alias, .. },
                }) = signatures.value_type(&identity)
                else {
                    panic!("lost SDK iterable function type for {name}")
                };
                assert_eq!(alias.identity(), retains_alias.then_some(&g), "{name}");
            }
            let integer = model.resolve_value("integer").unwrap();
            assert!(
                signatures
                    .value_type(&integer)
                    .unwrap()
                    .interface("dart:core", "int")
            );
        },
    );
}

fn with_parsed_core(
    source: &str,
    core: &str,
    test: impl FnOnce(Program<'_>, &SemanticModel<'_>, &SignatureIndex),
) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let sdk = root.join("flutter/bin/cache/dart-sdk/lib");
    std::fs::create_dir_all(sdk.join("core")).unwrap();
    std::fs::write(
        sdk.join("libraries.json"),
        r#"{"vm":{"libraries":{"core":{"uri":"core/core.dart"}}}}"#,
    )
    .unwrap();
    std::fs::write(sdk.join("core/core.dart"), core).unwrap();
    std::fs::create_dir_all(root.join("app/.dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("app/lib")).unwrap();
    std::fs::write(
        root.join("app/.dart_tool/package_config.json"),
        serde_json::json!({
            "configVersion": 2,
            "flutterRoot": format!("file://{}", root.join("flutter").display()),
            "packages": [{"name": "app", "rootUri": "../", "packageUri": "lib/"}]
        })
        .to_string(),
    )
    .unwrap();
    let path = root.join("app/lib/main.dart");
    std::fs::write(&path, source).unwrap();
    let path = crate::canonical_or_lexical(&path);
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    let parsed = Arc::new(crate::ParsedFile {
        source: source.into(),
        tree,
    });
    let program = parsed.program();
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let environment =
        crate::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |config| {
            let root = config?.flutter_root.as_ref()?;
            crate::environment::DartSdk::load(&root.join("bin/cache/dart-sdk"))
        });
    let external = ExternalLibraries::load(
        &environment,
        &[(path.as_path(), program, source)],
        |candidate| candidate == path.as_path(),
    );
    assert!(!external.is_empty(), "parsed core fixture was not loaded");
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        Some(crate::ExternalSources {
            libraries: &external,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            path.as_path(),
            LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&external, &identities);
    let signatures = SignatureIndex::build(
        &[(path.clone(), parsed.clone())],
        Some(&external),
        &identities,
        &types,
    );
    test(
        program,
        &SemanticModel::new(&path, &parsed, &identities, Some(&types)),
        &signatures,
    );
}

#[test]
fn concrete_property_chain_readiness_reuses_receiver_results() {
    const DEPTH: usize = 40;
    crate::with_rules_stack(|| {
        for chain in [".next".repeat(DEPTH), ".next.step()".repeat(DEPTH)] {
            let source = format!(
                "class Link {{ Link get next => throw 0; Link step() => throw 0; }} \
             final value = root{chain};",
            );
            with_model(&source, |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Variable(variable) = program.declarations().nth(1).unwrap()
                else {
                    panic!("expected variable");
                };
                let expression = variable
                    .declarators()
                    .next()
                    .unwrap()
                    .initializer()
                    .unwrap();
                let receiver = ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name("Link").unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    },
                };
                let lookups = std::cell::Cell::new(0);
                assert!(signatures.declaration_dependencies_known_using_lookup(
                    expression,
                    model,
                    None,
                    &TypeEnvironment::new(),
                    &TypeParameterScope::default(),
                    &|name| {
                        lookups.set(lookups.get() + 1);
                        (name == "root").then(|| receiver.clone())
                    },
                ));
                assert_eq!(lookups.get(), 1, "readiness re-inferred a receiver prefix");
            });
        }
    })
    .unwrap();
}

#[test]
fn root_dependencies_preserve_partial_progress_and_dynamic_roots() {
    for source in [
        "final rows = legacy == null ? [header] : [header]; final header = 'h'; var legacy;",
        "var legacy; final header = 'h'; final rows = legacy == null ? [header] : [header];",
        "final rows = cycle == null ? [header] : [header]; final header = 'h'; final cycle = cycle;",
        "final cycle = cycle; final header = 'h'; final rows = cycle == null ? [header] : [header];",
    ] {
        with_model(source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            assert_eq!(
                signatures.value_type(&model.resolve_value("rows").unwrap()),
                Some(&ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "List"),
                        arguments: vec![model.core_type("String", false)].into(),
                        nullable: false,
                        extension_type: false,
                    },
                }),
            );
            if let Some(identity) = model.resolve_value("legacy") {
                assert_eq!(
                    signatures.value_type(&identity),
                    Some(&ResolvedType::DYNAMIC)
                );
            }
        });
    }
}

#[test]
fn root_dependency_scopes_do_not_create_shadowing_cycles() {
    for initializer in [
        "[for (final value in <int>[1]) value * 2]",
        "(() { int f(int value) => value; return <int>[f(1)]; })()",
        "(() { for (var value = 0; value < 1; value++) {} return <int>[1]; })()",
        "(() { for (final value in <int>[1]) { print(value); } return <int>[1]; })()",
        "[for (final (value,) in <(int,)>[(1,)]) value]",
        "(() { try {} catch (value) { print(value); } return <int>[1]; })()",
        "(() { final (value,) = (1,); print(value); return <int>[1]; })()",
        "(() { if ((1,) case (var value,)) { print(value); } return <int>[1]; })()",
        "(() { switch (1) { case var value: print(value); } return <int>[1]; })()",
        "[if ((1,) case (var value,)) value]",
        "[switch (1) { var value => value }]",
    ] {
        with_model(
            &format!("final items = {initializer}; final value = items.first;"),
            |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                assert!(signatures.cyclic_values.is_empty(), "{initializer}");
                assert_eq!(
                    signatures.value_type(&model.resolve_value("items").unwrap()),
                    Some(&ResolvedType {
                        kind: ResolvedTypeKind::Interface {
                            occurrence: Default::default(),
                            identity: DeclarationIdentity::sdk("dart:core", "List"),
                            arguments: vec![model.core_type("int", false)].into(),
                            nullable: false,
                            extension_type: false,
                        },
                    }),
                    "{initializer}",
                );
            },
        );
    }
}

#[test]
fn field_readiness_respects_closure_parameters() {
    with_parsed_core(
        "class C { static final value = [1].map((value) => value + 1).toList(); }",
        "library dart.core; class Object {} abstract class num {} \
         abstract class int extends num { int operator +(int other); } \
         abstract class Iterable<E> { Iterable<T> map<T>(T toElement(E value)); List<E> toList(); } \
         abstract class List<E> implements Iterable<E> {}",
        |_, model, signatures| {
            assert_eq!(
                signatures
                    .field(&model.resolve_name("C").unwrap(), "value")
                    .unwrap()
                    .1,
                ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "List"),
                        arguments: vec![model.core_type("int", false)].into(),
                        nullable: false,
                        extension_type: false,
                    },
                },
            );
        },
    );
}

#[test]
fn inherited_getter_dependencies_wait_for_declaration_inference() {
    with_model(
        "final result = Child().value; final dynamicResult = Explicit().value; \
         final plainResult = Plain().value; final receiver = Child(); \
         final indirect = receiver.value; \
         class Child extends Middle { get value => throw 0; } \
         class Middle extends Base { get value => throw 0; } \
         class Base { final value = later; } \
         final later = Packet(); class Packet {} \
         class Explicit extends Base { dynamic get value => throw 0; } \
         class Plain { get value => throw 0; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let packet = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Packet").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            for name in ["result", "indirect"] {
                assert_eq!(
                    signatures.value_type(&model.resolve_value(name).unwrap()),
                    Some(&packet),
                );
            }
            for name in ["Child", "Middle", "Base"] {
                assert_eq!(
                    signatures
                        .field(&model.resolve_name(name).unwrap(), "value")
                        .unwrap()
                        .1,
                    packet,
                );
            }
            for name in ["dynamicResult", "plainResult"] {
                assert_eq!(
                    signatures.value_type(&model.resolve_value(name).unwrap()),
                    Some(&ResolvedType::DYNAMIC),
                );
            }
        },
    );
}

#[test]
fn interface_diamonds_preserve_substitution_in_bounded_lookup() {
    let mut source = String::from(
        "class Packet {} class Root<T> {} \
         class A0<T> implements Root<T> {} class B0<T> implements Root<T> {} \
         class Tail<T> { T get value => throw 0; }",
    );
    for level in 1..24 {
        let previous = level - 1;
        source.push_str(&format!(
            "class A{level}<T> implements A{previous}<T>, B{previous}<T> {{}} \
             class B{level}<T> implements A{previous}<T>, B{previous}<T> {{}}",
        ));
    }
    source.push_str("class Child extends A23<Packet> implements Tail<Packet> {}");
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        assert!(signatures.cyclic_supertypes.is_some());
        let child = ResolvedType {
            kind: ResolvedTypeKind::Interface {
                occurrence: Default::default(),
                identity: model.resolve_name("Child").unwrap(),
                arguments: Vec::new().into(),
                nullable: false,
                extension_type: false,
            },
        };
        assert!(
            signatures
                .resolved_field(&child, "missing", model)
                .is_none()
        );
        let (ty, substitutions) = signatures.resolved_field(&child, "value", model).unwrap();
        assert_eq!(
            model.substitute(&ty, &substitutions),
            ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Packet").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            },
        );
    });
}

#[test]
fn missing_declaration_types_use_completed_signatures() {
    with_model(
        "final forward = later; final later = Box(1); get top => forward; \
         final cycle = Box(other); final other = cycle; \
         class Box<T> { Box(T value); T get value => throw 0; } \
         class Values<T> { \
           static final first = second; static final second = Box('text'); \
           static get result => first; final number = 1; get inferred => number; \
           T get typed => throw 0; get generic => typed; \
         }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let box_identity = model.resolve_name("Box").unwrap();
            let box_of = |argument| ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: box_identity.clone(),
                    arguments: vec![argument].into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            for name in ["forward", "later"] {
                assert_eq!(
                    signatures.value_type(&model.resolve_value(name).unwrap()),
                    Some(&box_of(model.core_type("int", false)))
                );
            }
            assert_eq!(
                signatures.value_type(&model.resolve_value("top").unwrap()),
                Some(&ResolvedType::DYNAMIC)
            );
            for name in ["cycle", "other"] {
                assert_eq!(
                    signatures.value_type(&model.resolve_value(name).unwrap()),
                    Some(&ResolvedType::UNKNOWN)
                );
            }
            let values = model.resolve_name("Values").unwrap();
            for name in ["first", "second"] {
                assert_eq!(
                    signatures.field(&values, name).unwrap().1,
                    box_of(model.core_type("String", false))
                );
            }
            assert_eq!(
                signatures.field(&values, "number").unwrap().1,
                model.core_type("int", false)
            );
            for name in ["result", "inferred", "generic"] {
                assert_eq!(
                    signatures.field(&values, name).unwrap().1,
                    ResolvedType::DYNAMIC
                );
            }
            assert!(matches!(
                signatures.field(&values, "typed").unwrap().1,
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { .. },
                }
            ));
        },
    );
}

#[test]
fn untyped_overrides_inherit_instantiated_declared_types() {
    with_model(
        "abstract class Base { num get value; num get empty; } \
         class FieldValue extends Base { final value = 1; var empty; } \
         class GetterValue extends Base { get value => 1.5; get empty => 1; } \
         abstract class Generic<T> { T get value; } \
         class GenericValue implements Generic<int> { get value => 1.5; } \
         class GenericField implements Generic<int> { final value = 1.5; } \
         class EarlyValue extends MiddleValue { final value = 1; var empty; } \
         class MiddleValue extends Base { get value => 1.5; get empty => 1; } \
         class StaticValue extends Base { static final value = 1; static get empty => 1.5; } \
         abstract class Other { String get value; } \
         class Conflict extends Base implements Other { final value = 1; get empty => 1; } \
         class GetterConflict extends Base implements Other { get value => 1.5; }",
        |program, model| {
            let mut signatures = SignatureIndex::default();
            signatures.add_program(program, model);
            for owner in ["FieldValue", "GetterValue", "EarlyValue", "MiddleValue"] {
                let identity = model.resolve_name(owner).unwrap();
                for name in ["value", "empty"] {
                    assert_eq!(
                        signatures.field(&identity, name).unwrap().1,
                        model.core_type("num", false)
                    );
                }
            }
            for owner in ["GenericValue", "GenericField"] {
                let generic = model.resolve_name(owner).unwrap();
                assert_eq!(
                    signatures.field(&generic, "value").unwrap().1,
                    model.core_type("int", false)
                );
            }
            let static_value = model.resolve_name("StaticValue").unwrap();
            assert_eq!(
                signatures.field(&static_value, "value").unwrap().1,
                model.core_type("int", false)
            );
            assert_eq!(
                signatures.field(&static_value, "empty").unwrap().1,
                ResolvedType::DYNAMIC
            );
            for owner in ["Conflict", "GetterConflict"] {
                let identity = model.resolve_name(owner).unwrap();
                assert_eq!(
                    signatures.field(&identity, "value").unwrap().1,
                    ResolvedType::UNKNOWN
                );
            }
        },
    );
}

#[test]
fn null_global_initializers_infer_dynamic_except_explicit_null() {
    with_model(
        "var mutable = null; final immutable = null; const constant = null; Null explicit = null;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for name in ["mutable", "immutable", "constant"] {
                let identity = model.resolve_value(name).unwrap();
                assert_eq!(
                    signatures.value_type(&identity),
                    Some(&ResolvedType::DYNAMIC)
                );
            }
            let identity = model.resolve_value("explicit").unwrap();
            assert_eq!(signatures.value_type(&identity), Some(&ResolvedType::NULL));
        },
    );
}

#[test]
fn global_partial_inference_converges_without_expanding_recursive_containers() {
    with_parsed_core(
        "class Partial<T> { Partial(T value); int get length => 0; } \
         final nested = [items]; final items = [later]; final later = 1; \
         final self = [self]; final left = [right]; final right = [left]; \
         final missingCount = Partial(missing).length; \
         final selfCount = self.length; final partialSelfCount = Partial(self).length; \
         final missing = [unresolved]; final consumer = [self]; \
         List<int> annotated = [annotated]; final barrier = [annotated]; \
         final shadow = (shadow) => shadow;",
        "library dart.core; class Object {} abstract class int {} \
         abstract class List<E> { int get length; }",
        |_, model, signatures| {
            let value = |name| {
                signatures
                    .value_type(&model.resolve_value(name).unwrap())
                    .unwrap()
            };
            let nested = value("nested");
            assert!(nested.interface("dart:core", "List"));
            assert!(nested.arguments()[0].interface("dart:core", "List"));
            assert!(nested.arguments()[0].arguments()[0].interface("dart:core", "int"));
            // Cyclic values stay unknown, as does member access on them.
            for name in ["self", "left", "right", "selfCount"] {
                assert_eq!(value(name), &ResolvedType::UNKNOWN, "{name}");
            }
            let missing = value("missing");
            assert!(missing.interface("dart:core", "List"));
            assert_eq!(missing.arguments(), &[ResolvedType::UNKNOWN]);
            assert!(value("missingCount").interface("dart:core", "int"));
            assert!(value("partialSelfCount").interface("dart:core", "int"));
            let consumer = value("consumer");
            assert!(consumer.interface("dart:core", "List"));
            assert_eq!(consumer.arguments(), &[ResolvedType::UNKNOWN]);
            let barrier = value("barrier");
            assert!(barrier.arguments()[0].interface("dart:core", "List"));
            assert!(barrier.arguments()[0].arguments()[0].interface("dart:core", "int"));
            assert!(matches!(
                value("shadow"),
                ResolvedType {
                    kind: ResolvedTypeKind::Function { .. },
                }
            ));
        },
    );
}

#[test]
fn partial_dependency_retry_is_independent_of_declaration_order() {
    for declarations in [
        "final result = total; final total = count + 1; \
         final nested = [collected]; final collected = [count]; \
         final count = secondCopy.length; final secondCopy = firstCopy; \
         final firstCopy = partial; final partial = Partial([unresolved]);",
        "final partial = Partial([unresolved]); final firstCopy = partial; \
         final secondCopy = firstCopy; final count = secondCopy.length; \
         final collected = [count]; final nested = [collected]; \
         final total = count + 1; final result = total;",
    ] {
        let source =
            format!("class Partial<T> {{ Partial(T value); int get length => 0; }} {declarations}");
        with_model(&source, |program, model| {
            let files = [(
                PathBuf::from("/project/lib/main.dart"),
                model.node_ref(program.syntax()).file,
            )];
            for signatures in [
                SignatureIndex::from_program(program, model),
                SignatureIndex::from_project_files(&files, model.identities, model.types.unwrap()),
            ] {
                for name in ["count", "total", "result"] {
                    let ty = signatures.value_type(&model.resolve_value(name).unwrap());
                    assert!(
                        ty.is_some_and(|ty| ty.interface("dart:core", "int")),
                        "{name}: {ty:?}; {declarations}",
                    );
                }
                let collected = signatures
                    .value_type(&model.resolve_value("collected").unwrap())
                    .unwrap();
                assert!(collected.interface("dart:core", "List"));
                assert!(collected.arguments()[0].interface("dart:core", "int"));
                let nested = signatures
                    .value_type(&model.resolve_value("nested").unwrap())
                    .unwrap();
                assert!(nested.interface("dart:core", "List"));
                assert_eq!(nested.arguments(), std::slice::from_ref(collected));
                let partial = signatures
                    .value_type(&model.resolve_value("partial").unwrap())
                    .unwrap();
                assert!(has_inference_holes(partial), "{partial:?}");
                assert!(partial.arguments()[0].interface("dart:core", "List"));
                assert_eq!(partial.arguments()[0].arguments(), &[ResolvedType::UNKNOWN],);
                for name in ["firstCopy", "secondCopy"] {
                    assert_eq!(
                        signatures.value_type(&model.resolve_value(name).unwrap()),
                        Some(partial),
                        "{name}; {declarations}",
                    );
                }
            }
        });
    }
}

#[test]
fn imported_partial_global_inference_is_independent_of_file_order() {
    let main_path = PathBuf::from("/project/lib/main.dart");
    let origin_path = PathBuf::from("/project/lib/origin.dart");
    let main_source = "import 'origin.dart' as origin; final items = [origin.later]; \
         dynamic readItems() => items; dynamic readNull() => origin.empty; \
         final cycle = [origin.other];";
    let falcon_dart_parser::ParseResult {
        tree: main_tree,
        errors,
    } = parse(main_source);
    let main_parsed = Arc::new(crate::ParsedFile {
        source: main_source.into(),
        tree: main_tree,
    });
    let main = main_parsed.program();
    assert!(errors.is_empty(), "{errors:?}");
    let origin_source = "import 'main.dart' as root; final later = 1; var empty = null; \
         final other = [root.cycle];";
    let falcon_dart_parser::ParseResult {
        tree: origin_tree,
        errors,
    } = parse(origin_source);
    let origin_parsed = Arc::new(crate::ParsedFile {
        source: origin_source.into(),
        tree: origin_tree,
    });
    assert!(errors.is_empty(), "{errors:?}");
    let sources = [
        IdentitySource {
            path: &main_path,
            parsed: &main_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &origin_path,
            parsed: &origin_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    for files in [
        [
            (main_path.clone(), main_parsed.clone()),
            (origin_path.clone(), origin_parsed.clone()),
        ],
        [
            (origin_path.clone(), origin_parsed.clone()),
            (main_path.clone(), main_parsed.clone()),
        ],
    ] {
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let model = SemanticModel::new(&main_path, &main_parsed, &identities, Some(&types));
        let items = signatures
            .value_type(&model.resolve_value("items").unwrap())
            .unwrap();
        assert!(items.arguments()[0].interface("dart:core", "int"));
        let origin_model =
            SemanticModel::new(&origin_path, &origin_parsed, &identities, Some(&types));
        for identity in [
            model.resolve_value("cycle").unwrap(),
            origin_model.resolve_value("other").unwrap(),
        ] {
            assert_eq!(
                signatures.value_type(&identity),
                Some(&ResolvedType::UNKNOWN)
            );
        }
        for (declaration, expected) in main
            .declarations()
            .skip(1)
            .zip([items.clone(), ResolvedType::DYNAMIC])
        {
            let TopLevelDecl::Function(function) = declaration else {
                panic!()
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!()
            };
            let body = body.expr();
            assert_eq!(
                TypeEnvironment::new().infer_with_signatures(
                    body,
                    &model,
                    &signatures,
                    &TypeParameterScope::default(),
                ),
                expected,
            );
        }
    }
}

#[test]
fn bound_static_access_heads_use_instance_types_and_lexical_lookup() {
    with_model(
        "class Root { static int value = 1; } \
         class Other { Other get child => this; String get value => ''; } \
         dynamic direct(Other Root) => Root.value; \
         dynamic nested(Other Root) => Root.child.value;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            let other = ResolvedType::interface_identity(model.resolve_name("Other").unwrap());
            for declaration in program.declarations().skip(2) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!()
                };
                let body = body.expr();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert!(
                    environment
                        .infer_with_signatures(body, model, &signatures, &scope)
                        .interface("dart:core", "String")
                );
                assert!(
                    TypeEnvironment::new()
                        .infer_with_signatures_using_lookup(
                            body,
                            model,
                            &signatures,
                            &scope,
                            &|name| (name == "Root").then(|| other.clone()),
                        )
                        .interface("dart:core", "String")
                );
            }
        },
    );
}

#[test]
fn named_mixin_applications_preserve_inherited_collection_arguments() {
    with_model(
        "abstract class Base<A, B> implements List<B> { B get first; } \
         mixin ReadOnly<E> {} \
         abstract class Alias<K, V> = Base<K, V> with ReadOnly<V>; \
         abstract class View<T> extends Alias<String, T> {} \
         View<int> view;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver = signatures
                .value_type(&model.resolve_value("view").unwrap())
                .unwrap();
            let iterable = signatures
                .instantiated_supertype(receiver, "dart:core", "Iterable", model)
                .unwrap();
            assert!(iterable.arguments()[0].interface("dart:core", "int"));
            let (field, substitutions) =
                signatures.resolved_field(receiver, "first", model).unwrap();
            assert!(
                model
                    .substitute(&field, &substitutions)
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn call_constructor_resolution_bounds_qualified_names_without_losing_wrappers() {
    for (expression, allow_generics, expected) in [
        ("Local()", false, Some(("Local", "new"))),
        ("Local.named()", false, Some(("Local", "named"))),
        ("core.List()", false, Some(("List", "new"))),
        ("core.List.filled(0, 0)", false, Some(("List", "filled"))),
        ("(core.List).filled(0, 0)", false, Some(("List", "filled"))),
        ("core.List.filled.extra()", false, None),
        ("unknown()", false, None),
        ("Local<int>", false, None),
        ("Local<int>", true, Some(("Local", "new"))),
    ] {
        let source =
            format!("import 'dart:core' as core; class Local<T> {{}} final value = {expression};");
        with_model(&source, |program, model| {
            let TopLevelDecl::Variable(variable) = program.declarations().last().unwrap() else {
                panic!("expected variable");
            };
            let expression = variable
                .declarators()
                .next()
                .unwrap()
                .initializer()
                .unwrap();
            let callee = match expression.unparenthesized() {
                Expr::Call(call) => call.callee(),
                expression => expression,
            };
            let resolved = resolve_call_constructor(callee, model, allow_generics);
            assert_eq!(
                resolved
                    .as_ref()
                    .map(|(identity, name)| (identity.name(), *name)),
                expected,
                "{source}"
            );
        });
    }
}

#[test]
fn executable_const_bodies_exclude_external_constructors() {
    with_model(
        "class Value { const Value(); external const factory Value.environment(String name); }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let identity = model.resolve_name("Value").unwrap();
            assert!(signatures.const_constructor(&identity, "new").is_some());
            assert!(
                signatures
                    .const_constructor(&identity, "environment")
                    .is_none()
            );
        },
    );
}

#[test]
fn extension_method_resolution_preserves_receiver_uncertainty_and_nullability() {
    with_model(
        "extension OnObject on Object { int strict() => 0; } extension OnNullable on Object? { int nullable() => 0; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for receiver in [ResolvedType::DYNAMIC, ResolvedType::UNKNOWN] {
                assert!(
                    signatures
                        .resolved_member(&receiver, "strict", model)
                        .is_none()
                );
                assert!(
                    signatures
                        .resolved_member(&receiver, "nullable", model)
                        .is_none()
                );
            }
            let receiver = model.core_type("String", true);
            assert!(
                signatures
                    .resolved_member(&receiver, "strict", model)
                    .is_none()
            );
            assert!(
                signatures
                    .resolved_member(&receiver, "nullable", model)
                    .is_some()
            );
            assert!(
                signatures
                    .resolved_member(&receiver.with_nullable(false), "strict", model)
                    .is_some()
            );
        },
    );
}

#[test]
fn collection_scopes_and_record_contexts_infer_without_sdk_sources() {
    with_model(
        "abstract class Source<A, B> implements Iterable<B> {} \
         List<T> empty<T>() => []; \
         dynamic list(Source<String, int> source) => [for (var item in source) item]; \
         dynamic map(Source<String, int> source) => {for (var item in source) item: item}; \
         dynamic counted() => [for (var index = 0; index < 2; index++) index]; \
         dynamic promoted(Object? value) => [if (value is int) value]; \
         dynamic record() => (1, name: 'text'); \
         dynamic positional() => (1, name: 'text').$1; \
         dynamic named() => (1, name: 'text').name; \
         (List<int>, {List<String> names}) contextual() => (empty(), names: empty());",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations().skip(2) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!()
                };
                let body = body.expr();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let expected = function.return_type().map(|ty| model.resolve_type(ty));
                let actual = environment.infer_with_context(
                    body,
                    model,
                    &signatures,
                    &scope,
                    expected.as_ref(),
                    &|_| None,
                );
                let int = model.core_type("int", false);
                let string = model.core_type("String", false);
                match function.name().name() {
                    "list" | "counted" | "promoted" => assert_eq!(actual.arguments(), &[int]),
                    "map" => assert_eq!(actual.arguments(), &[int.clone(), int]),
                    "positional" => assert_eq!(actual, int),
                    "named" => assert_eq!(actual, string),
                    "record" => assert_eq!(
                        actual,
                        ResolvedType {
                            kind: ResolvedTypeKind::Record {
                                occurrence: Default::default(),
                                positional: vec![int].into(),
                                named: vec![("name".into(), string)].into(),
                                nullable: false,
                            },
                        }
                    ),
                    "contextual" => assert_eq!(Some(actual), expected),
                    _ => panic!(),
                }
            }
        },
    );
}

#[test]
fn contextual_generic_spreads_and_record_fields_have_exact_types() {
    with_model(
        "Map<K, V> emptyEntries<K, V>() => {}; List<T> emptyElements<T>() => []; \
         Map<int, String> map() => {...emptyEntries()}; \
         Map<int, String> conditional(bool flag) => {if (flag) ...emptyEntries()}; \
         Map<int, String> loop() => {for (var i = 0; i < 2; i++) ...emptyEntries()}; \
         Map<int, String> nullable() => {...?emptyEntries()}; \
         List<int> list() => [...emptyElements()]; \
         (List<int>, {List<String> names}) record() => (emptyElements(), names: emptyElements());",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations().skip(2) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!()
                };
                let body = body.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let actual = environment.infer_with_context(
                    body,
                    model,
                    &signatures,
                    &scope,
                    Some(&expected),
                    &|_| None,
                );
                assert_eq!(actual, expected, "{}", function.name().name());
            }
        },
    );
}

#[test]
fn spread_only_braces_infer_map_shape_from_operands_without_context() {
    with_parsed_core(
        "abstract class Source extends Map<String, String> {} \
         Map<String, String> direct(Map<String, String> input) => {...input}; \
         Map<String, String> nullable(Map<String, String>? input) => {...?input}; \
         Map<String, String> inherited(Source input) => {...input}; \
         Map<String, String> conditional(bool flag, Map<String, String> input) => {if (flag) ...input}; \
         Map<String, String> loop(List<Map<String, String>> inputs) => {for (final input in inputs) ...input}; \
         String value(Map<String, String> input) => {...input}.values.first; \
         Set<String> set(Set<String> input) => {...input}; \
         List<String> list(List<String> input) => [...input];",
        "library dart.core; class Object {} abstract class String {} abstract class bool {} \
         abstract class Iterable<E> { E get first; } \
         abstract class Map<K, V> { Iterable<V> get values; } \
         abstract class Set<E> implements Iterable<E> {} \
         abstract class List<E> implements Iterable<E> {}",
        |program, model, signatures| {
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_signatures(expression, model, signatures, &scope),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn collection_literal_element_inference() {
    let cases: &[(&str, &str, &[Option<&str>])] = &[
        ("['a', 'b']", "List", &[Some("String")]),
        ("{1, 2}", "Set", &[Some("int")]),
        ("[1, 2.5]", "List", &[Some("num")]),
        ("{1.5, 2}", "Set", &[Some("num")]),
        ("[i, ni]", "List", &[Some("int?")]),
        ("[ni, i]", "List", &[Some("int?")]),
        ("[ni, ni]", "List", &[Some("int?")]),
        ("[ni, d]", "List", &[Some("num?")]),
        ("[null, 'a']", "List", &[Some("String?")]),
        ("['a', null]", "List", &[Some("String?")]),
        ("[null, null]", "List", &[Some("Null")]),
        ("[true, 1]", "List", &[Some("Object")]),
        ("[null, true, 1]", "List", &[Some("Object?")]),
        ("[a, a]", "List", &[Some("A")]),
        ("[a, b]", "List", &[Some("Object")]),
        ("[missing, 1]", "List", &[None]),
        ("[x, 1]", "List", &[Some("dynamic")]),
        ("[]", "List", &[Some("dynamic")]),
        ("{}", "Map", &[Some("dynamic"), Some("dynamic")]),
        ("{'a': 1, 'b': 2.5}", "Map", &[Some("String"), Some("num")]),
        (
            "{null: 'a', 1: null}",
            "Map",
            &[Some("int?"), Some("String?")],
        ),
        ("{'a': a, 'b': b}", "Map", &[Some("String"), Some("Object")]),
        ("<num>[1, 2]", "List", &[Some("num")]),
        ("<String>{}", "Set", &[Some("String")]),
        (
            "<String, num>{'a': 1}",
            "Map",
            &[Some("String"), Some("num")],
        ),
        ("[...[1]]", "List", &[Some("int")]),
        ("[if (true) 1]", "List", &[Some("int")]),
        ("[for (final n in [1]) n]", "List", &[Some("int")]),
        ("{...{1}}", "Set", &[Some("int")]),
        (
            "{'known': 1, ...{'a': 1}}",
            "Map",
            &[Some("String"), Some("int")],
        ),
        (
            "{'known': 1, if (true) 'a': 1}",
            "Map",
            &[Some("String"), Some("int")],
        ),
        (
            "{'known': 1, for (final n in [1]) n: n}",
            "Map",
            &[Some("Object"), Some("int")],
        ),
        ("<int>[...[1]]", "List", &[Some("int")]),
        (
            "<String, int>{...{'a': 1}}",
            "Map",
            &[Some("String"), Some("int")],
        ),
    ];
    let mut source = String::from("class A {} class B {}");
    for (index, (expression, _, _)) in cases.iter().enumerate() {
        use std::fmt::Write;
        write!(
            source,
            "dynamic f{index}(int i, int? ni, double d, dynamic x, A a, B b) => {expression};"
        )
        .unwrap();
    }
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let scope = TypeParameterScope::default();
        for ((expression, name, arguments), declaration) in
            cases.iter().zip(program.declarations().skip(2))
        {
            let TopLevelDecl::Function(function) = declaration else {
                panic!()
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!()
            };
            let body = body.expr();
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), model, &scope);
            let actual = environment.infer_with_signatures(body, model, &signatures, &scope);
            assert!(
                actual.interface("dart:core", name),
                "{expression}: {actual:?}"
            );
            assert_eq!(actual.arguments().len(), arguments.len(), "{expression}");
            for (actual, expected) in actual.arguments().iter().zip(*arguments) {
                let expected = expected.map_or(ResolvedType::UNKNOWN, |ty| {
                    let program_source = format!("{ty} expected;");
                    let falcon_dart_parser::ParseResult { tree, errors } = parse(&program_source);
                    let parsed = Arc::new(crate::ParsedFile {
                        source: program_source.into(),
                        tree,
                    });
                    let program = parsed.program();
                    assert!(errors.is_empty(), "{errors:?}");
                    let TopLevelDecl::Variable(variable) = program.declarations().next().unwrap()
                    else {
                        panic!()
                    };
                    model.resolve_type(variable.var_type().unwrap())
                });
                assert_eq!(*actual, expected, "{expression}");
            }
        }
    });
}

#[test]
fn raw_owner_parameters_use_declared_bounds() {
    with_model(
        "class Box<T> { T? get value => null; } class Bounded<T extends num> { T? get value => null; } dynamic raw(Box box) => box.value; num? bounded(Bounded box) => box.value;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_signatures(expression, model, &signatures, &scope),
                    model.resolve_type(function.return_type().unwrap())
                );
            }
        },
    );
}

#[test]
fn generic_closure_inputs_complete_before_upward_bound_inference() {
    with_model(
        "T bounded<T extends Object>(bool Function(T) callback) => throw 0; \
         T unbounded<T>(bool Function(T) callback) => throw 0; \
         T typed<T>(void Function(T) first, void Function(T) second) => throw 0; \
         void consumeNum(num value) {} \
         Object useBounded() => bounded((value) => true); \
         Object? useUnbounded() => unbounded((value) => true); \
         num useTyped() => typed((num value) {}, (value) {}); \
         num useReverseTyped() => typed((value) {}, (num value) {}); \
         num useOrdinary() => typed(consumeNum, (value) {}); \
         num useReverseOrdinary() => typed((value) {}, consumeNum);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations().skip(4) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let Expr::Call(call) = expression else {
                    panic!()
                };
                let callee = call.callee();
                let type_args = call.type_args();
                let args = call.args();
                let environment = TypeEnvironment::new();
                let context = InferenceContext {
                    model,
                    signatures: &signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                let invocation = environment
                    .resolved_invocation(callee, type_args.clone(), &context)
                    .unwrap();
                let mut argument_contexts = HashMap::new();
                let substitutions = environment.infer_signature_substitutions_with_result(
                    invocation.inference_signature(),
                    args,
                    type_args,
                    &context,
                    None,
                    Some(&mut argument_contexts),
                );
                assert_eq!(
                    model.substitute(&invocation.signature.return_type, &substitutions),
                    model.resolve_type(function.return_type().unwrap()),
                );
                assert_eq!(argument_contexts.len(), 1);
                let ResolvedType {
                    kind: ResolvedTypeKind::Function { positional, .. },
                } = argument_contexts.values().next().unwrap()
                else {
                    panic!()
                };
                assert_eq!(positional.as_ref(), &[model.core_type("Object", true)]);
            }
        },
    );
}

#[test]
fn sibling_closure_results_contextualize_positional_and_named_inputs() {
    with_model(
        "class Target {} \
         class Owner<A> { B method<B>(void Function(A) callback) => throw 0; } \
         T positional<T>(T Function() create, void Function(T) dispose) => create(); \
         T named<T>({required T Function() create, required void Function(T) dispose}) => create(); \
         T reverse<T>(void Function(T) dispose, T Function() create) => create(); \
         T pick<T>(T Function() first, T Function() second) => first(); \
         Target usePositional() => positional(() => Target(), (value) {}); \
         Target useNamed() => named(create: () => Target(), dispose: (value) {}); \
         Target useReverse() => reverse((value) {}, () => Target()); \
         Target useReverseNamed() => named(dispose: (value) {}, create: () => Target()); \
         num independentNumbers() => pick(() => 1.5, () => 2); \
         List<dynamic> independentLists() => pick(() => <int>[], () => []); \
         dynamic ownerHole(Owner<Missing> owner) => owner.method((value) {});",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations().skip(6) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let Expr::Call(call) = expression else {
                    panic!()
                };
                let callee = call.callee();
                let type_args = call.type_args();
                let args = call.args();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let context = InferenceContext {
                    model,
                    signatures: &signatures,
                    type_parameters: &scope,
                    lookup: &|_| None,
                    argument_fallback: None,
                };
                let invocation = environment
                    .resolved_invocation(callee, type_args.clone(), &context)
                    .unwrap();
                let mut argument_contexts = HashMap::new();
                let substitutions = environment.infer_signature_substitutions_with_result(
                    invocation.inference_signature(),
                    args,
                    type_args,
                    &context,
                    None,
                    Some(&mut argument_contexts),
                );
                assert_eq!(
                    model.substitute(&invocation.signature.return_type, &substitutions),
                    model.resolve_type(function.return_type().unwrap()),
                );
                assert!(argument_contexts.is_empty(), "{}", function.name().name());
            }
        },
    );
}

#[test]
fn conditional_callable_and_raw_interface_joins_preserve_inference() {
    with_model(
        "class S<T> { final T data; S(this.data); } class E {} \
         String encode(String value, {int encoding = 0}) => value; \
         String Function(String) callback(bool flag) => flag ? encode : (e) => e; \
         S<dynamic> choose(dynamic value) => value is S ? value : S(E());",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if function.name().name() == "encode" {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    ),
                    expected,
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn conditional_generic_tearoffs_preserve_alpha_equivalent_binders() {
    with_model(
        "T first<T extends num>(T value) => value; \
         U second<U extends num>(U value) => value; \
         int joined(bool flag) => (flag ? first : second)(1);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().nth(2).unwrap() else {
                panic!()
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!()
            };
            let expression = expression.expr();
            let scope = TypeParameterScope::default();
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), model, &scope);
            assert_eq!(
                environment.infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &scope,
                    None,
                    &|_| None,
                ),
                model.core_type("int", false),
            );
        },
    );
}

#[test]
fn primary_private_fields_expose_public_named_signature_labels() {
    with_model(
        "class C({required final double _progress, final int _count = 0});",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let identity = model.resolve_name("C").unwrap();
            let signature = signatures.constructor(&identity, "new").unwrap();
            assert_eq!(
                signature.named.get("progress"),
                Some(&model.core_type("double", false))
            );
            assert_eq!(
                signature.named.get("count"),
                Some(&model.core_type("int", false))
            );
            assert!(!signature.named.contains_key("_progress"));
            assert!(signature.defaults.contains_key("count"));
            assert_eq!(
                signature.shape.as_ref().unwrap().required_named.as_ref(),
                &["progress"]
            );
        },
    );
}

#[test]
fn generic_calls_infer_from_inherited_method_tearoffs() {
    with_model(
        "abstract class Random { double nextDouble(); } \
         class Mock {} class MockRandom extends Mock implements Random {} \
         class When<T> { T thenReturn(T value) => value; } \
         When<T> when<T>(T Function() callback) => throw 0; \
         double probe(MockRandom random) => when(random.nextDouble).thenReturn(1.0);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().nth(5).unwrap() else {
                panic!()
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!()
            };
            let expression = expression.expr();
            let scope = TypeParameterScope::default();
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), model, &scope);
            assert_eq!(
                environment.infer_with_signatures(expression, model, &signatures, &scope),
                model.core_type("double", false),
            );
        },
    );
}

#[test]
fn long_binary_chain_infers_on_the_normal_test_stack() {
    let source = format!("int sum(int a) => {}a;", "a + ".repeat(99_999));
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let scope = TypeParameterScope::default();
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!()
        };
        let Some(FunctionBody::Arrow(body)) = function.body() else {
            panic!()
        };
        let mut environment = TypeEnvironment::new();
        environment.bind_params(function.params().unwrap(), model, &scope);
        assert_eq!(
            environment.infer_with_signatures(body.expr(), model, &signatures, &scope),
            model.core_type("int", false),
        );
    });
}

#[test]
fn binary_chain_preserves_each_operator_parameter_context() {
    with_parsed_core(
        "T empty<T>() => throw 0; int sum(int a) => a + empty() + empty();",
        "library dart.core; class Object {} abstract class num {} \
         abstract class int extends num { int operator +(int other); }",
        |program, model, signatures| {
            let scope = TypeParameterScope::default();
            let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap() else {
                panic!()
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!()
            };
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), model, &scope);
            assert_eq!(
                environment.infer_with_signatures(body.expr(), model, signatures, &scope),
                model.core_type("int", false),
            );
        },
    );
}

#[test]
fn generic_calls_infer_from_arguments_and_result_context() {
    with_model(
        concat!(
            "import 'dart:async'; T identity<T>(T value) => value; T empty<T>() => throw 0; T merge<T>(T first, T second) => first; T namedIdentity<T>({required T value}) => value; T futureValue<T>(FutureOr<T> value) => throw 0; ",
            "T transform<T>(T Function(T) callback, T value) => callback(value); T inputType<T>(void Function(T) callback) => throw 0; ",
            "class Box<T> { final T value; Box(this.value); Box.named(this.value); T choose<U>(U input, T value) => value; U convert<U>(U value) => value; } class Fields { int Function() callback; Fields(this.callback); } ",
            "int arg() => identity(1); String named() => identity<String>('x'); int namedArgument() => namedIdentity(value: 1); Box<int> create() => Box(1); Box<int> explicitNew() => new Box(1); Box<int> explicitNamed() => Box<int>.named(1); Box<int> cascade() => Box(1)..convert(2); ",
            "int method(Box<String> box) => box.convert(1); String owner(Box<String> box) => box.choose(1, 'x'); int variable(int Function() callback) => callback(); int field(Fields fields) => fields.callback(); ",
            "int variance(int Function(Object) callback) => transform(callback, 1); int upperOnly(void Function(int) callback) => inputType(callback); ",
            "typedef NumBox = Box<num>; NumBox aliasNew() => new NumBox(1); num nestedSibling() => merge(identity(1), 2.0); int? nullableJoin(int? value) => merge(1, value); int? nullableCall(Fields? fields) => fields?.callback(); ",
            "int lexicalClosure() => transform((int x) { final value = x * 2; return value; }, 1); int lexicalArrow() => transform((int x) => x, 1); int tearoffTarget() => 1; int Function() tearoff() => tearoffTarget; num contextNumber() => identity(1.0); ",
            "Box<num> contextBoxNumber() => Box(1); Box<double> contextBoxDouble() => Box(1.0); double contextDouble() => identity(1.0); int? futureNullable(Future<int>? future) => futureValue(future); ",
            "num common(bool flag) => identity(flag ? 1 : 2.0); num lowerBounds() => merge(1, 2.0); int futureScalar() => futureValue(1); int futureWrapped(Future<int> future) => futureValue(future); int context() => empty(); int contextNested() => identity(empty());",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if !function.type_params().is_empty() {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_context(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None
                    ),
                    expected,
                    "{}",
                    function.name().name(),
                );
                if !function.name().name().starts_with("context") {
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, &signatures, &scope),
                        expected
                    );
                } else if function.name().name() == "contextNumber"
                    || function.name().name() == "contextDouble"
                {
                    assert_eq!(
                        environment.infer_with_signatures(expression, model, &signatures, &scope),
                        model.core_type("double", false)
                    );
                } else if function.name().name() == "contextBoxNumber" {
                    let ResolvedType {
                        kind: ResolvedTypeKind::Interface { arguments, .. },
                    } = environment.infer_with_signatures(expression, model, &signatures, &scope)
                    else {
                        panic!()
                    };
                    assert_eq!(arguments.as_ref(), &[model.core_type("int", false)]);
                }
            }
        },
    );
}

#[test]
fn omitted_generic_arguments_use_defaults_without_erasing_unknown_evidence() {
    with_model(
        "class Box<T> { Box([T? value]); } \
         class Bounded<T extends num> { Bounded(); } \
         typedef Alias<T> = Box<T>; \
         Box<dynamic> omitted() => Box(); \
         Box<dynamic> alias() => Alias(); \
         Bounded<num> bounded() => Bounded(); \
         Box<int> explicit() => Box<int>(); \
         Box<int> supplied() => Box(1); \
         Box<dynamic> unresolved() => Box(missing);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let actual = TypeEnvironment::new().infer_with_signatures(
                    expression,
                    model,
                    &signatures,
                    &scope,
                );
                if function.name().name() == "unresolved" {
                    assert_eq!(actual.arguments(), &[ResolvedType::UNKNOWN]);
                } else {
                    assert_eq!(
                        actual,
                        model.resolve_type(function.return_type().unwrap()),
                        "{}",
                        function.name().name(),
                    );
                }
            }
        },
    );
}

#[test]
fn function_context_coerces_callable_values_but_not_unrelated_values() {
    with_model(
        "class Callable { void call() {} } \
         void Function() callback(Callable value) => value; \
         Callable ordinary(Callable value) => value; \
         void Function() unrelated(Object value) => value;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let expected = model.resolve_type(function.return_type().unwrap());
                let actual = environment.infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &scope,
                    Some(&expected),
                    &|_| None,
                );
                let parameter = model.resolve_formal_parameter_in(
                    function.params().unwrap().params().next().unwrap(),
                    &scope,
                );
                assert_eq!(
                    actual,
                    if function.name().name() == "unrelated" {
                        parameter
                    } else {
                        expected
                    },
                    "{}",
                    function.name().name(),
                );
                assert_eq!(
                    environment.infer_with_signatures(expression, model, &signatures, &scope),
                    model.resolve_formal_parameter_in(
                        function.params().unwrap().params().next().unwrap(),
                        &scope
                    ),
                );
            }
        },
    );
}

#[test]
fn record_schema_downcasts_dynamic_fields_without_downcasting_object_fields() {
    with_model(
        "(String, int) dynamicField(dynamic value) => (value, 1); \
         (String, int) objectField(Object value) => (value, 1); \
         String standalone(dynamic value) => value;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let expected = model.resolve_type(function.return_type().unwrap());
                let actual = environment.infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &scope,
                    Some(&expected),
                    &|_| None,
                );
                match function.name().name() {
                    "dynamicField" => assert_eq!(actual, expected),
                    "objectField" => {
                        let ResolvedType {
                            kind: ResolvedTypeKind::Record { positional, .. },
                        } = actual
                        else {
                            panic!()
                        };
                        assert_eq!(positional[0], model.core_type("Object", false));
                    }
                    "standalone" => assert_eq!(actual, ResolvedType::DYNAMIC),
                    _ => unreachable!(),
                }
            }
        },
    );
}

#[test]
fn mixin_application_constructors_retain_names_types_and_defaults() {
    with_model(
        concat!(
            "class Base<T> { Base(T value); Base.named(T value, [int count = 7]); ",
            "factory Base.factory(T value) => Base(value); } ",
            "mixin Extra {} class Alias<U> = Base<List<U>> with Extra; ",
            "class Chained<V> = Alias<V> with Extra; ",
            "class Child extends Chained<String> { Child(super.value, [super.count]) : super.named(); }",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = model.resolve_name("Base").unwrap();
            for name in ["Alias", "Chained"] {
                let owner = model.resolve_name(name).unwrap();
                assert!(signatures.constructor(&owner, "new").is_some());
                assert!(signatures.constructor(&owner, "factory").is_none());
                let named = signatures.constructor(&owner, "named").unwrap();
                assert_eq!(
                    named.owner_parameters,
                    signatures.type_parameters(&owner).unwrap()
                );
                assert!(matches!(
                    &named.defaults["count"],
                    ParameterDefault::Inherited { owner, .. } if *owner == base
                ));
                let facts = signatures.declaration(&owner).unwrap();
                assert_eq!(facts.constructors.len(), 2);
                let facts = facts
                    .constructors
                    .iter()
                    .find(|facts| facts.name == "named")
                    .unwrap();
                assert_eq!(facts.parameters[0].ty, named.positional[0]);
            }
            let child = model.resolve_name("Child").unwrap();
            let signature = signatures.constructor(&child, "new").unwrap();
            assert!(
                matches!(
                    &signature.positional[0],
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface { identity, arguments, .. },
                    } if identity.is_sdk("dart:core", "List")
                        && arguments[0].interface("dart:core", "String")
                ),
                "forwarded parameter: {:?}",
                signature.positional[0],
            );
            assert!(matches!(
                &signature.defaults["count"],
                ParameterDefault::Inherited { owner, .. } if *owner == base
            ));
        },
    );
}

#[test]
fn mixin_application_constness_requires_known_fieldless_mixins() {
    with_model(
        concat!(
            "class Base { const Base(); Base.named(); } ",
            "mixin Stateless {} mixin StaticOnly { static int value = 1; } ",
            "mixin Mutable { int value = 1; } mixin FinalField { final int value = 1; } ",
            "class EmptyAlias = Base with Stateless; ",
            "class StaticAlias = Base with StaticOnly; ",
            "class MutableAlias = Base with Mutable; ",
            "class FinalAlias = Base with FinalField; ",
            "class UnknownAlias = Base with Unresolved; ",
            "class MixedAlias = Base with Stateless, FinalField; ",
            "class Chained = MutableAlias with Stateless;",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (name, expected) in [
                ("EmptyAlias", true),
                ("StaticAlias", true),
                ("MutableAlias", false),
                ("FinalAlias", false),
                ("UnknownAlias", false),
                ("MixedAlias", false),
                ("Chained", false),
            ] {
                let owner = model.resolve_name(name).unwrap();
                assert_eq!(
                    signatures.is_const_constructor(&owner, "new"),
                    expected,
                    "{name}",
                );
                assert!(signatures.constructor(&owner, "new").is_some(), "{name}");
                assert!(!signatures.is_const_constructor(&owner, "named"), "{name}");
            }
        },
    );
}

#[test]
fn generated_mixin_application_constructors_drop_superclass_annotations() {
    with_model(
        concat!(
            "import 'package:meta/meta.dart'; ",
            "class Base { @awaitNotRequired const Base(); } ",
            "mixin Extra {} class Alias = Base with Extra;",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = model.resolve_name("Base").unwrap();
            assert_eq!(
                signatures.declaration(&base).unwrap().constructors[0].annotations,
                [DeclarationIdentity::package("meta", "awaitNotRequired")],
            );
            let alias = model.resolve_name("Alias").unwrap();
            let facts = &signatures.declaration(&alias).unwrap().constructors[0];
            assert!(facts.annotations.is_empty());
            assert!(facts.is_const);
        },
    );
}

#[test]
fn mixin_application_private_constructors_are_library_scoped() {
    let base_path = PathBuf::from("/project/lib/base.dart");
    let alias_path = PathBuf::from("/project/lib/alias.dart");
    let base_source = concat!(
        "class Base { Base(); Base._private(); } mixin Extra {} ",
        "class LocalAlias = Base with Extra;",
    );
    let falcon_dart_parser::ParseResult {
        tree: base_tree,
        errors: base_errors,
    } = parse(base_source);
    let base_parsed = Arc::new(crate::ParsedFile {
        source: base_source.into(),
        tree: base_tree,
    });
    let alias_source = "import 'base.dart'; class ImportedAlias = Base with Extra;";
    let falcon_dart_parser::ParseResult {
        tree: alias_tree,
        errors: alias_errors,
    } = parse(alias_source);
    let alias_parsed = Arc::new(crate::ParsedFile {
        source: alias_source.into(),
        tree: alias_tree,
    });
    assert!(base_errors.is_empty());
    assert!(alias_errors.is_empty());
    let sources = [
        IdentitySource {
            path: &alias_path,
            parsed: &alias_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &base_path,
            parsed: &base_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files = [
        (alias_path.clone(), alias_parsed.clone()),
        (base_path.clone(), base_parsed.clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    for (path, parsed, name, private_visible) in [
        (&base_path, &base_parsed, "LocalAlias", true),
        (&alias_path, &alias_parsed, "ImportedAlias", false),
    ] {
        let model = SemanticModel::new(path, parsed, &identities, Some(&types));
        let owner = model.resolve_name(name).unwrap();
        assert!(signatures.constructor(&owner, "new").is_some());
        assert_eq!(
            signatures.constructor(&owner, "_private").is_some(),
            private_visible,
        );
        assert_eq!(
            signatures
                .declaration(&owner)
                .unwrap()
                .constructors
                .iter()
                .any(|facts| facts.name == "_private"),
            private_visible,
        );
    }
}

#[test]
fn mixin_application_constructor_facts_support_documented_selectors() {
    with_model(
        "class Base { Base(); Base.named(); } mixin Extra {} class Alias = Base with Extra;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let alias = model.resolve_name("Alias").unwrap();
            for name in ["new", "named"] {
                assert!(signatures.constructor(&alias, name).is_some());
                assert!(
                    signatures
                        .declaration(&alias)
                        .unwrap()
                        .constructors
                        .iter()
                        .any(|facts| facts.name == name)
                );
            }
        },
    );
}

#[test]
fn generic_alias_constructors_infer_alias_parameters_upward() {
    with_model(
        concat!(
            "class Box<T> { final T value; Box(this.value); Box.named(this.value); } class Pair<K, V> { Pair(K key, V value); } ",
            "typedef Alias<T> = Box<T>; typedef Same<T> = Pair<T, T>; typedef Flipped<A, B> = Pair<B, A>; ",
            "Box<int> call() => Alias(1); Box<int> explicitNew() => new Alias(1); Box<String> named() => Alias.named('x'); ",
            "Box<num> explicitArguments() => Alias<num>(1); Pair<num, num> same() => Same(1, 2.0); Pair<int, String> flipped() => Flipped(1, 'x');",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope
                    ),
                    expected,
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn downward_generic_candidates_obey_bounds_and_raw_owner_contexts() {
    with_model(
        "T minimum<T extends num>(T first, T second) => first; \
         class Sequence<T extends Object> { Sequence.single(T value); } \
         class Items<T extends Object> extends Sequence<T> { Items.single(T value); } \
         int? bounded(int first, int second) => minimum(first, second); \
         Sequence rawContext() => Sequence.single((Object value) {}); \
         Sequence rawInheritedContext() => Items.single((Object value) {});",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if !matches!(
                    function.name().name(),
                    "bounded" | "rawContext" | "rawInheritedContext"
                ) {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let inferred = environment.infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &scope,
                    Some(&expected),
                    &|_| None,
                );
                if function.name().name() == "bounded" {
                    assert_eq!(inferred, expected.non_null());
                } else {
                    assert!(inferred.arguments()[0].interface("dart:core", "Object"));
                }
            }
        },
    );
}

#[test]
fn declared_bounds_meet_top_contexts_and_instantiate_unconstrained_tearoffs() {
    with_model(
        concat!(
            "class Route<T> {} class Routes { static Route<T>? of<T extends Object?>(int context) => null; } ",
            "Route<T>? find<T extends Object?>() => null; Route<T>? any<T>() => null; ",
            "class Open<T extends Object?> { Open(); } class Narrow<T extends num> { Narrow(); } class Raw<T> { Raw(); } ",
            "Route<dynamic>? staticCall(int context) => Routes.of(context); ",
            "Route<dynamic>? functionCall() => find(); ",
            "Route<dynamic>? omittedBound() => any(); ",
            "Open<dynamic> openConstructor() => Open(); ",
            "Object Function() boundedTearoff() => Narrow.new; ",
            "Object Function() rawTearoff() => Raw.new; ",
            "typedef Alias<X> = Raw<X>; typedef BoundedAlias<X extends Object?> = Raw<X>; ",
            "Object Function() aliasTearoff() => Alias.new; ",
            "Raw<dynamic> aliasConstructor() => Alias(); ",
            "Raw<dynamic> boundedAliasConstructor() => BoundedAlias(); ",
            "Raw<dynamic> Function() boundedAliasTearoff() => BoundedAlias.new;",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            let dynamic = ResolvedType::DYNAMIC;
            let object = model.core_type("Object", true);
            let num = model.core_type("num", false);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                if !function.type_params().is_empty() {
                    continue;
                }
                let expected = model.resolve_type(function.return_type().unwrap());
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let inferred = environment.infer_with_context(
                    expression,
                    model,
                    &signatures,
                    &scope,
                    Some(&expected),
                    &|_| None,
                );
                let instance = match &inferred {
                    ResolvedType {
                        kind: ResolvedTypeKind::Function { return_type, .. },
                    } => return_type.as_ref(),
                    other => other,
                };
                let argument = match function.name().name() {
                    "omittedBound" | "rawTearoff" | "aliasTearoff" | "aliasConstructor" => &dynamic,
                    "boundedTearoff" => &num,
                    _ => &object,
                };
                assert_eq!(
                    instance.arguments(),
                    std::slice::from_ref(argument),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn raw_interfaces_match_generic_formals_at_their_bounds() {
    with_model(
        concat!(
            "class Box<T> {} class Bounded<T extends num> {} T unbox<T>(Box<T> box) => throw 0; T unbound<T>(Bounded<T> box) => throw 0; ",
            "T first<T>(Iterable<T> values) => throw 0; ",
            "dynamic raw(Box box) => unbox(box); num bounded(Bounded box) => unbound(box); dynamic rawList(List list) => first(list);",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if !function.type_params().is_empty() {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_signatures(expression, model, &signatures, &scope),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn generic_function_values_infer_invocation_binders() {
    with_model(
        "class Stub<T> { T value() => throw 0; } \
         Stub<T> Function<T>(T Function()) get when => throw 0; \
         class Factory { final Stub<T> Function<T>(T Function()) create; Factory(this.create); } \
         double nextDouble() => 0; \
         Stub<double> getterInferred() => when(nextDouble); \
         Stub<int> getterExplicit() => when<int>(() => 1); \
         Stub<double> fieldInferred(Factory factory) => factory.create(nextDouble); \
         Stub<int> fieldExplicit(Factory factory) => factory.create<int>(() => 1); \
         Stub<int> fieldCall(Factory factory) => factory.create.call<int>(() => 1); \
         double chained() => when(nextDouble).value();",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                if function.name().name() == "nextDouble" {
                    continue;
                }
                let Some(params) = function.params() else {
                    continue;
                };
                let mut environment = TypeEnvironment::new();
                environment.bind_params(params, model, &scope);
                assert_eq!(
                    environment.infer_with_signatures(expression, model, &signatures, &scope),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn partial_function_contexts_preserve_tearoffs_and_coalescing_elements() {
    with_model(
        "T identity<T>(T value) => value; \
         R apply<T, R>(T value, R Function(T) callback) => callback(value); \
         class Box<T> { Box(T value); } \
         class Mapper { T convert<T>(T value) => value; } \
         class Chain<E> { Chain(E value); Chain<R> map<R>(R Function(E) callback) => throw 0; } \
         class Holder { final List<int>? items; Holder(this.items); } \
         dynamic functionTearoff() => apply(1, identity); \
         Box<dynamic> constructorTearoff() => apply(1, Box.new); \
         dynamic methodTearoff() => apply(1, Mapper().convert); \
         Chain<int> receiverFunctionTearoff() => Chain(1).map(identity); \
         Chain<int> receiverMethodTearoff() => Chain(1).map(Mapper().convert); \
         List<int> coalescing() => apply(Holder(null), (value) => value.items ?? []); \
         extension Twice on int { get twice => this * 2; } \
         dynamic implicitGetter() => 1.twice;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                if !function.type_params().is_empty() {
                    continue;
                }
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope,
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn inferred_field_formals_refresh_reversed_super_chains() {
    with_model(
        "class Leaf<T> extends Middle<T> { Leaf(super.items); } \
         class Middle<T> extends Box<T> { Middle(super.items); } \
         class Box<T> { var items = <T>[]; Box(this.items); } \
         Box<int> direct() => Box([1]); \
         Middle<int> middle() => Middle([1]); \
         Leaf<int> leaf() => Leaf([1]);",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope,
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
            for name in ["Box", "Middle", "Leaf"] {
                let identity = model.resolve_name(name).unwrap();
                let signature = signatures.constructor(&identity, "new").unwrap();
                let facts = signatures.declaration(&identity).unwrap();
                assert_eq!(
                    facts.constructors[0].parameters[0].ty,
                    signature.positional[0]
                );
                assert!(!has_inference_holes(&signature.positional[0]));
            }
        },
    );
}

#[test]
fn block_closures_preserve_private_field_promotion_mode() {
    with_model(
        "class Promoted { final int? _x; Promoted(this._x); \
         int Function() closure() => () { if (_x != null) return _x; return 0; }; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Class(class) = program.declarations().next().unwrap() else {
                panic!("expected class");
            };
            let method = class
                .members()
                .find_map(|member| match member {
                    ClassMember::Method(method) => Some(method),
                    _ => None,
                })
                .unwrap();
            let Some(FunctionBody::Arrow(expression)) = method.body() else {
                panic!("expected arrow body");
            };
            let expression = expression.expr();
            let receiver = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Promoted").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            let mut environment = TypeEnvironment::with_field_promotions(true);
            environment.declare_receiver(receiver.clone());
            let lookup = |name: &str| {
                signatures
                    .resolved_field(&receiver, name, model)
                    .map(|(ty, substitutions)| model.substitute(&ty, &substitutions))
            };
            assert_eq!(
                environment.infer_with_signatures_using_lookup(
                    expression,
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                    &lookup,
                ),
                model.resolve_type(method.return_type().unwrap()),
            );
        },
    );
}

#[test]
fn block_closures_demote_guarded_locals_after_writes() {
    with_model(
        "int? Function(int?, int?) guarded() => (int? a, int? b) { \
         int? value = a; if (value == null) return 0; value = b; return value; }; \
         int? Function(int?, int?, bool) nested() => (int? a, int? b, bool flag) { \
         int? value = a; if (value == null) return 0; \
         flag ? (value = b) : 0; return value; };",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn nested_coalescing_preserves_collection_and_callable_contexts() {
    with_model(
        "class Callable { int call() => 1; } \
         List<num> collection(List<num>? a, List<num> b) => [] ?? a ?? b; \
         int Function() callable(Callable? a, Callable b) => a ?? b ?? b;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!("expected arrow body");
                };
                let expected = model.resolve_type(function.return_type().unwrap());
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert_eq!(
                    environment.infer_with_context(
                        body.expr(),
                        model,
                        &signatures,
                        &scope,
                        Some(&expected),
                        &|_| None,
                    ),
                    expected,
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn block_closures_visit_long_optional_chains_on_the_normal_test_stack() {
    for operator in ["&&", "||", "??"] {
        let chain = format!("{}flag", format!("flag {operator} ").repeat(99_999));
        for statement in [
            format!("final value = {chain};"),
            format!("consume({chain});"),
            format!("{chain};"),
        ] {
            let source = format!(
                "bool Function(bool) chained() => (bool flag) {{ {statement} return flag; }}; \
                 void consume(bool value) {{}}"
            );
            with_model(&source, |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let TopLevelDecl::Function(function) = program.declarations().next().unwrap()
                else {
                    panic!("expected function");
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!("expected arrow body");
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        body.expr(),
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{operator}: {statement}",
                );
            });
        }
    }
}

#[test]
fn block_closures_preserve_optional_binary_write_order_and_joins() {
    with_model(
        "int? Function(bool, int?) andWrite() => (bool flag, int? other) { \
         int? value = other; if (value == null) return 0; \
         flag && ((value = other) != null); return value; }; \
         int? Function(bool, int?) orWrite() => (bool flag, int? other) { \
         int? value = other; if (value == null) return 0; \
         flag || ((value = other) != null); return value; }; \
         int? Function(int?, int?) coalesceWrite() => (int? first, int? other) { \
         int? value = other; if (value == null) return 0; \
         first ?? (value = other); return value; }; \
         int Function(bool, int?) ordered() => (bool flag, int? other) { \
         int? value = other; ((value = 1) != null) && flag || flag; return value; };",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!("expected function");
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!("expected arrow body");
                };
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        body.expr(),
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn long_conditions_preserve_only_unconditional_completed_promotions() {
    for operator in ["&&", "||", "??"] {
        let chain = format!(
            "(value! == 0) {operator} {}(other! == 0)",
            format!("flag {operator} ").repeat(99_998),
        );
        let source = format!("bool test(int? value, int? other, bool flag) => {chain};");
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!("expected function");
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!("expected arrow body");
            };
            for truth in [false, true] {
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                super::flow::FlowContext {
                    model,
                    signatures: &signatures,
                    type_parameters: &scope,
                    field_promotions: false,
                }
                .assume_condition(&mut environment, body.expr(), truth);
                assert_eq!(environment.lookup("value"), model.core_type("int", false),);
                let other_promoted = (operator == "&&" && truth) || (operator == "||" && !truth);
                assert_eq!(
                    environment.lookup("other"),
                    model.core_type("int", !other_promoted),
                    "{operator}: {truth}",
                );
            }
        });
    }
}

#[test]
fn block_closures_demote_branch_locals_after_writes() {
    with_parsed_core(
        "int? Function(List<int?>) branch() => (List<int?> values) { \
         int? value = values.first; if (value != null) { \
         value = values.last; return value; } return 0; };",
        "class Object {} class num extends Object {} class int extends num {} \
         class List<E> { E get first => throw 0; E get last => throw 0; }",
        |program, model, signatures| {
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!("expected function");
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!("expected arrow body");
            };
            let expression = expression.expr();
            assert_eq!(
                TypeEnvironment::new().infer_with_signatures(
                    expression,
                    model,
                    signatures,
                    &TypeParameterScope::default(),
                ),
                model.resolve_type(function.return_type().unwrap()),
            );
        },
    );
}

#[test]
fn block_closures_keep_optional_loop_writes_nullable() {
    with_model(
        "int? Function(bool) loop() => (bool condition) { \
         int? value; while (condition) { value = 1; condition = false; } return value; }; \
         int? Function(bool) collection() => (bool condition) { \
         int? value; [if (condition) value = 1]; return value; };",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn block_closures_keep_optional_switch_writes_nullable() {
    with_model(
        "int? Function(int) switched() => (int key) { int? value; \
         switch (key) { case 0: value = 1; default: break; } return value; }; \
         int? Function(int) expression() => (int key) { int? value; \
         (switch (key) { 0 => value = 1, _ => 0 }); return value; };",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn block_closures_keep_exceptional_writes_nullable() {
    with_model(
        "int? Function() caught() => () { int? value; \
         try { value = parse(); } catch (_) {} return value; }; int parse() => 1;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!("expected function");
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!("expected arrow body");
            };
            let expression = expression.expr();
            assert_eq!(
                TypeEnvironment::new().infer_with_signatures(
                    expression,
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                ),
                model.resolve_type(function.return_type().unwrap()),
            );
        },
    );
}

#[test]
fn block_closures_preserve_locals_and_guards_inside_optional_paths() {
    with_parsed_core(
        "int Function(List<int>) locals() => (List<int> values) { \
         for (final value in values) { final doubled = value * 2; return doubled; } return 0; }; \
         int Function(List<int?>) guards() => (List<int?> values) { \
         for (final value in values) { if (value == null) continue; return value; } return 0; }; \
         int Function() caught() => () { \
         try { final result = parse(); return result; } catch (_) { return 0; } }; \
         int parse() => 1;",
        "class Object {} class num extends Object {} class int extends num {} \
         class Iterable<E> {} class List<E> extends Iterable<E> {}",
        |program, model, signatures| {
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn block_closures_isolate_alternatives_and_loop_back_edges() {
    with_parsed_core(
        "int? Function() caught() => () { int? value = maybe(); \
         try { if (value == null) return 0; invoke(); } \
         catch (_) { return value; } return 1; }; \
         int? Function(int, int?) switched() => (int key, int? value) { \
         switch (key) { case 0: if (value == null) return 0; break; \
         case 1: return value; } return 2; }; \
         int? Function(Object, int?) patterned() => (Object key, int? value) { \
         if (key case int _) { if (value == null) return 0; } \
         else { return value; } return 1; }; \
         int? Function(int?) loop() => (int? value) { \
         if (value == null) return 0; \
         for (var i = 0; i < 3; i++) { \
         if (i == 2) return value; value = maybe(); } return 0; }; \
         int? maybe() => null; void invoke() {}",
        "class Object {} class num extends Object {} class int extends num {}",
        |program, model, signatures| {
            for declaration in program.declarations().take(4) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!("expected function");
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!("expected arrow body");
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn deep_generic_invocation_chains_infer_in_linear_time() {
    const DEPTH: usize = 48;
    let calls = format!("{}1{}", "identity(".repeat(DEPTH), ")".repeat(DEPTH));
    let methods = format!("Box(1){}", ".map((value) => value)".repeat(DEPTH));
    let named = format!("Box(1){}", ".named(f: (value) => value)".repeat(DEPTH));
    let blocks = format!("Box(1){}", ".map((value) { return value; })".repeat(DEPTH));
    let source = format!(
        "T identity<T>(T value) => value; class Box<T> {{ Box(T value); Box<U> map<U>(U Function(T) f) => throw 0; Box<U> named<U>({{required U Function(T) f}}) => throw 0; }} int calls() => {calls}; Box<int> methods() => {methods}; Box<int> named() => {named}; Box<int> blocks() => {blocks};",
    );
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let scope = TypeParameterScope::default();
        for declaration in program.declarations() {
            let TopLevelDecl::Function(function) = declaration else {
                continue;
            };
            if !function.type_params().is_empty() {
                continue;
            }
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                continue;
            };
            let expression = expression.expr();
            assert_eq!(
                TypeEnvironment::new().infer_with_signatures(
                    expression,
                    model,
                    &signatures,
                    &scope
                ),
                model.resolve_type(function.return_type().unwrap()),
                "{}",
                function.name().name(),
            );
        }
    });
}

#[test]
fn block_closures_bind_lexical_locals_and_join_reachable_ends() {
    with_model(
        concat!(
            "int? Function(bool) partial() => (bool flag) { if (flag) return 1; }; ",
            "int Function(bool) total() => (bool flag) { if (flag) return 1; throw 0; }; ",
            "int Function() looped() => () { while (true) { return 1; } }; ",
            "int Function(List<int>) iterated() => (List<int> values) { for (final value in values) { return value; } throw 0; }; ",
            "int Function() caught() => () { try { return 1; } on int catch (error) { return error; } }; ",
            "int Function((int, String)) destructured() => ((int, String) pair) { final (number, _) = pair; return number; }; ",
            "String Function(Object) cased() => (Object value) { if (value case String text) { return text; } throw 0; }; ",
            "int Function(Object) switched() => (Object value) { switch (value) { case int number: return number; default: throw 0; } }; ",
            "int Function() captured() => () { T identity<T>(T value) => value; final call = () => identity(1); return call(); };",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn closure_switches_join_null_only_for_reachable_ends() {
    with_model(
        concat!(
            "enum Choice { first, second } ",
            "class Other { static const first = 0; } ",
            "sealed class Closed {} class Child extends Closed {} ",
            "int Function(Choice) enumeration() => (Choice value) { switch (value) { case Choice.first: return 1; case Choice.second: return 2; } }; ",
            "int Function(bool) boolean() => (bool value) { switch (value) { case true: return 1; case false: return 2; } }; ",
            "int Function(bool?) nullableBoolean() => (bool? value) { switch (value) { case true: return 1; case false: return 2; case null: return 3; } }; ",
            "int Function(Choice?) nullableEnum() => (Choice? value) { switch (value) { case Choice.first: return 1; case Choice.second: return 2; case null: return 3; } }; ",
            "int Function(Object) wildcard() => (Object value) { switch (value) { case Object _: return 1; } }; ",
            "int Function(Object?) untypedWildcard() => (Object? value) { switch (value) { case _: return 1; } }; ",
            "int Function(Object?) wildcardAndNull() => (Object? value) { switch (value) { case Object _: return 1; case null: return 2; } }; ",
            "int Function(Closed) sealedCatchAll() => (Closed value) { switch (value) { case Closed(): return 1; } }; ",
            "int Function(List<int>) genericCatchAll() => (List<int> value) { switch (value) { case List<num> _: return 1; } }; ",
            "int Function(bool) lexical() => (bool value) { final local = value; { switch (local) { case true: return 1; case false: return 2; } } }; ",
            "int? Function(Choice) incompleteEnum() => (Choice value) { switch (value) { case Choice.first: return 1; } }; ",
            "int? Function(bool) incompleteBool() => (bool value) { switch (value) { case true: return 1; } }; ",
            "int? Function(bool?) missingNull() => (bool? value) { switch (value) { case true: return 1; case false: return 2; } }; ",
            "int? Function(bool) guarded() => (bool value) { switch (value) { case true when value: return 1; case false: return 2; } }; ",
            "int? Function(Object?) nullableWildcard() => (Object? value) { switch (value) { case Object _: return 1; } }; ",
            "int? Function(bool) broken() => (bool value) { switch (value) { case true: return 1; case false: break; } }; ",
            "int? Function(Choice) wrongOwner() => (Choice value) { switch (value) { case Other.first: return 1; case Choice.second: return 2; } }; ",
            "int? Function(List<num>) narrowGeneric() => (List<num> value) { switch (value) { case List<int> _: return 1; } }; ",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope,
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn interface_joins_take_the_deepest_unique_shared_superinterface() {
    with_model(
        concat!(
            "class Base<T> { T get value => throw 0; } ",
            "class A extends Base<A> {} class B extends Base<B> {} ",
            "class Box<T> {} class C extends Box<Box<C>> {} class D extends Box<Box<D>> {} ",
            "class Shape {} class Square extends Shape {} class Circle extends Shape {} ",
            "class I1 {} class I2 {} class X implements I1, I2 {} class Y implements I1, I2 {} ",
            "class S {} mixin M {} class J0 {} class J extends J0 {} class K extends S with M {} ",
            "class K1 extends K implements J {} class K2 extends K implements J {} ",
            "T pick<T>(T first, T second) { return first; } ",
            "Object generic() => pick(A(), B()); ",
            "Object Function(bool) conditional() => (bool flag) => flag ? A() : B(); ",
            "Object? Function(bool, B?) nullable() => (bool flag, B? other) => flag ? A() : other; ",
            "Object Function(bool) closure() => (bool flag) { if (flag) return A(); return B(); }; ",
            "Object Function(bool) nested() => (bool flag) => flag ? C() : Box<D>(); ",
            "Base<A> Function(bool) lifted() => (bool flag) => flag ? A() : Base<A>(); ",
            "Shape Function(bool) shared() => (bool flag) => flag ? Square() : Circle(); ",
            "Object Function(bool) ambiguous() => (bool flag) => flag ? X() : Y(); ",
            "K Function(bool) mixed() => (bool flag) => flag ? K1() : K2(); ",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    continue;
                };
                let expression = expression.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        expression,
                        model,
                        &signatures,
                        &scope,
                    ),
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
        },
    );
}

#[test]
fn expression_inference_regression_table() {
    let cases = [
        ("Box.named()", Some("Box")),
        ("box?.field", Some("int?")),
        ("box?.method()", Some("int?")),
        ("box?.fail()", Some("Null")),
        ("callback()", Some("int")),
        ("x.member", Some("dynamic")),
        ("x.member()", Some("dynamic")),
        ("x.member.nested", Some("dynamic")),
        ("x as int?", Some("int?")),
        ("x as int", Some("int")),
        ("x is int", Some("bool")),
        ("x is! int", Some("bool")),
        ("!flag", Some("bool")),
        ("-n", Some("num")),
        ("++n", Some("num")),
        ("--n", Some("num")),
        ("n++", Some("num")),
        ("n--", Some("num")),
        ("i + d", Some("double")),
        ("n + d", Some("double")),
        ("n * d", Some("double")),
        ("n % d", Some("double")),
        ("d + n", Some("double")),
        ("i + n", Some("num")),
        ("i + i", Some("int")),
        ("s + s", Some("String")),
        ("x ?? i", Some("dynamic")),
        ("i / i", Some("double")),
        ("d / n", Some("double")),
        ("n / i", Some("double")),
        ("d ~/ n", Some("int")),
        ("n ~/ i", Some("int")),
        ("i ~/ d", Some("int")),
        ("box / i", Some("Box")),
        ("box ~/ i", None),
        ("x / i", Some("dynamic")),
        ("i / x", Some("double")),
        ("x ~/ i", Some("dynamic")),
        ("i ~/ x", Some("int")),
        ("i + x", Some("num")),
        ("n + x", Some("num")),
        ("d + x", Some("double")),
        ("i & x", Some("int")),
        ("i << x", Some("int")),
        ("missing / i", None),
    ];
    let mut source = String::from(
        "class Box { Box.named(); int field = 0; int method() => 0; Never fail() => throw 0; Box operator /(num s) => this; }",
    );
    for (index, (expression, expected)) in cases.iter().enumerate() {
        source.push_str(&format!(
            "{} f{index}(Box box, dynamic x, int i, double d, num n, String s, bool flag, int Function() callback) => {expression};",
            expected.unwrap_or("dynamic"),
        ));
    }
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let scope = TypeParameterScope::default();
        for ((expression, expected), declaration) in
            cases.iter().zip(program.declarations().skip(1))
        {
            let TopLevelDecl::Function(function) = declaration else {
                panic!()
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!()
            };
            let body = body.expr();
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), model, &scope);
            let expected = expected.map_or(ResolvedType::UNKNOWN, |_| {
                model.resolve_type(function.return_type().unwrap())
            });
            assert_eq!(
                environment.infer_with_signatures(body, model, &signatures, &scope),
                expected,
                "{expression}",
            );
        }
    });
}

#[test]
fn named_extension_static_members_retain_declaration_facts_and_types() {
    with_model(
        "extension Tools on Object { static int value = 0; static int get count => 1; static int read() => value; } int field() => Tools.value; int getter() => Tools.count; int method() => Tools.read();",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let owner = model.resolve_extension("Tools").unwrap();
            for name in ["value", "count", "read"] {
                let facts = signatures.declared_member_facts(&owner, name);
                assert_eq!(facts.len(), 1);
                assert!(facts[0].is_static);
                assert_eq!(facts[0].declaring_type, owner);
            }
            for declaration in program.declarations().skip(1) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!()
                };
                let body = body.expr();
                assert_eq!(
                    TypeEnvironment::new().infer_with_signatures(
                        body,
                        model,
                        &signatures,
                        &TypeParameterScope::default(),
                    ),
                    model.core_type("int", false),
                );
            }
        },
    );
}

#[test]
fn never_returning_calls_are_acceptable_in_void_contexts() {
    with_model(
        "Never stop() => throw 0; void use() => stop();",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap() else {
                panic!()
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!()
            };
            let expression = expression.expr();
            let actual = TypeEnvironment::new().infer_with_signatures(
                expression,
                model,
                &signatures,
                &TypeParameterScope::default(),
            );
            assert_eq!(actual, ResolvedType::NEVER);
            assert_eq!(
                model.acceptable_for_void_context(&ResolvedType::VOID, &actual),
                Truth::Yes,
            );
        },
    );
}

#[test]
fn type_literals_and_contextual_tearoffs() {
    with_model(
        concat!(
            "class Box<T> { Box(T value); Box.named(T value); T method(T value) => value; ",
            "U convert<U>(U value) => value; static int staticMethod(int value) => value; ",
            "static Box<int> get empty => Box(0); } ",
            "enum Choice { first, second } T identity<T>(T value) => value; ",
            "Type simple() => int; Type bottom() => Never; Type generic() => Box<int>; ",
            "Symbol symbol() => #member; ",
            "int Function(int) explicitFunction() => identity<int>; ",
            "int Function(int) downwardFunction() => identity; ",
            "Box<int> Function(int) constructor() => Box<int>.new; ",
            "Box<int> Function(int) namedConstructor() => Box<int>.named; ",
            "int Function(int) method(Box<int> box) => box.method; ",
            "int Function(int) genericMethod(Box<int> box) => box.convert<int>; ",
            "int Function(int) staticMethod() => Box.staticMethod; ",
            "Choice choice() => .first; Box<int> getter() => .empty;",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            for declaration in program.declarations().skip(3) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(body)) = function.body() else {
                    panic!()
                };
                let body = body.expr();
                let expected = model.resolve_type(function.return_type().unwrap());
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let inferred = environment.infer_with_context(
                    body,
                    model,
                    &signatures,
                    &scope,
                    Some(&expected),
                    &|_| None,
                );
                assert_eq!(inferred, expected, "{body:?}");
            }
        },
    );
}

#[test]
fn core_never_resolves_as_bottom_but_project_never_does_not() {
    with_model(
        "Never stop() => throw 0; Never? nullable() => null;",
        |program, model| {
            for (index, declaration) in program.declarations().enumerate() {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let ty = model.resolve_type(function.return_type().unwrap());
                assert_eq!(
                    ty,
                    if index == 0 {
                        ResolvedType::NEVER
                    } else {
                        ResolvedType::NULL
                    }
                );
                assert_eq!(
                    model.acceptable_for_void_context(&ResolvedType::VOID, &ty),
                    Truth::Yes
                );
            }
        },
    );
    with_model(
        "class Never {} Never value() => Never();",
        |program, model| {
            let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap() else {
                panic!()
            };
            assert!(matches!(
                model.resolve_type(function.return_type().unwrap()),
                ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        identity: DeclarationIdentity::Library { .. },
                        ..
                    },
                }
            ));
        },
    );
}

#[test]
fn declaration_model_preserves_extension_types_and_parameter_owners() {
    with_model(
        "extension type Id(int value) {} Id f<T>() => Id(0);",
        |program, model| {
            let owner = model.resolve_value("f").unwrap();
            let declaration_model = model.for_declaration(&owner).unwrap();
            let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap() else {
                panic!()
            };
            assert!(matches!(
                declaration_model.resolve_type(function.return_type().unwrap()),
                ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        extension_type: true,
                        ..
                    },
                }
            ));
            let mut original = TypeParameterScope::default();
            original.push(function.type_params(), model);
            let mut declaration = TypeParameterScope::default();
            declaration.push(function.type_params(), &declaration_model);
            assert_eq!(original.lookup("T"), declaration.lookup("T"));
        },
    );
}

#[test]
fn imported_declaration_parameters_use_their_source_file() {
    let inputs = [
        (
            PathBuf::from("/project/lib/main.dart"),
            "import 'origin.dart'; void local<T>() {}",
        ),
        (
            PathBuf::from("/project/lib/origin.dart"),
            "void remote<T>() {}",
        ),
    ];
    let parsed: Vec<_> = inputs
        .iter()
        .map(|(_, source)| {
            let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
            let parsed = Arc::new(crate::ParsedFile {
                source: (*source).into(),
                tree,
            });
            assert!(errors.is_empty(), "{errors:?}");
            parsed
        })
        .collect();
    let sources: Vec<_> = inputs
        .iter()
        .zip(&parsed)
        .map(|((path, _), parsed)| IdentitySource {
            path,
            parsed,
            has_parse_errors: false,
        })
        .collect();
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let caller = SemanticModel::new(&inputs[0].0, &parsed[0], &identities, None);
    let origin = SemanticModel::new(&inputs[1].0, &parsed[1], &identities, None);
    let owner = caller.resolve_value("remote").unwrap();
    let imported = caller.for_declaration(&owner).unwrap();
    let TopLevelDecl::Function(function) = parsed[1].program().declarations().next().unwrap()
    else {
        panic!()
    };
    let mut direct_scope = TypeParameterScope::default();
    direct_scope.push(function.type_params(), &origin);
    let mut imported_scope = TypeParameterScope::default();
    imported_scope.push(function.type_params(), &imported);
    let mut caller_scope = TypeParameterScope::default();
    caller_scope.push(function.type_params(), &caller);
    assert_eq!(direct_scope.lookup("T"), imported_scope.lookup("T"));
    assert_ne!(caller_scope.lookup("T"), imported_scope.lookup("T"));
}

#[test]
fn resolves_canonical_generic_and_special_types() {
    with_model(
        "import 'dart:async'; class Box<T> {} void f(Box<String?> box, Future<void> future) {}",
        |program, model| {
            let TopLevelDecl::Function(function) = program.declarations().nth(1).unwrap() else {
                panic!()
            };
            let box_type = model.resolve_type(
                function
                    .params()
                    .unwrap()
                    .params()
                    .next()
                    .unwrap()
                    .param_type()
                    .unwrap(),
            );
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity,
                        arguments,
                        ..
                    },
            } = box_type
            else {
                panic!()
            };
            assert!(matches!(identity, DeclarationIdentity::Library { name, .. } if name == "Box"));
            assert!(arguments[0].nullable());

            let future = model.resolve_type(
                function
                    .params()
                    .unwrap()
                    .params()
                    .nth(1)
                    .unwrap()
                    .param_type()
                    .unwrap(),
            );
            assert!(model.void_context(&future));
        },
    );
}

#[test]
fn unrelatedness_is_conservative_and_numeric_aware() {
    with_model(
        "void f(int i, double d, String s, dynamic x) {}",
        |program, model| {
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!()
            };
            let types: Vec<_> = function
                .params()
                .unwrap()
                .params()
                .map(|param| model.resolve_type(param.param_type().unwrap()))
                .collect();
            assert_eq!(
                SignatureIndex::default().relation(
                    &types[0],
                    crate::TypeRelation::UnrelatedTo(&types[1]),
                    model
                ),
                Truth::No
            );
            assert_eq!(
                SignatureIndex::default().relation(
                    &types[0],
                    crate::TypeRelation::UnrelatedTo(&types[2]),
                    model
                ),
                Truth::Yes
            );
            assert_eq!(
                SignatureIndex::default().relation(
                    &types[0],
                    crate::TypeRelation::UnrelatedTo(&types[3]),
                    model
                ),
                Truth::Unknown
            );
        },
    );
}

#[test]
fn type_parameter_bounds_can_reference_the_complete_parameter_list() {
    with_model(
        "void compare<T extends Comparable<U>, U extends T>(T left, U right) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let signature = signatures
                .function(&model.resolve_value("compare").unwrap())
                .unwrap();
            let ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { bound, .. },
            } = &signature.positional[0]
            else {
                panic!()
            };
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            } = bound.as_ref()
            else {
                panic!()
            };
            assert!(matches!(arguments.first(), Some(ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { id, .. },
                }) if id.name.as_ref() == "U"));
        },
    );
}

#[test]
fn signature_shapes_preserve_required_names_and_unused_declared_binders() {
    with_model(
        "void accept<T extends num, U extends T>({required int z, int a = 0}) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let signature = signatures
                .function(&model.resolve_value("accept").unwrap())
                .unwrap();
            let shape = signature.shape.as_ref().unwrap();
            assert_eq!(shape.required_named.as_ref(), &["z".to_string()]);
            assert_eq!(shape.type_parameters.len(), 2);
            let ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { id, bound, .. },
            } = &shape.type_parameters[0]
            else {
                panic!("missing declared binder");
            };
            assert_eq!(bound.as_ref(), &model.core_type("num", false));
            assert!(matches!(
                &shape.type_parameters[1],
                ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { bound, .. },
                } if matches!(bound.as_ref(), ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { id: dependency, .. },
                } if dependency == id)
            ));
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        shape: Some(forwarded),
                        ..
                    },
            } = signature.function_type()
            else {
                panic!("missing function shape");
            };
            assert!(Arc::ptr_eq(shape, &forwarded));
            let mut local = signature.clone();
            local.call_parameters.clear();
            local.bind_local_type_parameters(&TypeParameterScope::default());
            assert_eq!(local.call_parameters, signature.call_parameters);
            let substitutions = FxHashMap::from_iter([(id.clone(), model.core_type("int", false))]);
            let instantiated = signature.substituted_shape(model, &substitutions).unwrap();
            assert_eq!(instantiated.required_named, shape.required_named);
            assert!(matches!(
                instantiated.type_parameters.as_ref(),
                [ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter { bound, .. },
                }] if bound.as_ref() == &model.core_type("int", false)
            ));
        },
    );
}

#[test]
fn super_formal_declared_defaults_override_inherited_defaults() {
    with_model(
        concat!(
            "class Base { Base([int count = 7]); } ",
            "class Inherited extends Base { Inherited([super.count]); } ",
            "class Overridden extends Base { Overridden([super.count = 9]); }",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = model.resolve_name("Base").unwrap();
            let inherited = signatures
                .constructor(&model.resolve_name("Inherited").unwrap(), "new")
                .unwrap();
            let ParameterDefault::Inherited { owner, default } = &inherited.defaults["count"]
            else {
                panic!()
            };
            assert_eq!(*owner, base);
            let ParameterDefault::Declared(inherited_default) = default.as_ref() else {
                panic!()
            };
            let TopLevelDecl::Class(base_class) = program.declarations().next().unwrap() else {
                panic!()
            };
            let ClassMember::Constructor(base_constructor) = base_class.members().next().unwrap()
            else {
                panic!()
            };
            let expected_base = base_constructor
                .params()
                .params()
                .next()
                .unwrap()
                .default_value()
                .unwrap();
            assert_eq!(inherited_default.id, expected_base.syntax().id());
            assert!(Arc::ptr_eq(
                &inherited_default.file,
                &model.node_ref(expected_base.syntax()).file,
            ));
            let overridden = signatures
                .constructor(&model.resolve_name("Overridden").unwrap(), "new")
                .unwrap();
            let ParameterDefault::Declared(default) = &overridden.defaults["count"] else {
                panic!()
            };
            let TopLevelDecl::Class(class) = program.declarations().last().unwrap() else {
                panic!()
            };
            let ClassMember::Constructor(constructor) = class.members().next().unwrap() else {
                panic!()
            };
            let expected = constructor
                .params()
                .params()
                .next()
                .unwrap()
                .default_value()
                .unwrap();
            assert_eq!(default.id, expected.syntax().id());
            assert!(Arc::ptr_eq(
                &default.file,
                &model.node_ref(expected.syntax()).file,
            ));
        },
    );
}

#[test]
fn declaration_contracts_preserve_names_kinds_defaults_and_optional_arity() {
    with_model(
        "class A { void m(covariant int first, [int second]) {} \
         void n({required int named, int optional = 1 + 1}) {} \
         void empty() {} int operator +(covariant int other) => other; \
         int get value => 0; set value(covariant int incoming) {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let identity = model.resolve_name("A").unwrap();
            let method = signatures.member(&identity, "m").unwrap();
            assert_eq!(method.optional_positional, 1);
            assert!(matches!(
                method.function_type(),
                ResolvedType {
                    kind: ResolvedTypeKind::Function {
                        optional_positional: 1,
                        ..
                    },
                }
            ));
            let facts = &signatures.declared_member_facts(&identity, "m")[0];
            assert_eq!(facts.positional_parameter_names, ["first", "second"]);
            let contracts = facts.parameter_contracts.as_ref().unwrap();
            assert_eq!(contracts[0].name, "first");
            assert_eq!(contracts[0].kind, ParamKind::Positional);
            assert!(contracts[0].is_covariant);
            assert_eq!(contracts[1].name, "second");
            assert_eq!(contracts[1].kind, ParamKind::OptionalPositional);
            assert!(contracts[1].default_value_code.is_none());
            let cloned_facts = facts.clone();
            assert!(Arc::ptr_eq(
                contracts,
                cloned_facts.parameter_contracts.as_ref().unwrap(),
            ));
            let facts = &signatures.declared_member_facts(&identity, "n")[0];
            assert!(facts.positional_parameter_names.is_empty());
            let contracts = facts.parameter_contracts.as_ref().unwrap();
            assert_eq!(contracts[0].name, "named");
            assert_eq!(contracts[0].kind, ParamKind::Named { required: true });
            assert_eq!(contracts[1].name, "optional");
            assert!(contracts[1].default_value_code.is_some());
            let empty = signatures.member(&identity, "empty").unwrap();
            assert!(empty.positional.is_empty());
            assert!(empty.named.is_empty());
            assert!(empty.defaults.is_empty());
            let facts = &signatures.declared_member_facts(&identity, "empty")[0];
            assert!(facts.positional_parameter_names.is_empty());
            assert!(facts.parameter_contracts.as_ref().unwrap().is_empty());
            let operator = signatures.member(&identity, "+").unwrap();
            assert_eq!(operator.positional, [model.core_type("int", false)]);
            assert_eq!(operator.optional_positional, 0);
            let facts = &signatures.declared_member_facts(&identity, "+")[0];
            assert_eq!(facts.kind, MemberKind::Operator);
            assert_eq!(facts.positional_parameter_names, ["other"]);
            let contracts = facts.parameter_contracts.as_ref().unwrap();
            assert_eq!(contracts.len(), 1);
            assert_eq!(contracts[0].name, "other");
            assert_eq!(contracts[0].kind, ParamKind::Positional);
            assert!(contracts[0].is_covariant);
            assert!(contracts[0].default_value_code.is_none());
            let accessors = &signatures.member_entry(&identity, "value").unwrap().facts;
            let getter = accessors
                .iter()
                .find(|facts| facts.kind == MemberKind::Getter)
                .unwrap();
            assert!(getter.parameter_contracts.as_ref().unwrap().is_empty());
            let setter = accessors
                .iter()
                .find(|facts| facts.kind == MemberKind::Setter)
                .unwrap();
            let contract = &setter.parameter_contracts.as_ref().unwrap()[0];
            assert_eq!(contract.name, "incoming");
            assert!(contract.is_covariant);
        },
    );
}

#[test]
fn signature_collection_preserves_default_slots_and_required_annotations() {
    with_model(
        "class A { void m(int first, [int second = 1, int third]) {} \
         void n({required int named, required int explicit = 4, \
         @required int legacy = 2, int optional}) {} \
         void p(int first, int second) {} \
         void r(int leading, int defaulted = 3, int trailing) {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let identity = model.resolve_name("A").unwrap();
            let positional = signatures.member(&identity, "m").unwrap();
            assert_eq!(
                positional.positional_parameter_names,
                ["", "second", "third"]
            );
            assert_eq!(positional.optional_positional, 2);
            assert_eq!(positional.positional.len(), 3);
            assert!(!positional.defaults.contains_key("first"));
            assert!(matches!(
                positional.defaults.get("second"),
                Some(ParameterDefault::Declared(_))
            ));
            assert!(matches!(
                positional.defaults.get("third"),
                Some(ParameterDefault::Null)
            ));
            assert!(positional.non_optional_defaults.is_empty());
            let facts = &signatures.declared_member_facts(&identity, "m")[0];
            assert_eq!(
                facts.positional_parameter_names,
                ["first", "second", "third"]
            );
            let contracts = facts.parameter_contracts.as_ref().unwrap();
            assert!(contracts[1].default_value_code.is_some());
            assert!(contracts[2].default_value_code.is_none());
            let named = signatures.member(&identity, "n").unwrap();
            assert!(named.positional_parameter_names.is_empty());
            assert!(named.positional.is_empty());
            assert_eq!(named.named.len(), 4);
            assert!(!named.defaults.contains_key("named"));
            assert!(matches!(
                named.defaults.get("explicit"),
                Some(ParameterDefault::Declared(_))
            ));
            assert!(matches!(
                named.defaults.get("legacy"),
                Some(ParameterDefault::Declared(_))
            ));
            assert!(matches!(
                named.defaults.get("optional"),
                Some(ParameterDefault::Null)
            ));
            assert_eq!(
                named.non_optional_defaults,
                HashSet::from(["explicit".to_string(), "legacy".to_string()])
            );
            let plain = signatures.member(&identity, "p").unwrap();
            assert_eq!(plain.positional.len(), 2);
            assert!(plain.positional_parameter_names.is_empty());
            let required_positional = signatures.member(&identity, "r").unwrap();
            assert_eq!(
                required_positional.positional_parameter_names,
                ["", "defaulted", ""]
            );
            assert_eq!(required_positional.optional_positional, 0);
            assert_eq!(required_positional.positional.len(), 3);
            assert!(matches!(
                required_positional.defaults.get("defaulted"),
                Some(ParameterDefault::Declared(_))
            ));
            assert!(!required_positional.defaults.contains_key("leading"));
            assert!(!required_positional.defaults.contains_key("trailing"));
            assert_eq!(
                required_positional.non_optional_defaults,
                HashSet::from(["defaulted".to_string()])
            );
            assert!(plain.defaults.is_empty());
        },
    );
}

#[test]
fn signatures_preserve_type_parameters_for_void_substitution() {
    with_model("void accept<T>(T value) {}", |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let signature = signatures
            .function(&model.resolve_value("accept").unwrap())
            .unwrap();
        assert!(contains_parameter(&signature.positional[0]));
        let substitutions =
            FxHashMap::from_iter([(signature.call_parameters[0].clone(), ResolvedType::VOID)]);
        let expected = model.substitute(&signature.positional[0], &substitutions);
        assert_eq!(expected, ResolvedType::VOID);
        assert_eq!(
            model.acceptable_for_void_context(&expected, &model.core_type("int", false)),
            Truth::No
        );
        assert_eq!(
            model.acceptable_for_void_context(&expected, &ResolvedType::NULL),
            Truth::Yes
        );
    });
}

#[test]
fn field_formal_signatures_include_fields_declared_after_constructors() {
    with_model(
        "class Value<T> { Value(this.value); final T value; T read() => value; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let identity = model.resolve_name("Value").unwrap();
            let constructor = signatures.constructor(&identity, "new").unwrap();
            let (parameters, field) = signatures.field(&identity, "value").unwrap();
            assert_eq!(&constructor.owner_parameters, parameters);
            assert_eq!(&constructor.positional[0], field);
            assert_eq!(
                &signatures.member(&identity, "read").unwrap().return_type,
                field
            );
            let facts = signatures.declaration(&identity).unwrap();
            assert_eq!(&facts.constructors[0].parameters[0].ty, field);
        },
    );
}

#[test]
fn synthetic_enum_values_have_static_list_types_with_parsed_core() {
    let source = "enum Choice { first, second } \
         enum Generic<T> { integer<int>(1), text<String>('a'); const Generic(this.value); final T value; } \
         enum Bounded<T extends num> { integer<int>(1), decimal<double>(1.5); const Bounded(this.value); final T value; } \
         List<Choice> choices() => Choice.values; \
         Choice selected(int? index) => Choice.values[index!]; \
         int position(Choice previous) => Choice.values.indexOf(previous); \
         List<Generic<dynamic>> generic() => Generic.values; \
         List<Bounded<num>> bounded() => Bounded.values;";
    with_parsed_core(
        source,
        "library dart.core; class Object {} abstract class num {} \
         abstract class int extends num {} abstract class double extends num {} \
         abstract class String {} abstract class Enum {} \
         abstract class List<E> { E operator [](int index); int indexOf(E value); }",
        |program, model, signatures| {
            let scope = TypeParameterScope::default();
            for declaration in program.declarations() {
                let TopLevelDecl::Function(function) = declaration else {
                    continue;
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                let inferred =
                    environment.infer_with_signatures(expression, model, signatures, &scope);
                assert_eq!(
                    inferred,
                    model.resolve_type(function.return_type().unwrap()),
                    "{}",
                    function.name().name(),
                );
            }
            let identity = model.resolve_name("Choice").unwrap();
            let facts = signatures.declared_member_facts(&identity, "values");
            assert!(facts.iter().any(|fact| {
                fact.is_static
                    && matches!(
                        fact.kind,
                        MemberKind::Field {
                            writable: false,
                            ..
                        }
                    )
            }));
            let receiver = ResolvedType::interface_identity(identity);
            assert!(
                signatures
                    .resolved_field(&receiver, "values", model)
                    .is_none()
            );
        },
    );
}

#[test]
fn inference_lookup_fallback_reaches_recursive_expressions_and_respects_bindings() {
    with_model(
        "Object direct() => value; Object asserted() => value!; Object binary() => value + value; Object conditional() => true ? value : value; Object method() => value.toUpperCase(); Object closure() => () => value;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let scope = TypeParameterScope::default();
            let environment = TypeEnvironment::new();
            let expected = model.core_type("String", false);
            let lookup = |name: &str| (name == "value").then(|| expected.clone());
            for (index, declaration) in program.declarations().enumerate() {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let inferred = environment.infer_with_signatures_using_lookup(
                    expression,
                    model,
                    &signatures,
                    &scope,
                    &lookup,
                );
                if index == 5 {
                    let ResolvedType {
                        kind: ResolvedTypeKind::Function { return_type, .. },
                    } = inferred
                    else {
                        panic!()
                    };
                    assert_eq!(*return_type, expected);
                } else {
                    assert_eq!(inferred, expected);
                }
                if index == 0 {
                    let mut bound = environment.clone();
                    bound.declare("value", model.core_type("int", false));
                    assert_eq!(
                        bound.infer_with_signatures_using_lookup(
                            expression,
                            model,
                            &signatures,
                            &scope,
                            &|_| Some(ResolvedType::UNKNOWN),
                        ),
                        model.core_type("int", false),
                    );
                    assert!(
                        bound
                            .infer_with_signatures_using_lookup(
                                expression,
                                model,
                                &signatures,
                                &scope,
                                &|_| None,
                            )
                            .interface("dart:core", "int")
                    );
                }
            }
        },
    );
}

#[test]
fn cast_inference_preserves_written_nullability_and_type_parameter_scope() {
    with_model(
        "int? nullable(Object? value) => value as int?; int nonNullable(Object value) => value as int; T? generic<T>(Object? value) => value as T?;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (index, declaration) in program.declarations().enumerate() {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let mut scope = TypeParameterScope::default();
                scope.push(function.type_params(), model);
                let expected = model.resolve_type_in(function.return_type().unwrap(), &scope);
                let environment = TypeEnvironment::new();
                assert_eq!(
                    environment.infer_with_signatures_using_lookup(
                        expression,
                        model,
                        &signatures,
                        &scope,
                        &|_| None
                    ),
                    expected
                );
                assert_eq!(
                    environment.infer_with_signatures(expression, model, &signatures, &scope),
                    expected,
                );
                assert_eq!(expected.nullable(), index != 1);
                if index == 2 {
                    assert!(matches!(
                        expected,
                        ResolvedType {
                            kind: ResolvedTypeKind::TypeParameter { .. },
                        }
                    ));
                }
            }
        },
    );
}

#[test]
fn inherited_method_calls_use_instantiated_return_types() {
    with_model(
        "class Base { int value() => 0; } class Child extends Base {} int inherited(Child child) => child.value(); class GenericBase<T> { T value() => throw UnimplementedError(); } class GenericChild extends GenericBase<int> {} int generic(GenericChild child) => child.value();",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in [
                program.declarations().nth(2).unwrap(),
                program.declarations().nth(5).unwrap(),
            ] {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let type_parameters = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &type_parameters);

                let inferred = environment.infer_with_signatures(
                    expression,
                    model,
                    &signatures,
                    &type_parameters,
                );
                assert!(inferred.interface("dart:core", "int"));
            }
        },
    );
}

#[test]
fn same_named_type_parameters_of_different_declarations_stay_distinct() {
    with_model(
        "class B<T> { T get x => throw 0; } class A<T> extends B<int> {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let a = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("A").unwrap(),
                    arguments: vec![model.core_type("String", false)].into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            let (field, substitutions) = signatures.resolved_field(&a, "x", model).unwrap();
            assert!(
                model
                    .substitute(&field, &substitutions)
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn member_lookup_prefers_later_mixins_and_includes_interface_contracts() {
    with_model(
        "class Base { num value() => 0; num field = 0; } mixin First<T> { T value() => throw UnimplementedError(); T field; } mixin Second<T> { T value() => throw UnimplementedError(); T field; } abstract class Contract { bool value(); bool field; int only(); } class Child extends Base with First<String>, Second<int> implements Contract {} class InterfaceOnly implements Contract {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let child_identity = model.resolve_name("Child").unwrap();
            let child = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: child_identity,
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            let (signature, substitutions) =
                signatures.resolved_member(&child, "value", model).unwrap();
            assert!(
                model
                    .substitute(&signature.return_type, &substitutions)
                    .interface("dart:core", "int")
            );
            let (field, substitutions) = signatures.resolved_field(&child, "field", model).unwrap();
            assert!(
                model
                    .substitute(&field, &substitutions)
                    .interface("dart:core", "int")
            );
            assert_eq!(
                signatures.resolved_member_owner(&child, "value", model),
                model.resolve_name("Second")
            );

            let contract = model.resolve_name("Contract").unwrap();
            let interface_only = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("InterfaceOnly").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            assert_eq!(
                signatures.relation(
                    &interface_only,
                    crate::TypeRelation::SubtypeOfDeclaration(&contract),
                    model
                ),
                Truth::Yes
            );
            let (signature, substitutions) = signatures
                .resolved_member(&interface_only, "only", model)
                .unwrap();
            assert!(
                model
                    .substitute(&signature.return_type, &substitutions)
                    .interface("dart:core", "int")
            );
            assert_eq!(
                signatures.resolved_member_owner(&interface_only, "only", model),
                Some(contract)
            );
        },
    );
}

#[test]
fn static_mixin_members_do_not_shadow_inherited_instance_members() {
    with_model(
        "class Base { int method() => 0; int field = 0; int get getter => 0; set setter(int value) {} } mixin StaticMembers { static String method() => ''; static bool field = false; static double get getter => 0; static set setter(num value) {} } class Child extends Base with StaticMembers {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let child = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            let base = model.resolve_name("Base").unwrap();
            let mixin = model.resolve_name("StaticMembers").unwrap();

            let (signature, substitutions) =
                signatures.resolved_member(&child, "method", model).unwrap();
            assert!(
                model
                    .substitute(&signature.return_type, &substitutions)
                    .interface("dart:core", "int")
            );
            for name in ["method", "field", "getter"] {
                assert_eq!(
                    signatures.resolved_member_owner(&child, name, model),
                    Some(base.clone())
                );
            }

            for name in ["field", "getter"] {
                let (field, substitutions) =
                    signatures.resolved_field(&child, name, model).unwrap();
                assert!(
                    model
                        .substitute(&field, &substitutions)
                        .interface("dart:core", "int")
                );
            }
            let (setter, substitutions) = signatures
                .resolved_writable_field(&child, "setter", model)
                .unwrap();
            assert!(
                model
                    .substitute(&setter, &substitutions)
                    .interface("dart:core", "int")
            );

            assert!(
                signatures
                    .member(&mixin, "method")
                    .unwrap()
                    .return_type
                    .interface("dart:core", "String")
            );
            for (name, expected) in [("field", "bool"), ("getter", "double"), ("setter", "num")] {
                assert!(
                    signatures
                        .field(&mixin, name)
                        .unwrap()
                        .1
                        .interface("dart:core", expected)
                );
            }
        },
    );
}

#[test]
fn legacy_function_typed_setter_has_a_nullable_function_write_type() {
    with_model(
        "class Value { set callback(void value()?) {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Value").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            let (write_type, substitutions) = signatures
                .resolved_writable_field(&receiver, "callback", model)
                .unwrap();
            let write_type = model.substitute(&write_type, &substitutions);
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type,
                        positional,
                        named,
                        nullable,
                        ..
                    },
            } = write_type
            else {
                panic!("setter parameter must remain a function type");
            };
            assert_eq!(*return_type, ResolvedType::VOID);
            assert!(positional.is_empty());
            assert!(named.is_empty());
            assert!(nullable);
        },
    );
}

#[test]
fn setter_does_not_overwrite_getter_return_type_index() {
    for source in [
        "class Value { String get value => ''; set value(num next) {} }",
        "class Value { set value(num next) {} String get value => ''; }",
    ] {
        with_model(source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let identity = model.resolve_name("Value").unwrap();
            assert!(
                signatures
                    .field(&identity, "value")
                    .unwrap()
                    .1
                    .interface("dart:core", "String")
            );
        });
    }
}

#[test]
fn only_uninitialized_late_final_fields_have_writable_owners() {
    with_model(
        "class Value {} class Base { late final Value pending, initialized = Value(); final Value ordinary = Value(); } class Child extends Base {} abstract class View implements Base {} mixin Init on Base {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = model.resolve_name("Base").unwrap();
            for owner in ["Child", "View", "Init"] {
                let child = ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name(owner).unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    },
                };
                assert_eq!(
                    signatures.resolved_writable_member_owner(&child, "pending", model),
                    Some(base.clone())
                );
                let (ty, substitutions) = signatures
                    .resolved_writable_field(&child, "pending", model)
                    .unwrap();
                let ResolvedType {
                    kind: ResolvedTypeKind::Interface { identity, .. },
                } = model.substitute(&ty, &substitutions)
                else {
                    panic!("late-final setter should retain its declared field type");
                };
                assert_eq!(Some(identity), model.resolve_name("Value"));
                for name in ["initialized", "ordinary"] {
                    assert_eq!(
                        signatures.resolved_readable_member_owner(&child, name, model),
                        Some(base.clone())
                    );
                    assert!(
                        signatures
                            .resolved_writable_member_owner(&child, name, model)
                            .is_none()
                    );
                    assert!(
                        signatures
                            .resolved_writable_field(&child, name, model)
                            .is_none()
                    );
                }
            }
        },
    );
}

#[test]
fn child_setter_does_not_hide_inherited_generic_getter() {
    with_model(
        "class Base<T> { T get value => throw UnimplementedError(); } class Child extends Base<String> { set value(num next) {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let child = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };

            let (read_type, read_substitutions) =
                signatures.resolved_field(&child, "value", model).unwrap();
            assert!(contains_parameter(&read_type));
            assert!(
                model
                    .substitute(&read_type, &read_substitutions)
                    .interface("dart:core", "String")
            );

            let (write_type, write_substitutions) = signatures
                .resolved_writable_field(&child, "value", model)
                .unwrap();
            assert!(
                model
                    .substitute(&write_type, &write_substitutions)
                    .interface("dart:core", "num")
            );
        },
    );
}

#[test]
fn named_constructor_and_instance_method_signatures_do_not_collide() {
    with_model(
        "class Base { Base.named(String input); int named(bool input) => 0; } class Child extends Base { Child(super.input) : super.named(); }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = model.resolve_name("Base").unwrap();
            let receiver = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: base.clone(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };

            let constructor = signatures.constructor(&base, "named").unwrap();
            assert!(constructor.positional[0].interface("dart:core", "String"));

            let (method, substitutions) = signatures
                .resolved_member(&receiver, "named", model)
                .unwrap();
            assert!(method.positional[0].interface("dart:core", "bool"));
            assert!(
                model
                    .substitute(&method.return_type, &substitutions)
                    .interface("dart:core", "int")
            );

            let child = model.resolve_name("Child").unwrap();
            assert!(
                signatures.constructor(&child, "new").unwrap().positional[0]
                    .interface("dart:core", "String")
            );
        },
    );
}

#[test]
fn mixin_constraints_do_not_override_applied_generic_superclass_fields() {
    with_model(
        "class Widget {} class State<T> { T get widget => throw 0; } mixin Constraint on State {} class Concrete extends State<Widget> with Constraint {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver =
                ResolvedType::interface_identity(model.resolve_name("Concrete").unwrap());
            let (field, substitutions) = signatures
                .resolved_field(&receiver, "widget", model)
                .unwrap();
            assert_eq!(
                model.substitute(&field, &substitutions),
                ResolvedType::interface_identity(model.resolve_name("Widget").unwrap()),
            );
            let constraint = model.resolve_name("Constraint").unwrap();
            assert_eq!(
                signatures.type_facts[&constraint]
                    .member_supertypes
                    .as_ref()
                    .unwrap(),
                &vec![ResolvedType::interface_identity(
                    DeclarationIdentity::OBJECT
                )],
            );
        },
    );
}

#[test]
fn later_mixin_getter_overrides_base_field_with_owner_substitution() {
    with_model(
        "class Base { num value = 0; } mixin ReadValue<T> { T get value => throw UnimplementedError(); } class Child extends Base with ReadValue<int> {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let child = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };

            let (field, substitutions) = signatures.resolved_field(&child, "value", model).unwrap();
            assert!(contains_parameter(&field));
            assert!(
                model
                    .substitute(&field, &substitutions)
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn later_mixin_getter_shadows_base_method_for_all_instance_lookup() {
    with_model(
        "class Base { num value() => 0; } mixin ReadValue<T> { T get value => throw UnimplementedError(); } class Child extends Base with ReadValue<String> {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let child = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };

            assert!(signatures.resolved_member(&child, "value", model).is_none());
            let (read, substitutions) = signatures.resolved_field(&child, "value", model).unwrap();
            assert!(contains_parameter(&read));
            assert!(
                model
                    .substitute(&read, &substitutions)
                    .interface("dart:core", "String")
            );
        },
    );
}

#[test]
fn later_mixin_method_shadows_base_read_for_all_instance_lookup() {
    with_model(
        "class Base { num get value => 0; } mixin ComputeValue<T> { T value() => throw UnimplementedError(); } class Child extends Base with ComputeValue<int> {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let child = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Child").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };

            assert!(signatures.resolved_field(&child, "value", model).is_none());
            let (signature, substitutions) =
                signatures.resolved_member(&child, "value", model).unwrap();
            assert!(contains_parameter(&signature.return_type));
            assert!(
                model
                    .substitute(&signature.return_type, &substitutions)
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn implemented_member_combination_is_order_independent_and_instantiated() {
    with_model(
        "class Result {} class Narrow extends Result {} \
         abstract class A<T> { T get value; T method(); set output(Narrow value); } \
         abstract class B<T> { T get value; T method(); set output(Result value); } \
         abstract class Forward implements A<Result>, B<Narrow> {} \
         abstract class Reverse implements B<Narrow>, A<Result> {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let narrow = ResolvedType::interface_identity(model.resolve_name("Narrow").unwrap());
            let wide = ResolvedType::interface_identity(model.resolve_name("Result").unwrap());
            for name in ["Forward", "Reverse"] {
                let receiver = ResolvedType::interface_identity(model.resolve_name(name).unwrap());
                let (field, substitutions) = signatures
                    .resolved_field(&receiver, "value", model)
                    .unwrap();
                assert_eq!(model.substitute(&field, &substitutions), narrow);
                let (method, substitutions) = signatures
                    .resolved_member(&receiver, "method", model)
                    .unwrap();
                assert_eq!(
                    model.substitute(&method.return_type, &substitutions),
                    narrow
                );
                let (field, substitutions) = signatures
                    .resolved_writable_field(&receiver, "output", model)
                    .unwrap();
                assert_eq!(model.substitute(&field, &substitutions), wide);
            }
        },
    );
}

#[test]
fn incompatible_implemented_members_remain_unknown() {
    with_model(
        "class Left {} class Right {} \
         abstract class A { Left get value; Left method(); } \
         abstract class B { Right get value; Right method(); } \
         abstract class Forward implements A, B {} \
         abstract class Reverse implements B, A {} \
         extension Fallback on Object { Left get value => Left(); Left method() => Left(); }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for name in ["Forward", "Reverse"] {
                let receiver = ResolvedType::interface_identity(model.resolve_name(name).unwrap());
                assert!(
                    signatures
                        .resolved_field(&receiver, "value", model)
                        .is_none()
                );
                assert!(
                    signatures
                        .resolved_member(&receiver, "method", model)
                        .is_none()
                );
            }
        },
    );
}

#[test]
fn negative_supertype_queries_visit_each_diamond_declaration_once() {
    let mut source = String::from("class Root<T> {} class Missing {} ");
    for level in 0..24 {
        let base = if level == 0 {
            "Root".to_owned()
        } else {
            format!("Join{}", level - 1)
        };
        source.push_str(&format!(
            "abstract class Left{level}<T> implements {base}<T> {{}} \
             abstract class Right{level}<T> implements {base}<T> {{}} \
             abstract class Join{level}<T> implements Left{level}<T>, Right{level}<T> {{}} "
        ));
    }
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let receiver = ResolvedType {
            kind: ResolvedTypeKind::Interface {
                occurrence: Default::default(),
                identity: model.resolve_name("Join23").unwrap(),
                arguments: vec![model.core_type("int", false)].into(),
                nullable: false,
                extension_type: false,
            },
        };
        let visits = std::cell::Cell::new(0);
        let mut visited = FxHashSet::default();
        assert!(
            signatures
                .instantiated_matching(
                    &receiver,
                    &|_| {
                        visits.set(visits.get() + 1);
                        false
                    },
                    model,
                    &mut visited,
                )
                .is_none()
        );
        assert_eq!(visits.get(), visited.len());
        assert_eq!(visited.len(), 74); // 24 diamonds, Root and Object.
        assert!(
            signatures
                .instantiated_identity(&receiver, &model.resolve_name("Missing").unwrap(), model,)
                .is_none()
        );
        assert!(
            signatures
                .instantiated_supertype(&receiver, "dart:async", "Future", model)
                .is_none()
        );
        let root = signatures
            .instantiated_identity(&receiver, &model.resolve_name("Root").unwrap(), model)
            .unwrap();
        assert_eq!(root.arguments(), &[model.core_type("int", false)]);
    });
}

#[test]
fn signature_index_retains_caller_arenas_independently_of_identity_sources() {
    let source = "void local({int count = 7}) {} const value = 9;";
    let parse_file = || {
        let result = parse(source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        Arc::new(crate::ParsedFile {
            source: source.into(),
            tree: result.tree,
        })
    };
    let caller = parse_file();
    let indexed = parse_file();
    let path = PathBuf::from("/project/lib/main.dart");
    let sources = [IdentitySource {
        path: &path,
        parsed: &indexed,
        has_parse_errors: false,
    }];
    let types = TypeIndex::from_program(caller.program());
    let files = [(path.clone(), caller.clone())];
    let empty = IdentityIndex::from_project_files(&[], &[]);
    let empty_signatures = SignatureIndex::build(&files, None, &empty, &types);
    assert!(empty_signatures.functions.is_empty());
    assert!(empty_signatures.constants.is_empty());

    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let signatures = SignatureIndex::build(&files, None, &identities, &types);
    let model = SemanticModel::new(&path, &caller, &identities, Some(&types));
    let signature = signatures
        .functions
        .get(&model.resolve_value("local").unwrap())
        .unwrap();
    let ParameterDefault::Declared(default) = &signature.defaults["count"] else {
        panic!()
    };
    let constant = signatures
        .constants
        .get(&model.resolve_value("value").unwrap())
        .unwrap();
    let mut declarations = caller.program().declarations();
    let TopLevelDecl::Function(function) = declarations.next().unwrap() else {
        panic!()
    };
    let TopLevelDecl::Variable(variable) = declarations.next().unwrap() else {
        panic!()
    };
    let expected_default = function
        .params()
        .unwrap()
        .params()
        .next()
        .unwrap()
        .default_value()
        .unwrap();
    let expected_constant = variable
        .declarators()
        .next()
        .unwrap()
        .initializer()
        .unwrap();
    for (retained, expected) in [(default, expected_default), (constant, expected_constant)] {
        assert!(Arc::ptr_eq(&retained.file, &caller));
        assert!(!Arc::ptr_eq(&retained.file, &indexed));
        assert_eq!(retained.id, expected.syntax().id());
        assert_eq!(retained.expr().syntax().id(), expected.syntax().id());
    }
}

#[test]
fn local_callable_retains_defaults_without_a_file_identity() {
    let source = "void local({int count = 7}) {}";
    let parsed_result = parse(source);
    assert!(
        parsed_result.errors.is_empty(),
        "{:?}",
        parsed_result.errors
    );
    let parsed = Arc::new(crate::ParsedFile {
        source: source.into(),
        tree: parsed_result.tree,
    });
    let identities = IdentityIndex::from_project_files(&[], &[]);
    let path = PathBuf::from("/project/lib/main.dart");
    let model = SemanticModel::new(&path, &parsed, &identities, None);
    assert!(model.file.is_none());
    let TopLevelDecl::Function(function) = parsed.program().declarations().next().unwrap() else {
        panic!()
    };
    let params = function.params().unwrap();
    let expected = params.params().next().unwrap().default_value().unwrap();
    let signature = ResolvedSignature::local_callable(
        &TypeParameterScope::default(),
        function.type_params(),
        params,
        &model,
    );
    let ParameterDefault::Declared(default) = &signature.defaults["count"] else {
        panic!()
    };
    assert!(Arc::ptr_eq(&default.file, &parsed));
    assert_eq!(default.id, expected.syntax().id());
    assert_eq!(default.expr().syntax().id(), expected.syntax().id());
}

#[test]
fn resolved_calls_borrow_signatures_and_share_default_expressions() {
    with_model(
        "class Value { Value({int n = 1}); void method({int n = 1}) {} } \
         class Receiver {} extension Methods on Receiver { void method({int n = 1}) {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let owner = model.resolve_name("Value").unwrap();
            let receiver = ResolvedType::interface_identity(owner.clone());
            let (constructor, _) = signatures.resolved_constructor(&receiver, "new").unwrap();
            assert!(std::ptr::eq(
                constructor,
                signatures.constructor(&owner, "new").unwrap()
            ));
            let (method, _) = signatures
                .resolved_member(&receiver, "method", model)
                .unwrap();
            assert!(std::ptr::eq(
                method,
                signatures.instance_member(&owner, "method").unwrap()
            ));
            let cloned = (*method).clone();
            let ParameterDefault::Declared(default) = &method.defaults["n"] else {
                panic!()
            };
            let ParameterDefault::Declared(copy) = &cloned.defaults["n"] else {
                panic!()
            };
            assert!(Arc::ptr_eq(&default.file, &copy.file));
            assert_eq!(default.id, copy.id);
            let receiver =
                ResolvedType::interface_identity(model.resolve_name("Receiver").unwrap());
            let (resolved, _) = signatures
                .resolved_member(&receiver, "method", model)
                .unwrap();
            let (_, _, declared) = signatures
                .extension_method(&receiver, "method", model)
                .unwrap();
            assert!(std::ptr::eq(resolved, declared));
        },
    );
}

#[test]
fn object_fallback_does_not_beat_earlier_application_chain_members() {
    with_model(
        "class Base { String toString() => ''; } mixin First { String toString() => ''; } mixin Last {} class FromBase extends Base with Last {} class FromMixin extends Base with First, Last {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (class_name, owner_name) in [("FromBase", "Base"), ("FromMixin", "First")] {
                let receiver = ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name(class_name).unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    },
                };
                assert_eq!(
                    signatures.resolved_member_owner(&receiver, "toString", model),
                    model.resolve_name(owner_name)
                );
            }
        },
    );
}

#[test]
fn super_owner_checks_mixin_constraints_before_object_and_superclasses() {
    with_model(
        "class Base { String toString() => ''; void member() {} } mixin Applied on Base { void member() {} } class Concrete extends Base with Applied {} mixin Constrained on Concrete {} class Child extends Base with Applied { void member() {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (receiver, member, owner) in [
                ("Constrained", "toString", "Base"),
                ("Constrained", "member", "Applied"),
                ("Child", "toString", "Base"),
                ("Child", "member", "Applied"),
            ] {
                let receiver = ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name(receiver).unwrap(),
                        arguments: Vec::new().into(),
                        nullable: false,
                        extension_type: false,
                    },
                };
                assert_eq!(
                    signatures.resolved_super_member_owner(&receiver, member, false, model),
                    model.resolve_name(owner)
                );
            }
        },
    );
}

#[test]
fn extension_owner_checks_concrete_arguments_and_preserves_unknowns() {
    with_model(
        "class IntElement {} class StringElement {} class SourceList<T> {} class IntList extends SourceList<IntElement> {} extension Strings on SourceList<StringElement> { void member() {} } extension General on Object { void member() {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for (element, owner) in [("IntElement", "General"), ("StringElement", "Strings")] {
                let receiver = ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: model.resolve_name("SourceList").unwrap(),
                        arguments: vec![ResolvedType {
                            kind: ResolvedTypeKind::Interface {
                                occurrence: Default::default(),
                                identity: model.resolve_name(element).unwrap(),
                                arguments: Vec::new().into(),
                                nullable: false,
                                extension_type: false,
                            },
                        }]
                        .into(),
                        nullable: false,
                        extension_type: false,
                    },
                };
                assert_eq!(
                    signatures.resolved_extension_owner(&receiver, "member", model),
                    model.resolve_extension(owner)
                );
            }
            let receiver = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("IntList").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            assert_eq!(
                signatures.resolved_extension_owner(&receiver, "member", model),
                model.resolve_extension("General")
            );
            let unknown = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("SourceList").unwrap(),
                    arguments: vec![ResolvedType::UNKNOWN].into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            assert_eq!(
                signatures.resolved_extension_owner(&unknown, "member", model),
                None
            );
        },
    );
}

#[test]
fn extension_owner_instantiates_raw_generic_and_bounded_on_types() {
    with_model(
        concat!(
            "class Element {} class NumberElement extends Element {} class TextElement extends Element {} ",
            "class SourceIterable<T> {} class SourceList<T> extends SourceIterable<T> {} ",
            "class SourceMap<K, V> {} class NestedList extends SourceList<SourceList<NumberElement>> {} ",
            "extension RawList on SourceList { void raw() {} } ",
            "extension RawIterable on SourceIterable { void raw() {} } ",
            "extension GenericList<T> on SourceList<T> { void generic() {} void concrete() {} T element() => throw 0; } ",
            "extension GenericIterable<U> on SourceIterable<U> { void generic() {} } ",
            "extension ConcreteIterable on SourceIterable<NumberElement> { void concrete() {} } ",
            "extension TextMap<K> on SourceMap<K, TextElement> { void mapping() {} } ",
            "extension BoundedList<T extends NumberElement> on SourceList<T> { void bounded() {} } ",
            "extension Nested<T extends NumberElement> on SourceList<SourceList<T>> { T nested() => throw 0; } ",
            "extension General on Object { void mapping() {} void bounded() {} } ",
            "void uses(SourceList<NumberElement> numbers, SourceList<TextElement> texts, ",
            "SourceList raw, SourceMap<NumberElement, NumberElement> numberMap, ",
            "SourceMap<NumberElement, TextElement> textMap, NestedList nested) {}",
        ),
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().last().unwrap() else {
                panic!()
            };
            let receivers: Vec<_> = function
                .params()
                .unwrap()
                .params()
                .map(|parameter| model.resolve_type(parameter.param_type().unwrap()))
                .collect();
            for (index, member, owner) in [
                (0, "raw", "RawList"),
                (2, "raw", "RawList"),
                (0, "generic", "GenericList"),
                (0, "concrete", "GenericList"),
                (0, "bounded", "BoundedList"),
                (1, "bounded", "General"),
                (3, "mapping", "General"),
                (4, "mapping", "TextMap"),
                (5, "nested", "Nested"),
            ] {
                assert_eq!(
                    signatures.resolved_extension_owner(&receivers[index], member, model),
                    model.resolve_extension(owner),
                    "{index}: {member}",
                );
            }
            for (index, method) in [(0, "element"), (5, "nested")] {
                let (signature, substitutions) = signatures
                    .resolved_member(&receivers[index], method, model)
                    .unwrap();
                assert_eq!(
                    model.substitute(&signature.return_type, &substitutions),
                    ResolvedType::interface_identity(model.resolve_name("NumberElement").unwrap()),
                );
            }
        },
    );
}

#[test]
fn generic_extension_getters_apply_to_void_type_arguments() {
    with_model(
        "class SourceFuture<T> {} extension Completion<T> on SourceFuture<T> { void get completed {} } void uses(SourceFuture<void> future) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().last().unwrap() else {
                panic!()
            };
            let receiver = model.resolve_type(
                function
                    .params()
                    .unwrap()
                    .params()
                    .next()
                    .unwrap()
                    .param_type()
                    .unwrap(),
            );
            assert!(signatures.has_possible_extension_getter(&receiver, "completed", model));
            assert_eq!(
                signatures.resolved_extension_owner(&receiver, "completed", model),
                model.resolve_extension("Completion"),
            );
        },
    );
}

#[test]
fn extension_specificity_breaks_instantiated_ties_at_parameter_bounds() {
    with_model(
        "class Element {} class SourceList<T> {} extension Exact on SourceList<Element> { void choose() {} } extension Generic<T> on SourceList<T> { void choose() {} } void uses(SourceList<Element> values) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver = &signatures
                .function(&model.resolve_value("uses").unwrap())
                .unwrap()
                .positional[0];
            assert_eq!(
                signatures.resolved_extension_owner(receiver, "choose", model),
                model.resolve_extension("Exact"),
            );
        },
    );
}

#[test]
fn extension_inference_looks_through_receiver_bounds_and_retains_missing_arguments() {
    with_model(
        "class Element {} class SourceList<T> {} class Holder {} extension Pick<T> on SourceList<T> { T pick() => throw 0; } extension Unused<T extends Element> on Holder { T unused() => throw 0; } void uses<X extends SourceList<Element>>(X values, Holder holder) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let signature = signatures
                .function(&model.resolve_value("uses").unwrap())
                .unwrap();
            for (receiver, member) in [
                (&signature.positional[0], "pick"),
                (&signature.positional[1], "unused"),
            ] {
                let (method, substitutions) =
                    signatures.resolved_member(receiver, member, model).unwrap();
                assert_eq!(
                    model.substitute(&method.return_type, &substitutions),
                    ResolvedType::interface_identity(model.resolve_name("Element").unwrap()),
                );
            }
            let (method, substitutions) = signatures
                .resolved_extension_override_method(
                    &model.resolve_extension("Pick").unwrap(),
                    &ResolvedType::UNKNOWN,
                    &[],
                    "pick",
                    model,
                )
                .unwrap();
            assert_eq!(
                model.substitute(&method.return_type, &substitutions),
                ResolvedType::UNKNOWN,
            );
        },
    );
}

#[test]
fn extension_applicability_proves_only_universal_missing_arguments() {
    with_model(
        "class Element {} class SourceFuture<T> {} extension Completion<T> on SourceFuture<T> { void get completed {} T element() => throw 0; } extension Fixed on SourceFuture<Element> { void fixed() {} } extension Bounded<T extends Element> on SourceFuture<T> { void bounded() {} } extension FunctionOwner<T> on T { void invoke() {} } void uses(SourceFuture<Element> future, void Function() callback) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let signature = signatures
                .function(&model.resolve_value("uses").unwrap())
                .unwrap();
            let mut receiver = signature.positional[0].clone();
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            } = &mut receiver
            else {
                panic!()
            };
            Arc::make_mut(arguments)[0] = ResolvedType::UNKNOWN;
            for (name, expected) in [
                ("Completion", Truth::Yes),
                ("Fixed", Truth::Unknown),
                ("Bounded", Truth::Unknown),
            ] {
                let extension = signatures
                    .extensions
                    .iter()
                    .find(|extension| extension.name.as_deref() == Some(name))
                    .unwrap();
                assert_eq!(
                    signatures
                        .instantiated_extension_on_type(&receiver, extension, model)
                        .1,
                    expected,
                    "{name}",
                );
            }
            assert!(signatures.has_possible_extension_getter(&receiver, "completed", model));
            assert!(signatures.has_possible_extension_getter(
                &signature.positional[0].with_nullable(true),
                "completed",
                model,
            ));
            let (method, substitutions) = signatures
                .resolved_member(&receiver, "element", model)
                .unwrap();
            assert_eq!(
                model.substitute(&method.return_type, &substitutions),
                ResolvedType::UNKNOWN
            );
            assert_eq!(
                signatures.resolved_extension_owner(&signature.positional[1], "invoke", model),
                model.resolve_extension("FunctionOwner"),
            );
            assert_eq!(
                signatures.relation(
                    &ResolvedType::UNKNOWN,
                    crate::TypeRelation::SubtypeOf(&ResolvedType::DYNAMIC),
                    model
                ),
                Truth::Unknown,
            );
        },
    );
}

#[test]
fn mixed_variance_extension_identity_does_not_conflate_missing_arguments() {
    with_model(
        "class Element {} class Pair<T, S> {} extension Mixed<T> on Pair<T, void Function(T)> { void mixed() {} } void uses(Pair<Element, void Function(Element)> pair) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let mut receiver = signatures
                .function(&model.resolve_value("uses").unwrap())
                .unwrap()
                .positional[0]
                .clone();
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            } = &mut receiver
            else {
                panic!()
            };
            Arc::make_mut(arguments)[0] = ResolvedType::UNKNOWN;
            let ResolvedType {
                kind: ResolvedTypeKind::Function { positional, .. },
            } = &mut Arc::make_mut(arguments)[1]
            else {
                panic!()
            };
            Arc::make_mut(positional)[0] = ResolvedType::UNKNOWN;
            assert_eq!(
                signatures
                    .instantiated_extension_on_type(&receiver, &signatures.extensions[0], model,)
                    .1,
                Truth::Unknown,
            );
            assert_eq!(
                signatures.resolved_extension_owner(&receiver, "mixed", model),
                None,
            );
        },
    );
}

#[test]
fn dependent_extension_bounds_do_not_accept_proof_only_dynamic_arguments() {
    with_model(
        "class Element {} class Pair<T, S> {} extension Dependent<T, S extends T> on Pair<T, S> { void get dependent {} } void uses(Pair<Element, Element> pair) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let mut receiver = signatures
                .function(&model.resolve_value("uses").unwrap())
                .unwrap()
                .positional[0]
                .clone();
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            } = &mut receiver
            else {
                panic!()
            };
            Arc::make_mut(arguments)[0] = ResolvedType::UNKNOWN;
            let extension = &signatures.extensions[0];
            assert_eq!(
                signatures
                    .instantiated_extension_on_type(&receiver, extension, model)
                    .1,
                Truth::Unknown,
            );
            assert_eq!(
                signatures.resolved_extension_owner(&receiver, "dependent", model),
                None,
            );
            assert!(signatures.has_possible_extension_getter(&receiver, "dependent", model));
        },
    );
}

#[test]
fn extension_specificity_retains_missing_arguments_instead_of_proof_dynamic() {
    with_model(
        "class Element {} class Pair<T, S> {} extension Constrained<S extends Element> on Pair<dynamic, S> { void choose() {} } extension Ordinary<T, S> on Pair<T, S> { void choose() {} } void uses(Pair<Element, Element> pair) {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let mut receiver = signatures
                .function(&model.resolve_value("uses").unwrap())
                .unwrap()
                .positional[0]
                .clone();
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            } = &mut receiver
            else {
                panic!()
            };
            Arc::make_mut(arguments)[0] = ResolvedType::UNKNOWN;
            let extension = signatures
                .extensions
                .iter()
                .find(|extension| extension.name.as_deref() == Some("Ordinary"))
                .unwrap();
            let (on_type, applicability, _) =
                signatures.instantiated_extension_on_type(&receiver, extension, model);
            assert_eq!(applicability, Truth::Yes);
            assert_eq!(on_type.as_ref(), &receiver);
            assert_eq!(
                signatures.resolved_extension_owner(&receiver, "choose", model),
                None,
            );
        },
    );
}

#[test]
fn extension_identity_proof_distinguishes_declared_from_receiver_derived_holes() {
    with_model(
        "class Element {} class Pair<T, S> {} extension Generic<T> on T { void invoke() {} } extension Fixed on Pair<Element, Element> { void get fixed {} } void uses(void Function(Element) callback, Pair<Element, Element> pair) {}",
        |program, model| {
            let mut signatures = SignatureIndex::from_program(program, model);
            let signature = signatures
                .function(&model.resolve_value("uses").unwrap())
                .unwrap();
            let mut callback = signature.positional[0].clone();
            let mut pair = signature.positional[1].clone();
            let ResolvedType {
                kind: ResolvedTypeKind::Function { positional, .. },
            } = &mut callback
            else {
                panic!()
            };
            Arc::make_mut(positional)[0] = ResolvedType::UNKNOWN;
            assert_eq!(
                signatures.resolved_extension_owner(&callback, "invoke", model),
                model.resolve_extension("Generic"),
            );
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            } = &mut pair
            else {
                panic!()
            };
            Arc::make_mut(arguments)[0] = ResolvedType::UNKNOWN;
            let fixed = signatures
                .extensions
                .iter_mut()
                .find(|extension| extension.name.as_deref() == Some("Fixed"))
                .unwrap();
            let ResolvedType {
                kind: ResolvedTypeKind::Interface { arguments, .. },
            } = &mut fixed.on_type
            else {
                panic!()
            };
            Arc::make_mut(arguments)[0] = ResolvedType::UNKNOWN;
            let fixed = signatures
                .extensions
                .iter()
                .find(|extension| extension.name.as_deref() == Some("Fixed"))
                .unwrap();
            assert_eq!(
                signatures
                    .instantiated_extension_on_type(&pair, fixed, model)
                    .1,
                Truth::Unknown,
            );
            assert!(signatures.has_possible_extension_getter(&pair, "fixed", model));
        },
    );
}

#[test]
fn mixin_super_concrete_lookup_excludes_implemented_interfaces() {
    with_model(
        "class Constraint {} class Interface { String toString() => ''; } mixin Mixed on Constraint implements Interface {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver = ResolvedType::interface_identity(model.resolve_name("Mixed").unwrap());
            assert_eq!(
                signatures.resolved_super_member_owner(&receiver, "toString", false, model),
                Some(DeclarationIdentity::OBJECT),
            );
        },
    );
}

#[test]
fn project_extension_owner_beats_more_specific_platform_extension() {
    with_model(
        "class Receiver {} extension Platform on Receiver { void member() {} } extension Project on Object { void member() {} }",
        |program, model| {
            let mut signatures = SignatureIndex::from_program(program, model);
            signatures
                .extensions
                .iter_mut()
                .find(|extension| extension.name.as_deref() == Some("Platform"))
                .unwrap()
                .identity = Some(DeclarationIdentity::sdk("dart:core", "Platform"));
            let receiver = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: model.resolve_name("Receiver").unwrap(),
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            assert_eq!(
                signatures.resolved_extension_owner(&receiver, "member", model),
                model.resolve_extension("Project")
            );
        },
    );
}

#[test]
fn sdk_set_add_returns_bool() {
    with_model("void f(Set<int> values) {}", |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
            panic!()
        };
        let receiver = model.resolve_type(
            function
                .params()
                .unwrap()
                .params()
                .next()
                .unwrap()
                .param_type()
                .unwrap(),
        );
        let (signature, _) = signatures.resolved_member(&receiver, "add", model).unwrap();
        assert!(signature.return_type.interface("dart:core", "bool"));
    });
}

#[test]
fn sdk_iterable_map_preserves_its_call_type_parameter() {
    with_model(
        "Iterable<String> mapped(List<int> values) => values.map<String>((value) => '$value');",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let iterable = DeclarationIdentity::sdk("dart:core", "Iterable");
            let signature = signatures.member(&iterable, "map").unwrap();
            assert_eq!(signature.call_parameters.len(), 1);
            let ResolvedType {
                kind: ResolvedTypeKind::Function { return_type, .. },
            } = &signature.positional[0]
            else {
                panic!()
            };
            assert_eq!(
                return_type.as_ref(),
                &ResolvedType {
                    kind: ResolvedTypeKind::TypeParameter {
                        id: signature.call_parameters[0].clone(),
                        bound: Arc::new(ResolvedType::DYNAMIC),
                        nullable: false,
                    },
                }
            );

            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!()
            };
            let Some(FunctionBody::Arrow(expression)) = function.body() else {
                panic!()
            };
            let expression = expression.expr();
            let type_parameters = TypeParameterScope::default();
            let mut environment = TypeEnvironment::new();
            environment.bind_params(function.params().unwrap(), model, &type_parameters);
            let inferred =
                environment.infer_with_signatures(expression, model, &signatures, &type_parameters);
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Interface {
                        identity,
                        arguments,
                        ..
                    },
            } = inferred
            else {
                panic!()
            };
            assert_eq!(identity, iterable);
            assert!(arguments[0].interface("dart:core", "String"));
        },
    );
}

#[test]
fn private_inherited_members_are_visible_from_same_library_parts() {
    let root_path = PathBuf::from("/project/lib/root.dart");
    let part_path = PathBuf::from("/project/lib/part.dart");
    let root_source =
        "library shared; part 'part.dart'; class Base { int _method() => 0; int _field = 0; }";
    let falcon_dart_parser::ParseResult {
        tree: root_tree,
        errors: root_errors,
    } = parse(root_source);
    let root_parsed = Arc::new(crate::ParsedFile {
        source: root_source.into(),
        tree: root_tree,
    });
    let part_source = "part of shared; class Child extends Base {}";
    let falcon_dart_parser::ParseResult {
        tree: part_tree,
        errors: part_errors,
    } = parse(part_source);
    let part_parsed = Arc::new(crate::ParsedFile {
        source: part_source.into(),
        tree: part_tree,
    });
    assert!(root_errors.is_empty());
    assert!(part_errors.is_empty());
    let sources = [
        IdentitySource {
            path: &root_path,
            parsed: &root_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &part_path,
            parsed: &part_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: source.has_parse_errors,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files = [
        (root_path, root_parsed.clone()),
        (part_path.clone(), part_parsed.clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    let model = SemanticModel::new(&part_path, &part_parsed, &identities, Some(&types));
    let child = ResolvedType {
        kind: ResolvedTypeKind::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        },
    };

    assert!(
        signatures
            .resolved_member(&child, "_method", &model)
            .is_some()
    );
    assert!(
        signatures
            .resolved_field(&child, "_field", &model)
            .is_some()
    );
    assert!(
        signatures
            .resolved_writable_field(&child, "_field", &model)
            .is_some()
    );
}

#[test]
fn private_inherited_members_are_hidden_across_libraries() {
    let base_path = PathBuf::from("/project/lib/base.dart");
    let child_path = PathBuf::from("/project/lib/child.dart");
    let base_source = "class Base { int _method() => 0; int _field = 0; }";
    let falcon_dart_parser::ParseResult {
        tree: base_tree,
        errors: base_errors,
    } = parse(base_source);
    let base_parsed = Arc::new(crate::ParsedFile {
        source: base_source.into(),
        tree: base_tree,
    });
    let child_program_source = "import 'base.dart'; class Child extends Base {}";
    let falcon_dart_parser::ParseResult {
        tree: child_program_tree,
        errors: child_errors,
    } = parse(child_program_source);
    let child_program_parsed = Arc::new(crate::ParsedFile {
        source: child_program_source.into(),
        tree: child_program_tree,
    });
    assert!(base_errors.is_empty());
    assert!(child_errors.is_empty());
    let sources = [
        IdentitySource {
            path: &base_path,
            parsed: &base_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &child_path,
            parsed: &child_program_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: source.has_parse_errors,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files = [
        (base_path, base_parsed.clone()),
        (child_path.clone(), child_program_parsed.clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    let model = SemanticModel::new(
        &child_path,
        &child_program_parsed,
        &identities,
        Some(&types),
    );
    let child = ResolvedType {
        kind: ResolvedTypeKind::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        },
    };

    assert!(
        signatures
            .resolved_member(&child, "_method", &model)
            .is_none()
    );
    assert!(
        signatures
            .resolved_field(&child, "_field", &model)
            .is_none()
    );
    assert!(
        signatures
            .resolved_writable_field(&child, "_field", &model)
            .is_none()
    );
}

#[test]
fn private_inherited_lookup_skips_invisible_override() {
    let base_path = PathBuf::from("/project/lib/a.dart");
    let middle_path = PathBuf::from("/project/lib/b.dart");
    let base_source = "import 'b.dart'; class Base { int _method() => 0; int _field = 0; } class Child extends Middle {}";
    let falcon_dart_parser::ParseResult {
        tree: base_tree,
        errors: base_errors,
    } = parse(base_source);
    let base_parsed = Arc::new(crate::ParsedFile {
        source: base_source.into(),
        tree: base_tree,
    });
    let middle_source =
        "import 'a.dart'; class Middle extends Base { int _method() => 1; int _field = 1; }";
    let falcon_dart_parser::ParseResult {
        tree: middle_tree,
        errors: middle_errors,
    } = parse(middle_source);
    let middle_parsed = Arc::new(crate::ParsedFile {
        source: middle_source.into(),
        tree: middle_tree,
    });
    assert!(base_errors.is_empty());
    assert!(middle_errors.is_empty());
    let sources = [
        IdentitySource {
            path: &base_path,
            parsed: &base_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &middle_path,
            parsed: &middle_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: source.has_parse_errors,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files = [
        (base_path.clone(), base_parsed.clone()),
        (middle_path, middle_parsed.clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    let model = SemanticModel::new(&base_path, &base_parsed, &identities, Some(&types));
    let base_identity = model.resolve_name("Base").unwrap();
    let child = ResolvedType {
        kind: ResolvedTypeKind::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Child").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        },
    };

    for (name, access) in [
        ("_method", InstanceAccess::Invoke),
        ("_field", InstanceAccess::Read),
        ("_field", InstanceAccess::Write),
    ] {
        let ResolvedType {
            kind: ResolvedTypeKind::Interface { identity, .. },
        } = signatures
            .resolved_instance_owner(&child, name, access, &model)
            .unwrap()
        else {
            panic!()
        };
        assert_eq!(identity, base_identity);
    }
}

#[test]
fn precomputed_cycles_match_uncached_queries_for_every_owner() {
    with_model(
        concat!(
            "class A extends B {} class B extends A {} ",
            "class C extends A {} class D extends C {} ",
            "class E {} class F implements E {} class G implements E, F {}"
        ),
        |program, model| {
            let mut signatures = SignatureIndex::from_program(program, model);
            let cached = signatures.cyclic_supertypes.take().unwrap();
            for (identity, facts) in &signatures.type_facts.local {
                if facts.supertypes.is_none() {
                    continue;
                }
                assert_eq!(
                    cached.contains(identity),
                    signatures.has_cyclic_supertypes(identity),
                    "{identity:?}"
                );
            }
            for name in ["A", "B", "C", "D"] {
                assert!(cached.contains(&model.resolve_name(name).unwrap()));
            }
            for name in ["E", "F", "G"] {
                assert!(!cached.contains(&model.resolve_name(name).unwrap()));
            }
        },
    );
}

#[test]
fn cyclic_supertypes_are_unknown() {
    with_model(
        "class A extends B { int field = 0; void method() {} } class B extends A {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let a = model.resolve_name("A").unwrap();
            let b = model.resolve_name("B").unwrap();
            let receiver = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: a,
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
            };

            assert_eq!(
                signatures.relation(
                    &receiver,
                    crate::TypeRelation::SubtypeOfDeclaration(&b),
                    model
                ),
                Truth::Unknown
            );
            assert!(
                signatures
                    .instantiated_supertype(&receiver, "dart:core", "Object", model)
                    .is_none()
            );
            assert!(
                signatures
                    .resolved_member(&receiver, "method", model)
                    .is_none()
            );
            assert!(
                signatures
                    .resolved_field(&receiver, "field", model)
                    .is_none()
            );
        },
    );
}

#[test]
fn field_formals_preserve_types_through_super_formal_chains() {
    with_model(
        "class Base<T> { final T value; Base(this.value); } class Middle extends Base<String> { Middle(super.value); } class Leaf extends Middle { Leaf(super.value); }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = model.resolve_name("Base").unwrap();
            assert!(contains_parameter(
                &signatures.declaration(&base).unwrap().constructors[0].parameters[0].ty
            ));

            for name in ["Middle", "Leaf"] {
                let identity = model.resolve_name(name).unwrap();
                let facts = signatures.declaration(&identity).unwrap();
                assert!(facts.constructors[0].parameters[0].is_super);
                assert!(
                    facts.constructors[0].parameters[0]
                        .ty
                        .interface("dart:core", "String")
                );
                assert!(
                    signatures.constructor(&identity, "new").unwrap().positional[0]
                        .interface("dart:core", "String")
                );
            }
        },
    );
}

#[test]
fn super_formal_chains_resolve_across_files_regardless_of_index_order() {
    let base_path = PathBuf::from("/project/lib/base.dart");
    let middle_path = PathBuf::from("/project/lib/middle.dart");
    let leaf_path = PathBuf::from("/project/lib/leaf.dart");
    let base_source = "class Base<T> { final T value; Base(this.value); }";
    let falcon_dart_parser::ParseResult {
        tree: base_tree,
        errors: base_errors,
    } = parse(base_source);
    let base_parsed = Arc::new(crate::ParsedFile {
        source: base_source.into(),
        tree: base_tree,
    });
    let middle_source =
        "import 'base.dart'; class Middle extends Base<String> { Middle(super.value); }";
    let falcon_dart_parser::ParseResult {
        tree: middle_tree,
        errors: middle_errors,
    } = parse(middle_source);
    let middle_parsed = Arc::new(crate::ParsedFile {
        source: middle_source.into(),
        tree: middle_tree,
    });
    let leaf_source = "import 'middle.dart'; class Leaf extends Middle { Leaf(super.value); }";
    let falcon_dart_parser::ParseResult {
        tree: leaf_tree,
        errors: leaf_errors,
    } = parse(leaf_source);
    let leaf_parsed = Arc::new(crate::ParsedFile {
        source: leaf_source.into(),
        tree: leaf_tree,
    });
    assert!(base_errors.is_empty());
    assert!(middle_errors.is_empty());
    assert!(leaf_errors.is_empty());
    let sources = [
        IdentitySource {
            path: &leaf_path,
            parsed: &leaf_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &middle_path,
            parsed: &middle_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &base_path,
            parsed: &base_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: source.has_parse_errors,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files = [
        (leaf_path.clone(), leaf_parsed.clone()),
        (middle_path, middle_parsed.clone()),
        (base_path, base_parsed.clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    let model = SemanticModel::new(&leaf_path, &leaf_parsed, &identities, Some(&types));
    let leaf_identity = model.resolve_name("Leaf").unwrap();
    assert!(
        signatures.declaration(&leaf_identity).unwrap().constructors[0].parameters[0]
            .ty
            .interface("dart:core", "String")
    );
    assert!(
        signatures
            .constructor(&leaf_identity, "new")
            .unwrap()
            .positional[0]
            .interface("dart:core", "String")
    );
}

#[test]
fn constructor_formal_fixpoint_handles_deep_chains_across_two_files() {
    let a_path = PathBuf::from("/project/lib/a.dart");
    let b_path = PathBuf::from("/project/lib/b.dart");
    let a_source = "import 'b.dart'; class A0<T> { final T value; A0(this.value); } class A2 extends B1 { A2(super.value); } class A4 extends B3 { A4(super.value); } class A6 extends B5 { A6(super.value); }";
    let falcon_dart_parser::ParseResult {
        tree: a_tree,
        errors: a_errors,
    } = parse(a_source);
    let a_parsed = Arc::new(crate::ParsedFile {
        source: a_source.into(),
        tree: a_tree,
    });
    let b_source = "import 'a.dart'; class B1 extends A0<String> { B1(super.value); } class B3 extends A2 { B3(super.value); } class B5 extends A4 { B5(super.value); }";
    let falcon_dart_parser::ParseResult {
        tree: b_tree,
        errors: b_errors,
    } = parse(b_source);
    let b_parsed = Arc::new(crate::ParsedFile {
        source: b_source.into(),
        tree: b_tree,
    });
    assert!(a_errors.is_empty());
    assert!(b_errors.is_empty());
    let sources = [
        IdentitySource {
            path: &a_path,
            parsed: &a_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &b_path,
            parsed: &b_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: source.has_parse_errors,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files = [
        (a_path.clone(), a_parsed.clone()),
        (b_path, b_parsed.clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    let model = SemanticModel::new(&a_path, &a_parsed, &identities, Some(&types));
    let leaf = model.resolve_name("A6").unwrap();

    assert!(
        signatures.declaration(&leaf).unwrap().constructors[0].parameters[0]
            .ty
            .interface("dart:core", "String")
    );
    assert!(
        signatures.constructor(&leaf, "new").unwrap().positional[0]
            .interface("dart:core", "String")
    );
}

#[test]
fn field_forwarders_check_private_getters_from_their_own_library() {
    // The unrelated library is indexed first: forwarder detection must not
    // judge `_value` from its scope, where every `_` name is invisible.
    let other_path = PathBuf::from("/project/lib/other.dart");
    let fake_path = PathBuf::from("/project/lib/fake.dart");
    let other_source = "class Other {}";
    let falcon_dart_parser::ParseResult {
        tree: other_tree,
        errors: other_errors,
    } = parse(other_source);
    let other_parsed = Arc::new(crate::ParsedFile {
        source: other_source.into(),
        tree: other_tree,
    });
    let fake_source = "abstract class Api { int? get _value; } class Fake { dynamic noSuchMethod(Invocation invocation) => null; } class Impl extends Fake implements Api { final int? _value; Impl(this._value); }";
    let falcon_dart_parser::ParseResult {
        tree: fake_tree,
        errors: fake_errors,
    } = parse(fake_source);
    let fake_parsed = Arc::new(crate::ParsedFile {
        source: fake_source.into(),
        tree: fake_tree,
    });
    assert!(other_errors.is_empty());
    assert!(fake_errors.is_empty());
    let sources = [
        IdentitySource {
            path: &other_path,
            parsed: &other_parsed,
            has_parse_errors: false,
        },
        IdentitySource {
            path: &fake_path,
            parsed: &fake_parsed,
            has_parse_errors: false,
        },
    ];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_resolved_files(
        sources.iter().map(|source| {
            (
                source.path,
                LibrarySource {
                    program: source.parsed.program(),
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )
        }),
        &identities,
    );
    let files = [
        (other_path, other_parsed.clone()),
        (fake_path.clone(), fake_parsed.clone()),
    ];
    let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
    let model = SemanticModel::new(&fake_path, &fake_parsed, &identities, Some(&types));
    let receiver = ResolvedType {
        kind: ResolvedTypeKind::Interface {
            occurrence: Default::default(),
            identity: model.resolve_name("Impl").unwrap(),
            arguments: Vec::new().into(),
            nullable: false,
            extension_type: false,
        },
    };

    assert!(signatures.is_promotable_field(&receiver, "_value", &model));
}

#[test]
fn extension_getters_do_not_supply_override_inference() {
    with_model(
        "class Base {} extension Values on Base { String get value => ''; } \
         class Child extends Base { final value = 1; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = ResolvedType::interface_identity(model.resolve_name("Base").unwrap());
            assert!(signatures.resolved_field(&base, "value", model).is_none());
            assert!(
                signatures
                    .resolved_extension_getter(&base, "value", model)
                    .is_some()
            );
            let child = ResolvedType::interface_identity(model.resolve_name("Child").unwrap());
            let (ty, substitutions) = signatures.resolved_field(&child, "value", model).unwrap();
            assert!(
                model
                    .substitute(&ty, &substitutions)
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn extension_getter_reads_and_callable_getters_still_infer() {
    with_model(
        "class Target {} extension Values on Target { \
         int get value => 1; int Function() get callback => () => 1; } \
         int read(Target target) => target.value; \
         int call(Target target) => target.callback();",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for declaration in program.declarations().skip(2) {
                let TopLevelDecl::Function(function) = declaration else {
                    panic!()
                };
                let Some(FunctionBody::Arrow(expression)) = function.body() else {
                    panic!()
                };
                let expression = expression.expr();
                let scope = TypeParameterScope::default();
                let mut environment = TypeEnvironment::new();
                environment.bind_params(function.params().unwrap(), model, &scope);
                assert!(
                    environment
                        .infer_with_signatures(expression, model, &signatures, &scope)
                        .interface("dart:core", "int")
                );
            }
        },
    );
}

#[test]
fn interface_getter_implementations_do_not_hide_no_such_method_forwarders() {
    with_model(
        "abstract class Contract { int? get _value; } \
         class Api implements Contract { final int? _value; Api(this._value); } \
         class Fake { dynamic noSuchMethod(Invocation invocation) => null; } \
         class Forwarded extends Fake implements Api {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let api = ResolvedType::interface_identity(model.resolve_name("Api").unwrap());
            assert!(!signatures.is_promotable_field(&api, "_value", model));
            let forwarded =
                ResolvedType::interface_identity(model.resolve_name("Forwarded").unwrap());
            assert!(
                signatures
                    .resolved_field(&forwarded, "_value", model)
                    .is_some()
            );
            assert!(
                signatures
                    .resolved_owner_through(
                        &forwarded,
                        "_value",
                        InstanceAccess::Read,
                        false,
                        model,
                    )
                    .is_none()
            );
        },
    );
}

fn contains_parameter(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType {
            kind: ResolvedTypeKind::TypeParameter { .. },
        }
    )
}

#[test]
fn inference_worklist_discovers_receivers_after_reverse_dependencies_settle() {
    use std::fmt::Write as _;
    let mut source = String::with_capacity(16_384);
    source.push_str("final result = receiver.item0; final receiver = Holder(); class Holder {");
    for index in 0..128 {
        write!(source, "final item{index} = item{};", index + 1).unwrap();
    }
    source.push_str("final item128 = seed0; }");
    for index in 0..128 {
        write!(source, "final seed{index} = seed{};", index + 1).unwrap();
    }
    source.push_str("final seed128 = 7;");
    for index in 0..256 {
        write!(source, "final independent{index} = 'unchanged';").unwrap();
    }
    with_model(&source, |program, model| {
        let signatures = SignatureIndex::from_program(program, model);
        let result = signatures
            .value_type(&model.resolve_value("result").unwrap())
            .unwrap();
        assert!(result.interface("dart:core", "int"), "{result:?}");
        let holder = model.resolve_name("Holder").unwrap();
        for index in [0, 64, 128] {
            let ty = &signatures
                .field(&holder, &format!("item{index}"))
                .unwrap()
                .1;
            assert!(ty.interface("dart:core", "int"), "{index}: {ty:?}");
        }
        assert!(
            signatures
                .value_type(&model.resolve_value("independent255").unwrap())
                .unwrap()
                .interface("dart:core", "String")
        );
    });
}

#[test]
fn inference_worklist_preserves_explicit_types_while_forwarding_headers() {
    with_model(
        "class Child extends Base { Child(super.value); \
             echo(dynamic argument) => argument; dynamic get selected => 1; } \
         class Base { final value = seed; Base(this.value); \
             dynamic echo(dynamic argument) => argument; dynamic get selected => 0; } \
         final seed = 7; dynamic pinned = seed; num annotated = seed; \
         final result = Child(1).value; \
         final noEvidence = Plain().selected; \
         class Plain { get selected => 1; echo(argument) => argument; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let child = model.resolve_name("Child").unwrap();
            let constructor = signatures.constructor(&child, "new").unwrap();
            assert!(constructor.positional[0].interface("dart:core", "int"));
            let method = signatures.member(&child, "echo").unwrap();
            assert_eq!(method.positional, [ResolvedType::DYNAMIC]);
            assert_eq!(method.return_type, ResolvedType::DYNAMIC);
            assert_eq!(
                &signatures.field(&child, "selected").unwrap().1,
                &ResolvedType::DYNAMIC
            );
            assert_eq!(
                signatures.value_type(&model.resolve_value("pinned").unwrap()),
                Some(&ResolvedType::DYNAMIC)
            );
            assert!(
                signatures
                    .value_type(&model.resolve_value("annotated").unwrap())
                    .unwrap()
                    .interface("dart:core", "num")
            );
            assert!(
                signatures
                    .value_type(&model.resolve_value("result").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
            let plain = model.resolve_name("Plain").unwrap();
            assert_eq!(
                &signatures.field(&plain, "selected").unwrap().1,
                &ResolvedType::DYNAMIC
            );
            assert_eq!(
                signatures.value_type(&model.resolve_value("noEvidence").unwrap()),
                Some(&ResolvedType::DYNAMIC)
            );
            let method = signatures.member(&plain, "echo").unwrap();
            assert_eq!(method.positional, [ResolvedType::DYNAMIC]);
            assert_eq!(method.return_type, ResolvedType::DYNAMIC);
        },
    );
}

#[test]
fn inference_worklist_publishes_recursive_partial_evidence_only_once() {
    with_model(
        "final result = [receiver.value]; final receiver = Holder(); \
         class Holder { final value = result; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let result = signatures
                .value_type(&model.resolve_value("result").unwrap())
                .unwrap();
            assert!(result.interface("dart:core", "List"));
            assert_eq!(result.arguments(), &[ResolvedType::UNKNOWN]);
            let holder = model.resolve_name("Holder").unwrap();
            assert_eq!(
                &signatures.field(&holder, "value").unwrap().1,
                &ResolvedType::UNKNOWN
            );
        },
    );
}

#[test]
fn inference_worklist_forwards_alias_headers_and_defaults_after_field_inference() {
    with_model(
        "class Alias = Base with Marker; mixin Marker {} \
         class Base { final value = seed; Base(this.value, [int count = 7]); } \
         final result = Alias(1).value; final seed = 1;",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let base = model.resolve_name("Base").unwrap();
            let alias = model.resolve_name("Alias").unwrap();
            let constructor = signatures.constructor(&alias, "new").unwrap();
            assert!(constructor.positional[0].interface("dart:core", "int"));
            assert!(constructor.positional[1].interface("dart:core", "int"));
            let ParameterDefault::Inherited { owner, default } = &constructor.defaults["count"]
            else {
                panic!("alias constructor lost its inherited default");
            };
            assert_eq!(owner, &base);
            let ParameterDefault::Declared(forwarded) = default.as_ref() else {
                panic!("alias constructor changed its declared default");
            };
            let ParameterDefault::Declared(original) =
                &signatures.constructor(&base, "new").unwrap().defaults["count"]
            else {
                panic!("base constructor lost its declared default");
            };
            assert_eq!(forwarded.id, original.id);
            assert!(
                signatures
                    .value_type(&model.resolve_value("result").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn inference_worklist_waits_for_every_inherited_candidate_before_combining() {
    let left = "class Left implements IntBase { conflict() => 1; apply(value) {} }";
    let right = "class Right implements NumBase { conflict() => ''; apply(value) {} }";
    for interfaces in ["Left, Right", "Right, Left"] {
        for (first, last) in [(left, right), (right, left)] {
            let source = format!(
                "abstract class IntBase {{ int conflict(); void apply(int value); }} \
                 abstract class NumBase {{ String conflict(); void apply(num value); }} \
                 {first} \
                 abstract class Combined implements {interfaces} {{ conflict(); apply(value); }} \
                 {last}"
            );
            with_model(&source, |program, model| {
                let signatures = SignatureIndex::from_program(program, model);
                let combined = model.resolve_name("Combined").unwrap();
                assert_eq!(
                    signatures
                        .member(&combined, "conflict")
                        .unwrap()
                        .return_type,
                    ResolvedType::DYNAMIC,
                    "{source}"
                );
                assert!(
                    signatures.member(&combined, "apply").unwrap().positional[0]
                        .interface("dart:core", "num"),
                    "{source}"
                );
            });
        }
    }
}

#[test]
fn inference_worklist_preserves_extension_getters_and_setter_read_placeholders() {
    with_model(
        "extension E on int { static get answer => 42; } \
         final alias = E.answer; \
         class C { set value(next) {} set typed(int next) {} \
             int get paired => 1; set paired(next) {} final answer = value; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            assert_eq!(
                signatures.value_type(&model.resolve_value("alias").unwrap()),
                Some(&ResolvedType::DYNAMIC)
            );
            let owner = model.resolve_name("C").unwrap();
            for name in ["value", "answer"] {
                assert_eq!(
                    &signatures.field(&owner, name).unwrap().1,
                    &ResolvedType::DYNAMIC,
                    "{name}"
                );
            }
            for name in ["typed", "paired"] {
                assert!(
                    signatures
                        .field(&owner, name)
                        .unwrap()
                        .1
                        .interface("dart:core", "int"),
                    "{name}"
                );
            }
        },
    );
}

#[test]
fn inference_worklist_visits_independent_entries_once() {
    use std::fmt::Write as _;
    let mut source = String::from(
        "final head = middle; final middle = tail; final tail = 1; \
         class Stable { final field = 'unchanged'; }",
    );
    for index in 0..128 {
        write!(source, "final independent{index} = 'unchanged';").unwrap();
    }
    let mut independent = Vec::new();
    let mut head = None;
    let counts = index_fixpoint::with_inference_visits(|| {
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            head = model.resolve_value("head");
            assert!(
                signatures
                    .value_type(head.as_ref().unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
            for index in 0..128 {
                independent.push(index_fixpoint::InferenceInput::Value(
                    model.resolve_value(&format!("independent{index}")).unwrap(),
                ));
            }
            independent.push(index_fixpoint::InferenceInput::Member(
                model.resolve_name("Stable").unwrap(),
                "field".into(),
            ));
        });
    });
    for input in independent {
        assert_eq!(counts.get(&input), Some(&1), "{input:?}");
    }
    assert_eq!(
        counts.get(&index_fixpoint::InferenceInput::Value(head.unwrap())),
        Some(&2)
    );
}

#[test]
fn inference_worklist_preserves_permanent_partial_diamond_evidence() {
    with_model(
        "abstract class A { void g(int a, Missing b); } \
         abstract class B implements A {} \
         abstract class C implements A {} \
         class D implements B, C { g(a, b) {} } \
         abstract class E implements B, C {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let method = signatures
                .member(&model.resolve_name("D").unwrap(), "g")
                .unwrap();
            assert!(method.positional[0].interface("dart:core", "int"));
            assert_eq!(method.positional[1], ResolvedType::UNKNOWN);
            let receiver = ResolvedType::interface_identity(model.resolve_name("E").unwrap());
            assert_eq!(
                signatures.resolved_member_owner(&receiver, "g", model),
                model.resolve_name("A")
            );
            let (method, _) = signatures.resolved_member(&receiver, "g", model).unwrap();
            assert!(method.positional[0].interface("dart:core", "int"));
            assert_eq!(method.positional[1], ResolvedType::UNKNOWN);
        },
    );
}

#[test]
fn inference_worklist_settles_distinct_permanent_partials_in_dependency_order() {
    for interfaces in ["Narrow, Wide", "Wide, Narrow"] {
        let source = format!(
            "class Through implements Combined, Wide {{ g(a, b) {{}} }} \
             class Combined implements {interfaces} {{ g(a, b) {{}} }} \
             abstract class Narrow {{ void g(int a, Missing b); }} \
             abstract class Wide {{ void g(num a, Missing b); }} \
             abstract class Lookup implements Narrow, Wide {{}}"
        );
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            for owner in ["Through", "Combined"] {
                let method = signatures
                    .member(&model.resolve_name(owner).unwrap(), "g")
                    .unwrap();
                assert!(
                    method.positional[0].interface("dart:core", "num"),
                    "{owner}: {method:?}"
                );
                assert_eq!(method.positional[1], ResolvedType::UNKNOWN);
            }
            let receiver = ResolvedType::interface_identity(model.resolve_name("Lookup").unwrap());
            assert_eq!(
                signatures.resolved_member_owner(&receiver, "g", model),
                model.resolve_name("Wide")
            );
        });
    }
}

#[test]
fn inherited_diamond_keeps_distinct_instantiations_of_the_same_owner() {
    with_model(
        "abstract class A<T> { void g(T a, Missing b); } \
         abstract class B implements A<int> {} \
         abstract class C implements A<String> {} \
         abstract class D implements B, C {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let receiver = ResolvedType::interface_identity(model.resolve_name("D").unwrap());
            assert!(
                signatures
                    .resolved_member_owner(&receiver, "g", model)
                    .is_none()
            );
            assert!(signatures.resolved_member(&receiver, "g", model).is_none());
        },
    );
}

#[test]
fn inference_worklist_does_not_publish_fallbacks_for_deferred_members() {
    with_model(
        "abstract class I1 { num get v; } \
         abstract class S implements I1, I2 {} \
         class C extends S { get v => 2; } \
         late S receiver; \
         final answer = receiver.v; \
         extension Fallback on S { String get v => 'fallback'; } \
         abstract class I2 { final v = 1; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let owner = model.resolve_name("C").unwrap();
            assert!(
                signatures
                    .field(&owner, "v")
                    .unwrap()
                    .1
                    .interface("dart:core", "int")
            );
            assert!(
                signatures
                    .value_type(&model.resolve_value("answer").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
        },
    );
    with_model(
        "abstract class I1 { Object get v; } \
         abstract class S implements I1, I2 {} \
         class C extends S { final v = 1; } \
         num number = 1; \
         abstract class I2 { final v = number; }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let owner = model.resolve_name("C").unwrap();
            assert!(
                signatures
                    .field(&owner, "v")
                    .unwrap()
                    .1
                    .interface("dart:core", "num")
            );
        },
    );
}

#[test]
fn inference_worklist_settles_shared_getter_setter_output_without_a_false_cycle() {
    with_model(
        "abstract class A { set value(Missing v); } \
         abstract class B { set value(int v); } \
         class C implements B, A { get value => 0; set value(v) {} }",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let owner = model.resolve_name("C").unwrap();
            assert_eq!(
                &signatures.field(&owner, "value").unwrap().1,
                &ResolvedType::DYNAMIC,
            );
            assert!(
                signatures
                    .member_entry(&owner, "value")
                    .unwrap()
                    .write
                    .as_ref()
                    .unwrap()
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn inference_worklist_checks_late_receiver_reads_during_partial_settlement() {
    with_model(
        "late Source source; \
         final answer = source.g(null).h(null); \
         abstract class G1 { Result g(Missing value); } \
         abstract class G2 { Result g(Missing value); } \
         abstract class Source implements G1, G2 {} \
         abstract class NumContract { num h(Missing value); } \
         abstract class H1 { int h(Missing value); } \
         abstract class H2 { int h(Missing value); } \
         class Pending implements H1, H2 { h(value) {} } \
         abstract class Result implements NumContract, Pending {}",
        |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            assert!(
                signatures
                    .value_type(&model.resolve_value("answer").unwrap())
                    .unwrap()
                    .interface("dart:core", "int")
            );
        },
    );
}
