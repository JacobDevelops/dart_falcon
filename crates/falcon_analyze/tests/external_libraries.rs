//! External (SDK and dependency) library loading against a small fake SDK
//! and package configuration, so the test needs no installed Dart SDK.

use std::fs;
use std::path::{Path, PathBuf};

use falcon_analyze::{
    AnalysisEnvironment, DeclarationIdentity, ExternalLibraries, ExternalOrigin, ExternalSources,
    IdentityIndex, IdentitySource, LanguageVersion, LibrarySource, ParameterDefault, ResolvedType,
    SemanticModel, SignatureIndex, TypeIndex, canonical_or_lexical,
};
use falcon_dart_parser::parse;

fn write(root: &Path, relative: &str, text: &str) -> PathBuf {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, text).unwrap();
    path
}

fn fixture_sdk(
    config: Option<&falcon_analyze::environment::PackageConfig>,
) -> Option<falcon_analyze::environment::DartSdk> {
    falcon_analyze::environment::DartSdk::load(
        &config?.flutter_root.as_ref()?.join("bin/cache/dart-sdk"),
    )
}

/// A fake Flutter checkout (`flutterRoot`) with its Dart SDK, a dependency
/// package, and an app whose `package_config.json` ties them together.
fn workspace() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let sdk = "flutter/bin/cache/dart-sdk/lib";
    write(
        root,
        &format!("{sdk}/libraries.json"),
        r#"{"vm": {"libraries": {
            "core": {"uri": "core/core.dart"},
            "async": {"uri": "async/async.dart"}
        }}}"#,
    );
    write(
        root,
        &format!("{sdk}/core/core.dart"),
        "library dart.core;\nexport 'dart:async' show Future;\npart 'list.dart';\n\
         class Object {}\nabstract class Iterable<E> {}\nabstract final class int {}\n",
    );
    write(
        root,
        &format!("{sdk}/core/list.dart"),
        "part of dart.core;\nabstract class List<E> implements Iterable<E> { E get first; }\n",
    );
    write(
        root,
        &format!("{sdk}/async/async.dart"),
        "library dart.async;\nabstract class Future<T> {}\n",
    );
    write(
        root,
        "cache/dep/lib/dep.dart",
        "typedef Callback<T> = void Function(T value);\nclass Base { int get value => 0; }\n",
    );
    let flutter = canonical_or_lexical(&root.join("flutter"));
    let dep = canonical_or_lexical(&root.join("cache/dep"));
    write(
        root,
        "app/.dart_tool/package_config.json",
        &format!(
            r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                {{"name": "dep", "rootUri": "file://{}", "packageUri": "lib/", "languageVersion": "3.4"}},
                {{"name": "app", "rootUri": "../", "packageUri": "lib/", "languageVersion": "3.9"}}
            ]}}"#,
            flutter.display(),
            dep.display()
        ),
    );
    let app = write(
        root,
        "app/lib/main.dart",
        "import 'package:dep/dep.dart';\n\
         class App extends Base {}\n\
         void run(Callback<int>? callback, List<int> values, Future<int> pending) {}\n",
    );
    (dir, canonical_or_lexical(&app))
}

#[test]
fn absent_sdk_parts_do_not_hide_missing_doc_names() {
    for (part, unreadable) in [
        ("missing.dart", false),
        ("missing.dart", true),
        ("existing.dart/child.dart", false),
        ("invalid.dart", false),
    ] {
        let (dir, app) = workspace();
        let sdk = dir.path().join("flutter/bin/cache/dart-sdk/lib/core");
        fs::write(
            sdk.join("core.dart"),
            format!("library dart.core;\npart '{part}';\nclass Object {{}}\n"),
        )
        .unwrap();
        // The analyzer treats failed reads and invalid UTF-8 as empty parts.
        if unreadable {
            fs::create_dir(sdk.join("missing.dart")).unwrap();
        }
        if part.contains('/') {
            fs::write(sdk.join("existing.dart"), "").unwrap();
        }
        if part == "invalid.dart" {
            fs::write(sdk.join(part), [0xff]).unwrap();
        }
        let source = "/// [Object] exists, but [Missing] does not.\nvoid f() {}\n";
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty());
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
        let externals =
            ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |_| {
                false
            });
        let core = externals
            .libraries()
            .iter()
            .find(|library| library.origin == ExternalOrigin::Sdk("dart:core".into()))
            .unwrap();
        assert!(!core.has_unresolved_parts);
        let sources = [IdentitySource {
            path: &app,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::build(
            &sources,
            &[source],
            &[],
            Some(ExternalSources {
                libraries: &externals,
                environment: &environment,
            }),
        );
        let library = identities.library(identities.file(&app).unwrap());
        assert!(matches!(
            identities.doc_name(library, "Object"),
            falcon_analyze::DocName::Declaration { .. }
        ));
        assert_eq!(
            identities.doc_name(library, "Missing"),
            falcon_analyze::DocName::Absent
        );
    }
}

#[test]
fn indexes_sdk_and_dependency_declarations() {
    let (_dir, app) = workspace();
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(&source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    assert!(environment.has_sdk());
    assert_eq!(
        environment.language_version(&app, &source),
        LanguageVersion::new(3, 9)
    );
    assert_eq!(
        environment.language_version(&app, "// @dart = 3.3\nlibrary;"),
        LanguageVersion::new(3, 3)
    );

    let roots = [(app.as_path(), &program, source.as_str())];
    let externals = ExternalLibraries::load(&environment, &roots, |path| path == app);
    let mut origins: Vec<_> = externals
        .libraries()
        .iter()
        .map(|library| library.origin.clone())
        .collect();
    origins.sort_by_key(|origin| format!("{origin:?}"));
    assert_eq!(
        origins,
        [
            ExternalOrigin::Package("dep".into()),
            ExternalOrigin::Sdk("dart:async".into()),
            ExternalOrigin::Sdk("dart:core".into()),
        ]
    );

    let sources = [IdentitySource {
        path: &app,
        program: &program,
        has_parse_errors: false,
    }];
    let external = ExternalSources {
        libraries: &externals,
        environment: &environment,
    };
    let identities = IdentityIndex::build(&sources, &[source.as_str()], &[], Some(external));
    let mut types = TypeIndex::from_resolved_files(
        [(
            app.as_path(),
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let files = [(app.clone(), &program)];
    let signatures = SignatureIndex::build(&files, Some(&externals), &identities, &types);
    let model = SemanticModel::new(&app, &identities, Some(&types));

    assert_eq!(
        model.resolve_name("Base"),
        Some(DeclarationIdentity::package("dep", "Base"))
    );
    // `dart:core` re-exports `Future` from `dart:async`; `List` lives in a part.
    assert!(
        model
            .resolve_name("Future")
            .is_some_and(|identity| identity.is_sdk("dart:async", "Future"))
    );
    let app_type = ResolvedType::interface_identity(model.resolve_name("App").unwrap());
    let (value, _) = signatures
        .resolved_field(&app_type, "value", &model)
        .unwrap();
    assert!(value.interface("dart:core", "int"));

    let int = model.core_type("int", false);
    let list = ResolvedType::Interface {
        occurrence: Default::default(),
        identity: model.resolve_name("List").unwrap(),
        arguments: vec![int.clone()].into(),
        nullable: false,
        extension_type: false,
    };
    let (first, substitutions) = signatures.resolved_field(&list, "first", &model).unwrap();
    assert_eq!(model.substitute(&first, &substitutions), int);

    // Typedefs expand to the aliased type, with arguments and `?` applied.
    let falcon_syntax::ast::TopLevelDecl::Function(run) = &program.declarations[1] else {
        panic!("expected function");
    };
    let callback = model.resolve_type(run.params().unwrap().params[0].param_type.as_ref().unwrap());
    assert_eq!(
        callback,
        ResolvedType::Function {
            shape: Some(falcon_analyze::resolve::FunctionTypeShape::empty()),
            alias: Default::default(),
            optional_positional: 0,
            return_type: Box::new(ResolvedType::Void).into(),
            positional: vec![int].into(),
            named: Vec::new().into(),
            nullable: true,
        }
    );
}

#[test]
fn compact_external_missing_parts_distinguish_absent_and_unresolved() {
    use falcon_analyze::{MemberKind, MemberResult};

    let (dir, app) = workspace();
    let source = "import 'package:dep/dep.dart'; import 'other.dart'; class App {}";
    let other_source = "class Base {}";
    let other = write(dir.path(), "app/lib/other.dart", other_source);
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty());
    let falcon_dart_parser::ParseResult {
        program: other_program,
        errors,
        ..
    } = parse(other_source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    std::fs::create_dir_all(dir.path().join("cache/dep/lib/unreadable.dart")).unwrap();
    write(dir.path(), "cache/dep/lib/not-a-directory", "");
    // Failed part reads are empty units; an unresolved part URI stays unknown.
    for (missing_part, absent) in [
        ("missing.dart", MemberResult::Absent),
        ("not-a-directory/missing.dart", MemberResult::Absent),
        ("unreadable.dart", MemberResult::Absent),
        ("package:unconfigured/missing.dart", MemberResult::Unknown),
    ] {
        write(
            dir.path(),
            "cache/dep/lib/dep.dart",
            &format!("part '{missing_part}'; class ExternalBase {{ int get value => 0; }}"),
        );
        let externals =
            ExternalLibraries::load(&environment, &[(app.as_path(), &program, "")], |path| {
                path == app || path == other
            });
        let dep = externals.libraries().iter()
            .find(|library| matches!(&library.origin, ExternalOrigin::Package(name) if name.as_str() == "dep"))
            .unwrap();
        let unresolved = absent == MemberResult::Unknown;
        assert_eq!(dep.has_unresolved_parts, unresolved, "{missing_part}");
        assert_eq!(dep.files.len(), 1);
        let sources = [
            IdentitySource {
                path: &app,
                program: &program,
                has_parse_errors: false,
            },
            IdentitySource {
                path: &other,
                program: &other_program,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::build(
            &sources,
            &[source, other_source],
            &[],
            Some(ExternalSources {
                libraries: &externals,
                environment: &environment,
            }),
        );
        let mut types = TypeIndex::from_resolved_files(
            [
                (
                    app.as_path(),
                    LibrarySource {
                        program: &program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                ),
                (
                    other.as_path(),
                    LibrarySource {
                        program: &other_program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                ),
            ],
            &identities,
        );
        types.add_external(&externals, &identities);
        let model = SemanticModel::new(&app, &identities, Some(&types));
        assert!(model.resolve_name("Base").is_some(), "{missing_part}");
        assert!(model.resolve_name("App").is_some());
        // Names an incomplete library declares may be redeclared by the part
        // it hides, so importers only bind them when every part is known.
        assert_eq!(
            model.resolve_name("ExternalBase").is_some(),
            absent == MemberResult::Absent,
            "{missing_part}"
        );
        assert_eq!(
            types.member_lookup_identity(
                &DeclarationIdentity::package("dep", "ExternalBase"),
                "absent"
            ),
            absent,
            "{missing_part}"
        );
        assert_eq!(
            types.member_lookup_identity(
                &DeclarationIdentity::package("dep", "ExternalBase"),
                "value"
            ),
            if unresolved {
                MemberResult::Unknown
            } else {
                MemberResult::Found(MemberKind::Getter)
            },
            "{missing_part}"
        );
    }
}

#[test]
fn compact_external_operators_and_constructor_headers_keep_semantic_bits() {
    use falcon_syntax::ast::{ClassMember, ConstructorInitializer, TopLevelDecl};

    let (dir, app) = workspace();
    write(
        dir.path(),
        "cache/dep/lib/dep.dart",
        "import 'middle.dart';\n\
         class Parent { const Parent.named(int value); }\n\
         class Concrete extends Parent {\n\
           final int value;\n\
           Concrete(int input) : value = input, assert(input >= 0, 'nonnegative'), super.named(input) {}\n\
           Concrete.redirect(int input) : this(input);\n\
           bool operator ==(Object other) => true;\n\
         }\n\
         abstract class Abstract { bool operator ==(Object other); }\n\
         class Forward extends Parent { Forward(super.value) : super.named(); }\n\
         class ForwardChain extends Middle { ForwardChain([super.value]); }\n",
    );
    write(
        dir.path(),
        "cache/dep/lib/middle.dart",
        "import 'root.dart'; class Middle extends Root<int> { Middle([super.value]); }",
    );
    write(
        dir.path(),
        "cache/dep/lib/root.dart",
        "class Root<T> { Root([T? value = null]); }",
    );
    // No Flutter/SDK locator: this fixture uses only dependency declarations
    // and the existing source-free core fallback.
    write(
        dir.path(),
        "app/.dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../../cache/dep","packageUri":"lib/","languageVersion":"3.4"}]}"#,
    );
    let source = "import 'package:dep/dep.dart'; class App implements Concrete {}";
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let externals =
        ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
            path == app
        });
    let file = externals.libraries().iter()
        .find(|library| matches!(&library.origin, ExternalOrigin::Package(name) if name.as_str() == "dep"))
        .unwrap().files.first().unwrap();
    assert!(!file.has_parse_errors);
    let TopLevelDecl::Class(concrete) = &file.facts.declarations[1] else {
        panic!("expected compact concrete class");
    };
    let ClassMember::Constructor(constructor) = &concrete.members[1] else {
        panic!("expected compact constructor");
    };
    assert!(constructor.body.is_none());
    assert_eq!(constructor.initializers.len(), 1);
    let ConstructorInitializer::SuperCall {
        call_name, args, ..
    } = &constructor.initializers[0]
    else {
        panic!("expected retained super-constructor name");
    };
    assert_eq!(call_name.as_ref().unwrap().name, "named");
    assert!(args.positional.is_empty() && args.named.is_empty());
    assert_eq!(args.positional.capacity(), 0);
    let ClassMember::Constructor(redirect) = &concrete.members[2] else {
        panic!("expected compact redirecting constructor");
    };
    assert!(redirect.initializers.is_empty());
    let ClassMember::Operator(operator) = &concrete.members[3] else {
        panic!("expected compact operator");
    };
    assert!(operator.body.is_none());
    assert_eq!(file.concrete_operators, [(1, 3)]);

    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &app,
            program: &program,
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
            app.as_path(),
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(app.clone(), &program)],
        Some(&externals),
        &identities,
        &types,
    );
    let model = SemanticModel::new(&app, &identities, Some(&types));
    let concrete = model.resolve_name("Concrete").unwrap();
    let abstract_ = model.resolve_name("Abstract").unwrap();
    let concrete_facts = signatures.declared_member_facts(&concrete, "==");
    assert_eq!(concrete_facts.len(), 1);
    assert!(!concrete_facts[0].is_abstract);
    let abstract_facts = signatures.declared_member_facts(&abstract_, "==");
    assert_eq!(abstract_facts.len(), 1);
    assert!(abstract_facts[0].is_abstract);
    let forward = model.resolve_name("Forward").unwrap();
    assert!(
        signatures.constructor(&forward, "new").unwrap().positional[0]
            .interface("dart:core", "int")
    );
    // External files load importer-first: resolve the generic super-formal
    // chain to completion before value inference, including inherited defaults.
    let chain = model.resolve_name("ForwardChain").unwrap();
    let signature = signatures.constructor(&chain, "new").unwrap();
    assert!(signature.positional[0].interface("dart:core", "int"));
    assert!(signature.positional[0].nullable());
    assert!(signature.defaults.contains_key("value"));
}

#[test]
fn compact_external_types_do_not_collide_with_project_names() {
    use falcon_analyze::{MemberKind, MemberResult, QualifiedName, Truth};

    let (_dir, app) = workspace();
    let source = "import 'package:dep/dep.dart' as dep;\n\
                  class Base { void local() {} }\n\
                  class App extends dep.Base {}\n";
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let externals =
        ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
            path == app
        });
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &app,
            program: &program,
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
            app.as_path(),
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let model = SemanticModel::new(&app, &identities, Some(&types));
    let local = model.resolve_name("Base").unwrap();
    let external = model
        .resolve_name(QualifiedName::prefixed("dep", "Base"))
        .unwrap();
    let child = model.resolve_name("App").unwrap();
    assert_eq!(
        types.member_lookup_identity(&local, "local"),
        MemberResult::Found(MemberKind::Method)
    );
    assert_eq!(
        types.member_lookup_identity(&external, "value"),
        MemberResult::Found(MemberKind::Getter)
    );
    assert_eq!(
        types.member_lookup_identity(&local, "value"),
        MemberResult::Absent
    );
    assert_eq!(types.is_subtype_identity(&child, &external), Truth::Yes);
    assert_eq!(types.is_subtype_identity(&child, &local), Truth::No);
}

#[test]
fn reexports_keep_distinct_same_package_declarations_resolvable() {
    use falcon_analyze::{MemberKind, MemberResult, QualifiedName};

    let (dir, app) = workspace();
    write(
        dir.path(),
        "flutter/bin/cache/dart-sdk/lib/core/core.dart",
        "class Object {} class double {} class int {}",
    );
    for (file, source) in [
        (
            "left.dart",
            "class Vector { Vector(double x); void left() {} } typedef Alias = Vector; extension type Measure(double value) {}",
        ),
        (
            "right.dart",
            "class Vector { Vector(int x); void right() {} } typedef Alias = Vector; extension type Measure(int value) {}",
        ),
        (
            "dep.dart",
            "export 'left.dart' show Vector, Alias, Measure; export 'other.dart' hide Vector, Alias, Measure;",
        ),
        (
            "other.dart",
            "import 'right.dart'; export 'left.dart' show Vector, Alias, Measure;",
        ),
    ] {
        write(dir.path(), &format!("cache/dep/lib/{file}"), source);
    }
    let source = "import 'package:dep/dep.dart'; import 'package:dep/right.dart' as right;";
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let externals =
        ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
            path == app
        });
    let sources = [IdentitySource {
        path: &app,
        program: &program,
        has_parse_errors: false,
    }];
    let base = std::sync::Arc::new(IdentityIndex::build_external(ExternalSources {
        libraries: &externals,
        environment: &environment,
    }));
    for identities in [
        IdentityIndex::build(
            &sources,
            &[source],
            &[],
            Some(ExternalSources {
                libraries: &externals,
                environment: &environment,
            }),
        ),
        IdentityIndex::with_external(
            &sources,
            &[source],
            &[],
            ExternalSources {
                libraries: &externals,
                environment: &environment,
            },
            base,
        ),
    ] {
        let mut types = TypeIndex::from_resolved_files(
            [(
                app.as_path(),
                LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        types.add_external(&externals, &identities);
        let signatures = SignatureIndex::build(
            &[(app.clone(), &program)],
            Some(&externals),
            &identities,
            &types,
        );
        let model = SemanticModel::new(&app, &identities, Some(&types));
        let left = model.resolve_name("Vector").unwrap();
        let right = model
            .resolve_name(QualifiedName::prefixed("right", "Vector"))
            .unwrap();
        assert_ne!(
            model.resolve_name("Measure").unwrap(),
            model
                .resolve_name(QualifiedName::prefixed("right", "Measure"))
                .unwrap(),
        );
        assert_ne!(left, right);
        assert!(identities.is_package_declaration(&left, "dep", "Vector"));
        assert_eq!(
            types.member_lookup_identity(&left, "left"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(
            types.member_lookup_identity(&left, "right"),
            MemberResult::Absent
        );
        assert_eq!(
            types.member_lookup_identity(&right, "right"),
            MemberResult::Found(MemberKind::Method)
        );
        assert!(
            signatures.constructor(&left, "new").unwrap().positional[0]
                .interface("dart:core", "double")
        );
        assert!(
            signatures.constructor(&right, "new").unwrap().positional[0]
                .interface("dart:core", "int")
        );
        assert_eq!(model.unaliased(model.resolve_name("Alias").unwrap()), left);
        assert_eq!(
            model.unaliased(
                model
                    .resolve_name(QualifiedName::prefixed("right", "Alias"))
                    .unwrap()
            ),
            right,
        );
    }
}

#[test]
fn compact_external_facts_keep_body_errors_separate_from_header_errors() {
    let (dir, app) = workspace();
    let dep = write(
        dir.path(),
        "cache/dep/lib/dep.dart",
        "class Base { int method() { var broken = ; return 0; } }\n",
    );
    let dep_source = fs::read_to_string(&dep).unwrap();
    let falcon_dart_parser::ParseResult { errors, .. } = parse(&dep_source);
    assert!(!errors.is_empty());
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(&source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let roots = [(app.as_path(), &program, source.as_str())];
    let externals = ExternalLibraries::load(&environment, &roots, |path| path == app);
    let file = externals
        .libraries()
        .iter()
        .flat_map(|library| library.files.iter())
        .find(|file| file.path == canonical_or_lexical(&dep))
        .unwrap();
    assert!(!file.has_parse_errors);
    let falcon_syntax::ast::TopLevelDecl::Class(base) = &file.facts.declarations[0] else {
        panic!("expected class facts");
    };
    let falcon_syntax::ast::ClassMember::Method(method) = &base.members[0] else {
        panic!("expected method facts");
    };
    assert!(method.body.is_none());

    fs::write(&dep, "class Base {").unwrap();
    let externals = ExternalLibraries::load(&environment, &roots, |path| path == app);
    let file = externals
        .libraries()
        .iter()
        .flat_map(|library| library.files.iter())
        .find(|file| file.path == canonical_or_lexical(&dep))
        .unwrap();
    assert!(file.has_parse_errors);
}

#[test]
fn compact_external_facts_preserve_defaults_constants_and_parts() {
    use falcon_syntax::ast::{ClassMember, Expr, TopLevelDecl};

    let (dir, app) = workspace();
    write(
        dir.path(),
        "cache/dep/lib/dep.dart",
        "part 'facts.dart';\n\
         const answer = 42;\n\
         int compute([int value = 7]) { return value + answer; }\n\
         class Base { const Base({int value = 9}); static const token = 11; \
         int get value => 0; static final inferred = compute(); \
         get inferredValue => inferred; int get discarded => compute(); }\n\
         final forward = later; final later = Box(1); get inferredTop => forward; \
         class Box<T> { Box(T value); }",
    );
    write(
        dir.path(),
        "cache/dep/lib/facts.dart",
        "part of 'dep.dart'; enum Choice { first, second }\n",
    );
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(&source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let roots = [(app.as_path(), &program, source.as_str())];
    let mut externals = ExternalLibraries::load(&environment, &roots, |path| path == app);
    let count = externals.libraries().len();
    externals.extend(&environment, &roots, |path| path == app);
    assert_eq!(externals.libraries().len(), count);
    let dep = externals.libraries().iter().find(|library| {
        matches!(&library.origin, ExternalOrigin::Package(name) if name.as_str() == "dep")
    }).unwrap();
    assert_eq!(dep.files.len(), 2);
    let TopLevelDecl::Function(compute) = &dep.files[0].facts.declarations[1] else {
        panic!("expected function facts");
    };
    assert!(compute.body.is_none());
    assert!(matches!(
        &compute.params().unwrap().params[0].default_value,
        Some(Expr::IntLit { value, .. }) if value == "7"
    ));
    let TopLevelDecl::Class(base) = &dep.files[0].facts.declarations[2] else {
        panic!("expected class facts");
    };
    assert!(base.members.iter().all(|member| match member {
        ClassMember::Constructor(member) => member.body.is_none(),
        ClassMember::Getter(member) => member.body.is_none(),
        _ => true,
    }));
    let sources = [IdentitySource {
        path: &app,
        program: &program,
        has_parse_errors: false,
    }];
    let identities = IdentityIndex::build(
        &sources,
        &[&source],
        &[],
        Some(ExternalSources {
            libraries: &externals,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            app.as_path(),
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(app.clone(), &program)],
        Some(&externals),
        &identities,
        &types,
    );
    let model = SemanticModel::new(&app, &identities, Some(&types));
    let answer = model.resolve_value("answer").unwrap();
    assert!(matches!(
        signatures.constant_initializer(&answer),
        Some(Expr::IntLit { value, .. }) if value == "42"
    ));
    let compute = model.resolve_value("compute").unwrap();
    assert!(matches!(
        signatures.function(&compute).unwrap().defaults.get("value"),
        Some(ParameterDefault::Declared(expr))
            if matches!(expr.as_ref(), Expr::IntLit { value, .. } if value == "7")
    ));
    let base = model.resolve_name("Base").unwrap();
    let facts = signatures.declaration(&base).unwrap();
    assert!(matches!(
        signatures.constructor(&base, "new").unwrap().defaults.get("value"),
        Some(ParameterDefault::Declared(expr))
            if matches!(expr.as_ref(), Expr::IntLit { value, .. } if value == "9")
    ));
    assert!(matches!(
        &facts.static_consts[0].initializer,
        Expr::IntLit { value, .. } if value == "11"
    ));
    assert!(model.resolve_name("Choice").is_some());
    let box_identity = model.resolve_name("Box").unwrap();
    let inferred_box = ResolvedType::Interface {
        occurrence: Default::default(),
        identity: box_identity,
        arguments: vec![model.core_type("int", false)].into(),
        nullable: false,
        extension_type: false,
    };
    for name in ["forward", "later"] {
        assert_eq!(
            signatures.value_type(&model.resolve_value(name).unwrap()),
            Some(&inferred_box)
        );
    }
    for name in ["inferred", "discarded"] {
        assert_eq!(
            signatures.field(&base, name).unwrap().1,
            model.core_type("int", false)
        );
    }
    assert_eq!(
        signatures.value_type(&model.resolve_value("inferredTop").unwrap()),
        Some(&ResolvedType::Dynamic)
    );
    assert_eq!(
        signatures.field(&base, "inferredValue").unwrap().1,
        ResolvedType::Dynamic
    );
}

#[test]
fn compact_external_const_constructors_preserve_instance_values() {
    use falcon_analyze::{ConstantValue, evaluate_constant};
    use falcon_syntax::ast::TopLevelDecl;
    use std::collections::HashMap;

    let (dir, app) = workspace();
    write(
        dir.path(),
        "cache/dep/lib/dep.dart",
        "const _seed = 42; \
         class Value { \
           final int value; final int seed = _seed; \
           const Value([int input = 1]) : value = input, assert(input > 0); \
           const Value.redirect(int input) : this(input); \
         } \
         class Child extends Value { const Child(int input) : super(input); } \
         class Formal { final int value; const Formal({this.value = 1}); }",
    );
    let source = "import 'package:dep/dep.dart'; \
        const first = Value(1); const second = Value(2); \
        const defaulted = Value(); const redirected = Value.redirect(2); \
        const inheritedFirst = Child(1); const inheritedSecond = Child(2); \
        const formalDefault = Formal(); const formalExplicit = Formal(value: 1); \
        const formalOther = Formal(value: 2); const invalid = Value(0); \
        const expectedSeed = 42; const expectedValue = 1;";
    fs::write(&app, source).unwrap();
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let roots = [(app.as_path(), &program, source)];
    let externals = ExternalLibraries::load(&environment, &roots, |path| path == app);
    let sources = [IdentitySource {
        path: &app,
        program: &program,
        has_parse_errors: false,
    }];
    let identities = IdentityIndex::build(
        &sources,
        &[source],
        &[],
        Some(ExternalSources {
            libraries: &externals,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            app.as_path(),
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(app.clone(), &program)],
        Some(&externals),
        &identities,
        &types,
    );
    let model = SemanticModel::new(&app, &identities, Some(&types));
    let values: HashMap<_, _> = program
        .declarations
        .iter()
        .filter_map(|declaration| {
            let TopLevelDecl::Variable(variable) = declaration else {
                return None;
            };
            let declarator = variable.declarators.first()?;
            Some((
                declarator.name.name.as_str(),
                evaluate_constant(
                    declarator.initializer.as_ref()?,
                    None,
                    &HashMap::new(),
                    &model,
                    &signatures,
                ),
            ))
        })
        .collect();
    for name in [
        "first",
        "second",
        "defaulted",
        "redirected",
        "inheritedFirst",
        "inheritedSecond",
        "formalDefault",
        "formalExplicit",
        "formalOther",
    ] {
        assert!(
            matches!(values[name], Some(ConstantValue::Instance(..))),
            "{name}"
        );
    }
    assert_ne!(values["first"], values["second"]);
    assert_eq!(values["defaulted"], values["first"]);
    assert_eq!(values["redirected"], values["second"]);
    assert_ne!(values["inheritedFirst"], values["inheritedSecond"]);
    assert_eq!(values["formalDefault"], values["formalExplicit"]);
    assert_ne!(values["formalDefault"], values["formalOther"]);
    assert_eq!(values["invalid"], None);
    let Some(ConstantValue::Instance(_, _, fields)) = &values["defaulted"] else {
        panic!("expected constant instance");
    };
    assert_eq!(fields.len(), 2);
    for (field, expected) in [("value", "expectedValue"), ("seed", "expectedSeed")] {
        let value = &fields.iter().find(|(_, name, _)| name == field).unwrap().2;
        assert_eq!(Some(value), values[expected].as_ref(), "{field}");
    }
}

#[test]
fn environment_without_configuration_uses_the_current_version() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "a.dart", "void main() {}\n");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([file.as_path()], |_| None);
    assert_eq!(environment.package_language_version(&file), None);
    assert_eq!(
        environment.language_version(&file, "void main() {}"),
        LanguageVersion::CURRENT
    );
}

#[test]
fn analyze_context_caches_the_package_version_and_prefers_the_file_override() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        ".dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"2.19"}]}"#,
    );
    let file = write(dir.path(), "lib/a.dart", "void main() {}\n");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([file.as_path()], |_| None);
    let config = falcon_config::FalconConfig::default();
    let package_version = LanguageVersion::new(2, 19);

    let syntax = parse("void main() {}").syntax;
    let ctx = falcon_analyze::AnalyzeContext::new(&file, "void main() {}", &config, &syntax)
        .with_environment(Some(&environment));
    assert_eq!(ctx.package_language_version, Some(package_version));
    assert_eq!(ctx.language_version(), package_version);

    let source = "// @dart = 2.12\nvoid main() {}";
    let overridden_syntax = parse(source).syntax;
    let overridden =
        falcon_analyze::AnalyzeContext::new(&file, source, &config, &overridden_syntax)
            .with_environment(Some(&environment));
    assert_eq!(overridden.package_language_version, Some(package_version));
    assert_eq!(overridden.language_version(), LanguageVersion::new(2, 12));
}

#[test]
fn imported_static_and_enum_constants_use_external_lexical_context() {
    use falcon_analyze::{ConstantValue, evaluate_constant};
    use falcon_syntax::ast::TopLevelDecl;
    use std::collections::HashMap;

    let (dir, app) = workspace();
    write(
        dir.path(),
        "cache/dep/lib/dep.dart",
        "export 'constants.dart' show Values, Choice;\n",
    );
    write(
        dir.path(),
        "cache/dep/lib/constants.dart",
        "part 'choices.dart'; const _seed = 21; const first = 1; \
         class Values { static const _inside = _seed; static const first = Values._inside; \
         static const answer = first + first; static const cycleA = cycleB; static const cycleB = cycleA; \
         static const _hidden = 0; static final mutable = 42; final instance = 42; }",
    );
    write(
        dir.path(),
        "cache/dep/lib/choices.dart",
        "part of 'constants.dart'; enum Choice { first, second; static const selected = first; }",
    );
    let source = "import 'package:dep/dep.dart' as dep; import 'package:dep/dep.dart'; const _seed = 1; \
        const answer = dep.Values.answer; const choice = dep.Choice.first; \
        const selected = dep.Choice.selected; const other = dep.Choice.second; \
        const hidden = dep.Values._hidden; const mutable = dep.Values.mutable; \
        const instance = dep.Values.instance; const cyclic = dep.Values.cycleA; \
        const unprefixedAnswer = Values.answer; const unprefixedChoice = Choice.first;";
    fs::write(&app, source).unwrap();
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let roots = [(app.as_path(), &program, source)];
    let externals = ExternalLibraries::load(&environment, &roots, |path| path == app);
    let sources = [IdentitySource {
        path: &app,
        program: &program,
        has_parse_errors: false,
    }];
    let identities = IdentityIndex::build(
        &sources,
        &[source],
        &[],
        Some(ExternalSources {
            libraries: &externals,
            environment: &environment,
        }),
    );
    let mut types = TypeIndex::from_resolved_files(
        [(
            app.as_path(),
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(app.clone(), &program)],
        Some(&externals),
        &identities,
        &types,
    );
    let model = SemanticModel::new(&app, &identities, Some(&types));
    let values: HashMap<_, _> = program
        .declarations
        .iter()
        .filter_map(|declaration| {
            let TopLevelDecl::Variable(variable) = declaration else {
                return None;
            };
            let declarator = variable.declarators.first()?;
            Some((
                declarator.name.name.as_str(),
                evaluate_constant(
                    declarator.initializer.as_ref()?,
                    None,
                    &HashMap::new(),
                    &model,
                    &signatures,
                ),
            ))
        })
        .collect();
    let expected = parse("const expected = 42;").program;
    let TopLevelDecl::Variable(expected) = &expected.declarations[0] else {
        panic!()
    };
    assert_eq!(
        values["answer"],
        evaluate_constant(
            expected.declarators[0].initializer.as_ref().unwrap(),
            None,
            &HashMap::new(),
            &model,
            &signatures
        ),
    );
    assert!(matches!(
        values["choice"],
        Some(ConstantValue::EnumValue(..))
    ));
    assert_eq!(values["choice"], values["selected"]);
    assert!(matches!(
        values["other"],
        Some(ConstantValue::EnumValue(..))
    ));
    assert_eq!(values["answer"], values["unprefixedAnswer"]);
    assert_eq!(values["choice"], values["unprefixedChoice"]);
    assert_ne!(values["choice"], values["other"]);
    for name in ["hidden", "mutable", "instance", "cyclic"] {
        assert_eq!(values[name], None, "{name}");
    }
    let owner = model.resolve_dotted(&["dep", "Values"]).unwrap();
    assert!(owner.is_package("dep", "Values"));
    assert!(
        signatures
            .declaration(&owner)
            .unwrap()
            .static_consts
            .iter()
            .any(|constant| constant.name == "answer")
    );
}

#[test]
fn inherited_object_members_use_the_effective_superclass_and_mixins() {
    use falcon_analyze::{MemberKind, Truth};

    let (dir, app) = workspace();
    write(
        dir.path(),
        "flutter/bin/cache/dart-sdk/lib/core/core.dart",
        "library dart.core;\n\
         class Object { dynamic noSuchMethod(Invocation invocation) => null; }\n\
         abstract class Enum {}\n\
         abstract class Invocation {}\n\
         abstract final class int {}\n",
    );
    // Accepted without errors by the Dart analyzer. Covariance makes this a
    // valid override, but its function type is not a subtype of Object's:
    // its first parameter is narrower and it adds an optional parameter.
    let source = r#"
abstract class MyInvocation implements Invocation {}
class A {
  @override
  dynamic noSuchMethod(covariant MyInvocation i, [int x = 0]) => null;
}
class B extends A {
  dynamic noSuchMethod(Invocation i, [int x = 0]) => null;
}
mixin Replacement {
  @override
  dynamic noSuchMethod(covariant MyInvocation i, [int x = 0]) => null;
}
class Applied with Replacement {
  dynamic noSuchMethod(Invocation i, [int x = 0]) => null;
}
class ExtendedAndApplied extends A with Replacement {
  dynamic noSuchMethod(Invocation i, [int x = 0]) => null;
}
enum AppliedEnum with Replacement {
  value;
  dynamic noSuchMethod(Invocation i, [int x = 0]) => null;
}
class FirstValue {}
class SecondValue {}
abstract class Generic<T> { T make(); }
abstract class Left implements Generic<FirstValue> {}
abstract class Right implements Generic<SecondValue> {}
abstract class ConflictingDiamond implements Left, Right {}
abstract class SameLeft implements Generic<FirstValue> {}
abstract class SameRight implements Generic<FirstValue> {}
abstract class EquivalentDiamond implements SameLeft, SameRight {}
"#;
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let externals =
        ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
            path == app
        });
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path: &app,
            program: &program,
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
            app.as_path(),
            LibrarySource {
                program: &program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&externals, &identities);
    let signatures = SignatureIndex::build(
        &[(app.clone(), &program)],
        Some(&externals),
        &identities,
        &types,
    );
    let model = SemanticModel::new(&app, &identities, Some(&types));
    for name in ["A", "B", "Applied", "ExtendedAndApplied", "AppliedEnum"] {
        let identity = model.resolve_name(name).unwrap();
        assert!(
            signatures.has_combined_inherited_member(
                &identity,
                "noSuchMethod",
                MemberKind::Method,
                &model,
            ),
            "{name}",
        );
        assert_eq!(
            signatures.implements_sdk(
                &ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity,
                    arguments: Vec::new().into(),
                    nullable: false,
                    extension_type: false,
                },
                "dart:core",
                "Object",
            ),
            Truth::Yes,
            "{name} must retain Object in the subtype graph",
        );
    }
    for (name, expected) in [("ConflictingDiamond", false), ("EquivalentDiamond", true)] {
        assert_eq!(
            signatures.has_combined_inherited_member(
                &model.resolve_name(name).unwrap(),
                "make",
                MemberKind::Method,
                &model,
            ),
            expected,
            "{name} must substitute each generic path independently",
        );
    }
}

#[test]
fn doc_imports_share_the_transitive_external_loader() {
    let (dir, app) = workspace();
    write(
        dir.path(),
        "cache/dep/lib/doc.dart",
        "export 'dep.dart'; class Documented {}",
    );
    let source = "/// @docImport 'package:dep/doc.dart';\nlibrary;\n/// [Documented]\nvoid f() {}";
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let external =
        ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
            path == app
        });
    for name in ["doc.dart", "dep.dart"] {
        assert!(
            external
                .libraries()
                .iter()
                .flat_map(|library| library.files.iter())
                .any(|file| file.path.ends_with(name)),
            "{name}"
        );
    }
    assert!(!environment.has_sdk());
}

#[test]
fn unindexed_project_package_imports_and_exports_in_dependencies_remain_uncertain() {
    for directive in ["import", "export"] {
        let (dir, app) = workspace();
        let dependency = write(
            dir.path(),
            "cache/dep/lib/dep.dart",
            &format!("{directive} 'known.dart';\n{directive} 'package:app/generated.dart';\n"),
        );
        let known = write(dir.path(), "cache/dep/lib/known.dart", "class Known {}");
        let source = "import 'package:dep/dep.dart';";
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
        let external =
            ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
                path == app
            });
        let sources = [IdentitySource {
            path: &app,
            program: &program,
            has_parse_errors: false,
        }];
        let packages = [falcon_analyze::PackageIdentity {
            name: "app".into(),
            lib_root: canonical_or_lexical(&dir.path().join("app/lib")),
        }];
        let identities = IdentityIndex::build(
            &sources,
            &[source],
            &packages,
            Some(ExternalSources {
                libraries: &external,
                environment: &environment,
            }),
        );
        assert_eq!(
            SemanticModel::new(&known, &identities, None).resolve_name("Known"),
            Some(DeclarationIdentity::package("dep", "Known"))
        );
        let scope = if directive == "import" {
            &dependency
        } else {
            &app
        };
        assert_eq!(
            SemanticModel::new(scope, &identities, None).resolve_name("Known"),
            None,
            "{directive} of an unindexed project library may also supply Known"
        );
    }
}

#[test]
fn external_doc_imports_keep_package_fallback_unless_configuration_proves_absence() {
    use falcon_analyze::{DocName, NameIdentity};

    for configured in [false, true] {
        let (dir, app) = workspace();
        if !configured {
            fs::remove_file(dir.path().join("app/.dart_tool/package_config.json")).unwrap();
        }
        let dependency = write(
            dir.path(),
            "cache/dep/lib/dep.dart",
            "/// @docImport 'package:meta/meta.dart';\nlibrary;\nclass Documented {}",
        );
        let source = "import '../../cache/dep/lib/dep.dart';";
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
        let external =
            ExternalLibraries::load(&environment, &[(app.as_path(), &program, source)], |path| {
                path == app
            });
        let sources = [IdentitySource {
            path: &app,
            program: &program,
            has_parse_errors: false,
        }];
        let identities = IdentityIndex::build(
            &sources,
            &[source],
            &[],
            Some(ExternalSources {
                libraries: &external,
                environment: &environment,
            }),
        );
        let library = identities.library(identities.file(&dependency).unwrap());
        assert_eq!(
            identities.library_name(library, "immutable"),
            DocName::Absent
        );
        assert_eq!(
            identities.doc_name(library, "immutable"),
            if configured {
                DocName::Absent
            } else {
                DocName::Declaration {
                    identity: Some(DeclarationIdentity::package("meta", "immutable")),
                    kind: NameIdentity::Value,
                }
            },
            "configured: {configured}"
        );
    }
}

#[test]
fn opening_external_files_evicts_the_disk_copy_and_allows_reloading_after_close() {
    let (dir, app) = workspace();
    let dependency = canonical_or_lexical(&dir.path().join("cache/dep/lib/dep.dart"));
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult { program, .. } = parse(&source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let roots = [(app.as_path(), &program, source.as_str())];
    let mut external = ExternalLibraries::load(&environment, &roots, |path| path == app);
    assert!(!external.is_empty());
    external.extend(&environment, &roots, |path| {
        path == app || path == dependency
    });
    assert!(
        !external
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .any(|file| file.path == dependency)
    );
    external.extend(&environment, &roots, |path| path == app);
    assert!(
        external
            .libraries()
            .iter()
            .flat_map(|library| library.files.iter())
            .any(|file| file.path == dependency)
    );
}

#[test]
fn external_snapshots_reload_dependency_edits_and_removal_explicitly() {
    let (dir, app) = workspace();
    let dependency = dir.path().join("cache/dep/lib/dep.dart");
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult { program, .. } = parse(&source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let roots = [(app.as_path(), &program, source.as_str())];
    let external = ExternalLibraries::load(&environment, &roots, |path| path == app);
    assert!(external_has_class(&external, "Base"));
    fs::write(&dependency, "class ChangedDependency {}").unwrap();
    let external = ExternalLibraries::load(&environment, &roots, |path| path == app);
    assert!(external_has_class(&external, "ChangedDependency"));
    fs::remove_file(&dependency).unwrap();
    let removed = ExternalLibraries::load(&environment, &roots, |path| path == app);
    assert!(!external_has_class(&removed, "ChangedDependency"));
}

#[test]
fn configuration_stamps_detect_first_and_nested_package_creation() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "nested/lib/a.dart", "");
    let mut environment =
        AnalysisEnvironment::discover_with_sdk_locator([file.as_path()], |_| None);
    let absent = environment.config_stamps();
    write(
        dir.path(),
        ".dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"outer","rootUri":"../","packageUri":"lib/","languageVersion":"3.4"}]}"#,
    );
    assert_ne!(environment.config_stamps(), absent);
    environment.reset();
    environment.extend([file.as_path()]);
    let outer = environment.config_stamps();
    write(
        dir.path(),
        "nested/.dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"inner","rootUri":"../","packageUri":"lib/","languageVersion":"2.19"}]}"#,
    );
    assert_ne!(environment.config_stamps(), outer);
    environment.reset();
    environment.extend([file.as_path()]);
    assert_eq!(
        environment.package_language_version(&file),
        Some(LanguageVersion::new(2, 19))
    );
    assert!(!environment.has_sdk());
}

#[test]
fn configuration_stamps_detect_same_time_same_size_changes_without_sdk() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(dir.path(), "lib/a.dart", "");
    let config = write(
        dir.path(),
        ".dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.4"}]}"#,
    );
    let mut environment =
        AnalysisEnvironment::discover_with_sdk_locator([file.as_path()], |_| None);
    let before = environment.config_stamps();
    let modified = fs::metadata(&config).unwrap().modified().unwrap();
    let contents = fs::read_to_string(&config).unwrap();
    fs::write(&config, contents.replace("3.4", "3.9")).unwrap();
    fs::File::options()
        .write(true)
        .open(&config)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_ne!(environment.config_stamps(), before);
    environment.reset();
    environment.extend([file.as_path()]);
    assert_eq!(
        environment.package_language_version(&file),
        Some(LanguageVersion::new(3, 9))
    );
    assert!(!environment.has_sdk());
    assert_eq!(environment.config_stamps(), environment.config_stamps());
}

#[test]
fn explicit_reload_observes_same_time_same_size_rewrites_and_atomic_replacements() {
    let (dir, app) = workspace();
    let dependency = dir.path().join("cache/dep/lib/dep.dart");
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult { program, .. } = parse(&source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let roots = [(app.as_path(), &program, source.as_str())];
    for atomic in [false, true] {
        let metadata = fs::metadata(&dependency).unwrap();
        let text = fs::read_to_string(&dependency).unwrap();
        let changed = if text.contains("Base") {
            text.replace("Base", "Next")
        } else {
            text.replace("Next", "Base")
        };
        assert_ne!(changed, text);
        assert_eq!(changed.len(), text.len());
        let target = if atomic {
            dependency.with_extension("replacement")
        } else {
            dependency.clone()
        };
        fs::write(&target, changed).unwrap();
        fs::File::options()
            .write(true)
            .open(&target)
            .unwrap()
            .set_times(fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        if atomic {
            fs::rename(&target, &dependency).unwrap();
        }
        assert_eq!(
            fs::metadata(&dependency).unwrap().modified().unwrap(),
            metadata.modified().unwrap()
        );
        assert_eq!(fs::metadata(&dependency).unwrap().len(), metadata.len());
        let refreshed = ExternalLibraries::load(&environment, &roots, |path| path == app);
        let file = &refreshed.libraries().iter()
            .find(|library| matches!(&library.origin, ExternalOrigin::Package(name) if name.as_str() == "dep"))
            .unwrap().files[0];
        assert!(file.facts.declarations.iter().any(|decl| {
            matches!(decl, falcon_syntax::ast::TopLevelDecl::Class(class) if class.name.name == if atomic { "Base" } else { "Next" })
        }));
    }
}

#[cfg(unix)]
#[test]
fn configuration_stamps_detect_retargeted_package_roots_without_config_changes() {
    let dir = tempfile::tempdir().unwrap();
    let app = write(
        dir.path(),
        "app/lib/main.dart",
        "import 'package:dep/dep.dart';",
    );
    let first = write(dir.path(), "first/lib/dep.dart", "class First {}");
    let second = write(dir.path(), "second/lib/dep.dart", "class Second {}");
    let link = dir.path().join("dep");
    std::os::unix::fs::symlink(dir.path().join("first"), &link).unwrap();
    let config = write(
        dir.path(),
        "app/.dart_tool/package_config.json",
        r#"{"configVersion":2,"packages":[{"name":"dep","rootUri":"../../dep","packageUri":"lib/"}]}"#,
    );
    let mut environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let stamps = environment.config_stamps();
    let config_bytes = fs::read(&config).unwrap();
    assert_eq!(
        environment
            .package_config(&app)
            .unwrap()
            .resolve_package_uri("package:dep/dep.dart"),
        Some(canonical_or_lexical(&first))
    );
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(dir.path().join("second"), &link).unwrap();
    assert_eq!(fs::read(&config).unwrap(), config_bytes);
    assert_ne!(environment.config_stamps(), stamps);
    environment.reset();
    environment.extend([app.as_path()]);
    assert_eq!(
        environment
            .package_config(&app)
            .unwrap()
            .resolve_package_uri("package:dep/dep.dart"),
        Some(canonical_or_lexical(&second))
    );
    assert!(!environment.has_sdk());
}

#[test]
fn external_documentation_imports_load_transitively_and_resolve_in_overlays() {
    let (dir, app) = workspace();
    let dependency = write(
        dir.path(),
        "cache/dep/lib/dep.dart",
        "/// @docImport 'doc.dart';\nlibrary dep;\nclass Base {}",
    );
    write(
        dir.path(),
        "cache/dep/lib/doc.dart",
        "/// @docImport 'leaf.dart';\nlibrary doc;\nclass Documented {}",
    );
    write(dir.path(), "cache/dep/lib/leaf.dart", "class Leaf {}");
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult { program, .. } = parse(&source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let external = ExternalLibraries::load(
        &environment,
        &[(app.as_path(), &program, &source)],
        |path| path == app,
    );
    assert_eq!(external.libraries().len(), 3);
    let sources = ExternalSources {
        libraries: &external,
        environment: &environment,
    };
    let base = std::sync::Arc::new(IdentityIndex::build_external(sources));
    let inputs = [IdentitySource {
        path: &app,
        program: &program,
        has_parse_errors: false,
    }];
    let overlay = IdentityIndex::with_external(&inputs, &[&source], &[], sources, base);
    let library = overlay.library(overlay.file(&dependency).unwrap());
    assert!(matches!(overlay.doc_name(library, "Documented"),
        falcon_analyze::DocName::Declaration { identity: Some(DeclarationIdentity::Package { name, .. }), .. } if name.as_str() == "Documented"));
}

#[test]
fn configuration_stamps_detect_same_time_sdk_manifest_changes_without_installed_sdk() {
    let (dir, app) = workspace();
    let mut environment =
        AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let stamps = environment.config_stamps();
    let manifest = dir
        .path()
        .join("flutter/bin/cache/dart-sdk/lib/libraries.json");
    let modified = fs::metadata(&manifest).unwrap().modified().unwrap();
    let text = fs::read_to_string(&manifest).unwrap();
    let replacement = write(
        dir.path(),
        "flutter/bin/cache/dart-sdk/lib/core/next.dart",
        "class Object {}",
    );
    fs::write(&manifest, text.replace("core/core.dart", "core/next.dart")).unwrap();
    fs::File::options()
        .write(true)
        .open(&manifest)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    assert_ne!(environment.config_stamps(), stamps);
    environment.reset();
    environment.extend([app.as_path()]);
    assert_eq!(
        environment.sdk(&app).unwrap().library("dart:core"),
        Some(canonical_or_lexical(&replacement).as_path())
    );
}

#[test]
fn missing_external_package_sources_and_parts_are_available_after_explicit_reload() {
    let (dir, app) = workspace();
    let source = "import 'package:dep/generated.dart';";
    let falcon_dart_parser::ParseResult { program, .. } = parse(source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let roots = [(app.as_path(), &program, source)];
    let missing_library = ExternalLibraries::load(&environment, &roots, |path| path == app);
    assert!(
        !missing_library
            .libraries()
            .iter()
            .any(|library| library.files[0].path.ends_with("generated.dart"))
    );
    write(
        dir.path(),
        "cache/dep/lib/generated.dart",
        "library generated;\npart 'generated_part.dart';",
    );
    let missing_part = ExternalLibraries::load(&environment, &roots, |path| path == app);
    assert_eq!(
        missing_part
            .libraries()
            .iter()
            .find(|library| library.files[0].path.ends_with("generated.dart"))
            .unwrap()
            .files
            .len(),
        1
    );
    write(
        dir.path(),
        "cache/dep/lib/generated_part.dart",
        "part of generated;\nclass Generated {}",
    );
    let complete = ExternalLibraries::load(&environment, &roots, |path| path == app);
    assert_eq!(complete.libraries()[0].files.len(), 2);
}

#[test]
fn replacing_package_directory_refreshes_facts_on_explicit_reload() {
    let (dir, app) = workspace();
    let source = fs::read_to_string(&app).unwrap();
    let falcon_dart_parser::ParseResult { program, .. } = parse(&source);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let external = ExternalLibraries::load(
        &environment,
        &[(app.as_path(), &program, source.as_str())],
        |path| path == app,
    );
    assert!(external_has_class(&external, "Base"));
    let root = dir.path().join("cache/dep");
    let dependency = root.join("lib/dep.dart");
    let original = fs::read_to_string(&dependency).unwrap();
    let modified = fs::metadata(&dependency).unwrap().modified().unwrap();
    let replacement = write(
        dir.path(),
        "cache/replacement/lib/dep.dart",
        &original.replace("Base", "Next"),
    );
    fs::File::options()
        .write(true)
        .open(&replacement)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(modified))
        .unwrap();
    fs::rename(&root, dir.path().join("cache/old")).unwrap();
    fs::rename(dir.path().join("cache/replacement"), &root).unwrap();
    let refreshed = ExternalLibraries::load(
        &environment,
        &[(app.as_path(), &program, source.as_str())],
        |path| path == app,
    );
    assert!(external_has_class(&refreshed, "Next"));
    assert!(!external_has_class(&refreshed, "Base"));
}

#[test]
fn transitive_libraries_and_parts_reload_after_project_buffers_close_without_sdk() {
    let (dir, app) = workspace();
    write(
        dir.path(),
        "cache/dep/lib/bridge.dart",
        "export 'leaf.dart';",
    );
    let leaf = write(
        dir.path(),
        "cache/dep/lib/leaf.dart",
        "library leaf; part 'leaf_part.dart'; class Leaf {}",
    );
    let leaf = leaf.canonicalize().unwrap();
    let part = write(
        dir.path(),
        "cache/dep/lib/leaf_part.dart",
        "part of leaf; class LeafPart {}",
    );
    let part = part.canonicalize().unwrap();
    let source = "import 'package:dep/bridge.dart';";
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], |_| None);
    let roots = [(app.as_path(), &program, source)];
    for opened in [&leaf, &part] {
        for warm in [false, true] {
            let mut libraries = if warm {
                ExternalLibraries::load(&environment, &roots, |path| path == app)
            } else {
                ExternalLibraries::default()
            };
            libraries.extend(&environment, &roots, |path| path == app || path == opened);
            let files: Vec<_> = libraries
                .libraries()
                .iter()
                .flat_map(|library| library.files.iter())
                .map(|file| &file.path)
                .collect();
            assert!(
                !files.contains(&&leaf),
                "whole library must leave the external index"
            );
            assert!(
                !files.contains(&&part),
                "parts must never survive as orphan libraries"
            );
            if opened == &part {
                let companions: Vec<_> = libraries.project_companions().collect();
                assert!(companions.contains(&leaf.as_path()));
                assert!(!companions.contains(&part.as_path()));
            }
            libraries.extend(&environment, &roots, |path| path == app);
            let files: Vec<_> = libraries
                .libraries()
                .iter()
                .flat_map(|library| library.files.iter())
                .map(|file| &file.path)
                .collect();
            assert_eq!(
                files
                    .iter()
                    .filter(|path| path.as_path() == leaf.as_path())
                    .count(),
                1
            );
            assert_eq!(
                files
                    .iter()
                    .filter(|path| path.as_path() == part.as_path())
                    .count(),
                1
            );
            let owner = libraries
                .libraries()
                .iter()
                .find(|library| library.files[0].path == leaf)
                .unwrap();
            assert_eq!(owner.files.len(), 2);
            assert_eq!(owner.files[1].path, part);
        }
    }
}

fn external_has_class(libraries: &ExternalLibraries, name: &str) -> bool {
    libraries
        .libraries()
        .iter()
        .flat_map(|library| library.files.iter())
        .flat_map(|file| &file.facts.declarations)
        .any(|declaration| {
            matches!(declaration,
            falcon_syntax::ast::TopLevelDecl::Class(class) if class.name.name == name)
        })
}

#[test]
fn executable_library_names_preserve_collisions_without_documentation_imports() {
    use falcon_analyze::DocName;

    let dir = tempfile::tempdir().unwrap();
    let source = "/// @docImport 'docs.dart';\nlibrary;\nimport 'first.dart';\nimport 'second.dart';\nimport 'first.dart' as pref;\nimport 'package:missing/missing.dart' show hidden;\nextension Named on int {}";
    let texts = [
        ("app.dart", source),
        ("first.dart", "int collision = 1;"),
        ("second.dart", "int collision = 2;"),
        ("docs.dart", "class Documented {}"),
    ];
    let paths: Vec<_> = texts
        .iter()
        .map(|(name, text)| write(dir.path(), name, text))
        .collect();
    let programs: Vec<_> = texts
        .iter()
        .map(|(_, text)| {
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(text);
            assert!(errors.is_empty(), "{errors:?}");
            program
        })
        .collect();
    let sources: Vec<_> = paths
        .iter()
        .zip(&programs)
        .map(|(path, program)| IdentitySource {
            path,
            program,
            has_parse_errors: false,
        })
        .collect();
    let content: Vec<_> = texts.iter().map(|(_, text)| *text).collect();
    let identities = IdentityIndex::build(&sources, &content, &[], None);
    let model = SemanticModel::new(&paths[0], &identities, None);

    assert!(matches!(
        model.resolve_library_name("collision"),
        DocName::Declaration { identity: None, .. }
    ));
    assert_eq!(model.resolve_library_name("pref"), DocName::Prefix);
    assert_eq!(model.resolve_library_name("Named"), DocName::Extension);
    assert_eq!(model.resolve_library_name("hidden"), DocName::Unknown);
    assert_eq!(model.resolve_library_name("absent"), DocName::Absent);
    assert_eq!(model.resolve_library_name("Documented"), DocName::Absent);
    assert!(matches!(
        model.resolve_doc_name("Documented"),
        DocName::Declaration {
            identity: Some(_),
            ..
        }
    ));
}

#[test]
fn package_contexts_resolve_package_uris_to_their_own_copies_in_every_order() {
    use falcon_analyze::{MemberKind, MemberResult, MemberSelector, MemberSelectorKind};

    let (dir, _) = workspace();
    let source = "import 'package:foo/foo.dart';";
    let flutter = canonical_or_lexical(&dir.path().join("flutter"));
    let apps: Vec<_> = [
        (
            "getter",
            "@Deprecated('getter type') class Foo { @Deprecated('getter member') int get value => 1; }",
        ),
        (
            "method",
            "@Deprecated('method type') class Foo { @Deprecated('method member') Foo value(Foo argument) => argument; }",
        ),
    ]
    .into_iter()
    .map(|(name, declaration)| {
        let package = dir.path().join(format!("packages/{name}"));
        write(&package, "lib/foo.dart", declaration);
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                    {{"name": "foo", "rootUri": "file://{}", "packageUri": "lib/"}}
                ]}}"#,
                flutter.display(),
                canonical_or_lexical(&package).display(),
            ),
        );
        canonical_or_lexical(&write(
            dir.path(),
            &format!("{name}/lib/main.dart"),
            source,
        ))
    })
    .collect();
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");

    for order in [[0, 1], [1, 0]] {
        let paths = order.map(|index| apps[index].as_path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, fixture_sdk);
        let roots = paths.map(|path| (path, &program, source));
        let is_project = |path: &Path| apps.iter().any(|app| app == path);
        for incremental in [false, true] {
            let externals = if incremental {
                let mut externals = ExternalLibraries::load(&environment, &roots[..1], is_project);
                externals.extend(&environment, &roots[1..], is_project);
                externals
            } else {
                ExternalLibraries::load(&environment, &roots, is_project)
            };
            let sources = paths.map(|path| IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            });
            let external = ExternalSources {
                libraries: &externals,
                environment: &environment,
            };
            let base = std::sync::Arc::new(IdentityIndex::build_external(external));
            for identities in [
                IdentityIndex::build(&sources, &[source, source], &[], Some(external)),
                IdentityIndex::with_external(
                    &sources,
                    &[source, source],
                    &[],
                    external,
                    base.clone(),
                ),
            ] {
                let mut types = TypeIndex::from_resolved_files(
                    paths.map(|path| {
                        (
                            path,
                            LibrarySource {
                                program: &program,
                                has_parse_errors: false,
                                has_unresolved_parts: false,
                            },
                        )
                    }),
                    &identities,
                );
                types.add_external(&externals, &identities);
                let files = paths.map(|path| (path.to_path_buf(), &program));
                let signatures =
                    SignatureIndex::build(&files, Some(&externals), &identities, &types);
                let mut resolved = Vec::with_capacity(apps.len());
                for (app, copy) in apps.iter().zip(["getter", "method"]) {
                    let expected_path = canonical_or_lexical(
                        &dir.path().join(format!("packages/{copy}/lib/foo.dart")),
                    );
                    let model = SemanticModel::new(app, &identities, Some(&types));
                    let identity = model.resolve_name("Foo").unwrap();
                    assert!(identity.is_package("foo", "Foo"));
                    assert!(identity.matches(&DeclarationIdentity::package("foo", "Foo")));
                    let receiver = ResolvedType::interface_identity(identity.clone());
                    assert_eq!(
                        signatures.is_subtype_of(
                            &receiver,
                            &DeclarationIdentity::package("foo", "Foo"),
                            &model
                        ),
                        falcon_analyze::Truth::Yes
                    );
                    for index in [&identities, base.as_ref()] {
                        assert_eq!(
                            index.declaration_library(&identity),
                            Some(index.library(index.file(&expected_path).unwrap())),
                            "order={order:?}, incremental={incremental}",
                        );
                        for selector in [
                            MemberSelector::DECLARATION,
                            MemberSelector::new(MemberSelectorKind::Read, false, "value"),
                        ] {
                            let (path, annotations) = index
                                .source_annotations(&identity, selector.into())
                                .unwrap();
                            assert_eq!(path, expected_path);
                            assert_eq!(annotations.len(), 1);
                        }
                    }
                    if copy == "getter" {
                        assert_eq!(
                            types.member_lookup_identity(&identity, "value"),
                            MemberResult::Found(MemberKind::Getter)
                        );
                        let (field, _) = signatures
                            .resolved_field(&receiver, "value", &model)
                            .unwrap();
                        assert!(field.interface("dart:core", "int"));
                    } else {
                        assert_eq!(
                            types.member_lookup_identity(&identity, "value"),
                            MemberResult::Found(MemberKind::Method)
                        );
                        let (signature, _) = signatures
                            .resolved_member(&receiver, "value", &model)
                            .unwrap();
                        assert_eq!(signature.return_type, receiver);
                        assert!(
                            signatures
                                .resolved_field(&receiver, "value", &model)
                                .is_none()
                        );
                    }
                    resolved.push(identity);
                }
                assert_ne!(resolved[0], resolved[1]);
            }
        }
    }
}

#[test]
fn shared_dependency_resolves_divergent_transitive_copies_in_every_order() {
    assert_shared_dependency_instances(true, false);
}

#[test]
fn shared_dependency_without_divergent_imports_keeps_one_copyless_instance() {
    assert_shared_dependency_instances(false, false);
}

#[test]
fn shared_dependency_cycle_resolves_three_contexts_with_equivalent_nonprimary_instances() {
    assert_shared_dependency_instances(true, true);
}

fn assert_shared_dependency_instances(reaches_foo: bool, through_cycle: bool) {
    let (dir, _) = workspace();
    let flutter = canonical_or_lexical(&dir.path().join("flutter"));
    let bar = dir.path().join("packages/bar");
    let bridge = dir.path().join("packages/baz");
    if through_cycle {
        write(
            &bridge,
            "lib/baz.dart",
            "export 'package:foo/foo.dart';\nexport 'package:bar/bar.dart';",
        );
    }
    let bar_path = canonical_or_lexical(&write(
        &bar,
        "lib/bar.dart",
        if through_cycle {
            "import 'package:baz/baz.dart';\n\
             export 'package:baz/baz.dart';\n\
             Foo makeValue() => Foo();"
        } else if reaches_foo {
            "import 'package:foo/foo.dart';\n\
             export 'package:foo/foo.dart';\n\
             Foo makeValue() => Foo();"
        } else {
            "export 'src/shared.dart';\n\
             import 'src/shared.dart';\n\
             Shared makeValue() => Shared();"
        },
    ));
    if !reaches_foo {
        write(&bar, "lib/src/shared.dart", "class Shared {}");
    }
    let source = "import 'package:foo/foo.dart';\nimport 'package:bar/bar.dart';";
    let apps = [
        ("first", "first"),
        ("second", "second"),
        ("third", "second"),
    ]
    .map(|(name, copy)| {
        let package_root = dir.path().join(format!("packages/{copy}/foo"));
        write(&package_root, "lib/foo.dart", "class Foo {}");
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                    {{"name": "foo", "rootUri": "file://{}", "packageUri": "lib/"}},
                    {{"name": "bar", "rootUri": "file://{}", "packageUri": "lib/"}},
                    {{"name": "baz", "rootUri": "file://{}", "packageUri": "lib/"}}
                ]}}"#,
                flutter.display(),
                canonical_or_lexical(&package_root).display(),
                canonical_or_lexical(&bar).display(),
                canonical_or_lexical(&bridge).display(),
            ),
        );
        canonical_or_lexical(&write(dir.path(), &format!("{name}/lib/main.dart"), source))
    });
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");

    for order in [[0, 1, 2], [2, 1, 0]] {
        let paths = order.map(|index| apps[index].as_path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, fixture_sdk);
        let roots = paths.map(|path| (path, &program, source));
        let is_project = |path: &Path| apps.iter().any(|app| app == path);
        for incremental in [false, true] {
            let externals = if incremental {
                let mut externals = ExternalLibraries::load(&environment, &roots[..1], is_project);
                externals.extend(&environment, &roots[1..], is_project);
                externals
            } else {
                ExternalLibraries::load(&environment, &roots, is_project)
            };
            let bar_instances: Vec<_> = externals
                .libraries()
                .iter()
                .filter(|library| library.files[0].path == bar_path)
                .collect();
            assert_eq!(
                bar_instances.len(),
                if reaches_foo { 2 } else { 1 },
                "order={order:?}, incremental={incremental}",
            );
            if !reaches_foo {
                let defining_paths: std::collections::BTreeSet<_> = externals
                    .libraries()
                    .iter()
                    .map(|library| &library.files[0].path)
                    .collect();
                assert_eq!(externals.libraries().len(), defining_paths.len());
            }
            let sources = paths.map(|path| IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            });
            let external = ExternalSources {
                libraries: &externals,
                environment: &environment,
            };
            let base = std::sync::Arc::new(IdentityIndex::build_external(external));
            for identities in [
                IdentityIndex::build(&sources, &[source; 3], &[], Some(external)),
                IdentityIndex::with_external(&sources, &[source; 3], &[], external, base.clone()),
            ] {
                let mut types = TypeIndex::from_resolved_files(
                    paths.map(|path| {
                        (
                            path,
                            LibrarySource {
                                program: &program,
                                has_parse_errors: false,
                                has_unresolved_parts: false,
                            },
                        )
                    }),
                    &identities,
                );
                types.add_external(&externals, &identities);
                let files = paths.map(|path| (path.to_path_buf(), &program));
                let signatures =
                    SignatureIndex::build(&files, Some(&externals), &identities, &types);
                let mut foos = Vec::with_capacity(apps.len());
                let mut functions = Vec::with_capacity(apps.len());
                for (app, name) in apps.iter().zip(["first", "second", "second"]) {
                    let model = SemanticModel::new(app, &identities, Some(&types));
                    let foo_symbol = model.resolve_name("Foo").unwrap();
                    assert!(foo_symbol.is_package("foo", "Foo"));
                    let expected_path = canonical_or_lexical(
                        &dir.path().join(format!("packages/{name}/foo/lib/foo.dart")),
                    );
                    for index in [&identities, base.as_ref()] {
                        assert_eq!(
                            index.declaration_library(&foo_symbol),
                            Some(index.library(index.file(&expected_path).unwrap())),
                            "order={order:?}, incremental={incremental}",
                        );
                    }
                    let function = model.resolve_value("makeValue").unwrap();
                    assert!(function.is_package("bar", "makeValue"));
                    let returned = if reaches_foo {
                        foo_symbol.clone()
                    } else {
                        let shared = model.resolve_name("Shared").unwrap();
                        assert_eq!(shared, DeclarationIdentity::package("bar", "Shared"));
                        assert_eq!(function, DeclarationIdentity::package("bar", "makeValue"));
                        shared
                    };
                    assert_eq!(
                        signatures.function(&function).unwrap().return_type,
                        ResolvedType::interface_identity(returned),
                        "order={order:?}, incremental={incremental}",
                    );
                    foos.push(foo_symbol);
                    functions.push(function);
                }
                assert_ne!(foos[0], foos[1]);
                assert_eq!(foos[1], foos[2]);
                assert_eq!(functions[1], functions[2]);
                assert_eq!(functions[0] == functions[1], !reaches_foo);
            }
        }
    }
}

#[test]
fn contextless_source_keeps_separate_project_and_package_identities() {
    assert_contextless_shared_dependency(false, false);
}

#[test]
fn contextless_divergent_dependency_does_not_collide_with_configured_package() {
    assert_contextless_shared_dependency(true, false);
}

#[test]
fn configured_context_omitting_shared_package_keeps_project_identity() {
    assert_contextless_shared_dependency(false, true);
}

fn assert_contextless_shared_dependency(divergent: bool, configured_outside: bool) {
    let (dir, _) = workspace();
    let flutter = canonical_or_lexical(&dir.path().join("flutter"));
    let bar = dir.path().join("packages/bar");
    let foo_root = dir.path().join("packages/foo");
    write(&foo_root, "lib/foo.dart", "class Foo {}");
    let bar_path = canonical_or_lexical(&write(
        &bar,
        "lib/bar.dart",
        if divergent {
            "import 'package:foo/foo.dart';\nclass Shared extends Foo {}"
        } else {
            "class Shared {}"
        },
    ));
    let mut paths = vec![canonical_or_lexical(&write(
        dir.path(),
        "outside/main.dart",
        "import '../packages/bar/lib/bar.dart';",
    ))];
    let mut texts = vec!["import '../packages/bar/lib/bar.dart';"];
    if configured_outside {
        write(
            dir.path(),
            "outside/.dart_tool/package_config.json",
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": []}}"#,
                flutter.display(),
            ),
        );
    }
    for name in ["first", "second"]
        .into_iter()
        .take(if divergent || configured_outside {
            1
        } else {
            2
        })
    {
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                    {{"name": "bar", "rootUri": "file://{}", "packageUri": "lib/"}},
                    {{"name": "foo", "rootUri": "file://{}", "packageUri": "lib/"}}
                ]}}"#,
                flutter.display(),
                canonical_or_lexical(&bar).display(),
                canonical_or_lexical(&foo_root).display(),
            ),
        );
        paths.push(canonical_or_lexical(&write(
            dir.path(),
            &format!("{name}/lib/main.dart"),
            "import 'package:bar/bar.dart';",
        )));
        texts.push("import 'package:bar/bar.dart';");
    }
    let programs: Vec<_> = texts
        .iter()
        .map(|text| {
            let falcon_dart_parser::ParseResult {
                program, errors, ..
            } = parse(text);
            assert!(errors.is_empty(), "{errors:?}");
            program
        })
        .collect();
    let orders = if divergent || configured_outside {
        vec![vec![0, 1], vec![1, 0]]
    } else {
        vec![vec![0, 1, 2], vec![2, 1, 0], vec![1, 0, 2]]
    };
    for order in orders {
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            order.iter().map(|&index| paths[index].as_path()),
            fixture_sdk,
        );
        let roots: Vec<_> = order
            .iter()
            .map(|&index| (paths[index].as_path(), &programs[index], texts[index]))
            .collect();
        let is_project = |path: &Path| paths.iter().any(|project| project == path);
        for incremental in [false, true] {
            let externals = if incremental {
                let mut externals = ExternalLibraries::load(&environment, &roots[..1], is_project);
                for root in &roots[1..] {
                    externals.extend(&environment, std::slice::from_ref(root), is_project);
                }
                externals
            } else {
                ExternalLibraries::load(&environment, &roots, is_project)
            };
            assert_eq!(
                externals
                    .libraries()
                    .iter()
                    .filter(|library| library.files[0].path == bar_path)
                    .count(),
                2,
                "order={order:?}, incremental={incremental}",
            );
            let sources: Vec<_> = order
                .iter()
                .map(|&index| IdentitySource {
                    path: &paths[index],
                    program: &programs[index],
                    has_parse_errors: false,
                })
                .collect();
            let content: Vec<_> = order.iter().map(|&index| texts[index]).collect();
            let external = ExternalSources {
                libraries: &externals,
                environment: &environment,
            };
            let base = std::sync::Arc::new(IdentityIndex::build_external(external));
            for identities in [
                IdentityIndex::build(&sources, &content, &[], Some(external)),
                IdentityIndex::with_external(&sources, &content, &[], external, base.clone()),
            ] {
                for path in paths.iter().skip(1) {
                    let model = SemanticModel::new(path, &identities, None);
                    let identity = model.resolve_name("Shared").unwrap();
                    assert_eq!(
                        identity,
                        DeclarationIdentity::package("bar", "Shared"),
                        "order={order:?}, incremental={incremental}, path={path:?}",
                    );
                    for index in [&identities, base.as_ref()] {
                        let library = index.declaration_library(&identity).unwrap();
                        assert_eq!(index.library_uri(library), Some("package:bar/bar.dart"));
                    }
                }
                let outside = SemanticModel::new(&paths[0], &identities, None);
                let identity = outside.resolve_name("Shared").unwrap();
                assert!(matches!(&identity, DeclarationIdentity::Project { .. }));
                let library = identities.declaration_library(&identity).unwrap();
                assert_eq!(identities.library_uri(library), None);
            }
        }
    }
}

#[test]
fn different_entrypoints_of_one_nondivergent_package_keep_copyless_identities() {
    let (dir, _) = workspace();
    let flutter = canonical_or_lexical(&dir.path().join("flutter"));
    let package_root = dir.path().join("packages/shared");
    write(&package_root, "lib/common.dart", "class Shared {}");
    let texts = [
        "import 'package:shared/first.dart';",
        "import 'package:shared/second.dart';",
    ];
    let apps = ["first", "second"].map(|name| {
        write(
            &package_root,
            &format!("lib/{name}.dart"),
            &format!("export 'common.dart';\nclass Entry{name} {{}}"),
        );
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                    {{"name": "shared", "rootUri": "file://{}", "packageUri": "lib/"}}
                ]}}"#,
                flutter.display(),
                canonical_or_lexical(&package_root).display(),
            ),
        );
        let index = usize::from(name == "second");
        canonical_or_lexical(&write(
            dir.path(),
            &format!("{name}/lib/main.dart"),
            texts[index],
        ))
    });
    let programs = texts.map(|text| {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(text);
        assert!(errors.is_empty(), "{errors:?}");
        program
    });
    for order in [[0, 1], [1, 0]] {
        let paths = order.map(|index| apps[index].as_path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, fixture_sdk);
        let roots = order.map(|index| (apps[index].as_path(), &programs[index], texts[index]));
        let is_project = |path: &Path| apps.iter().any(|app| app == path);
        for incremental in [false, true] {
            let externals = if incremental {
                let mut externals = ExternalLibraries::load(&environment, &roots[..1], is_project);
                externals.extend(&environment, &roots[1..], is_project);
                externals
            } else {
                ExternalLibraries::load(&environment, &roots, is_project)
            };
            let sources = order.map(|index| IdentitySource {
                path: &apps[index],
                program: &programs[index],
                has_parse_errors: false,
            });
            let content = order.map(|index| texts[index]);
            let external = ExternalSources {
                libraries: &externals,
                environment: &environment,
            };
            let base = std::sync::Arc::new(IdentityIndex::build_external(external));
            for identities in [
                IdentityIndex::build(&sources, &content, &[], Some(external)),
                IdentityIndex::with_external(&sources, &content, &[], external, base),
            ] {
                for (app, name) in apps.iter().zip(["Entryfirst", "Entrysecond"]) {
                    let model = SemanticModel::new(app, &identities, None);
                    assert_eq!(
                        model.resolve_name("Shared"),
                        Some(DeclarationIdentity::package("shared", "Shared")),
                        "order={order:?}, incremental={incremental}",
                    );
                    assert_eq!(
                        model.resolve_name(name),
                        Some(DeclarationIdentity::package("shared", name)),
                        "order={order:?}, incremental={incremental}",
                    );
                }
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn extending_external_libraries_observes_changed_symlink_targets() {
    let directory = tempfile::tempdir().unwrap();
    let first = write(directory.path(), "first.dart", "class First {}");
    let second = write(directory.path(), "second.dart", "class Second {}");
    let link = directory.path().join("dependency.dart");
    std::os::unix::fs::symlink(&first, &link).unwrap();
    let source = "import 'dependency.dart';";
    let main = canonical_or_lexical(&write(directory.path(), "main.dart", source));
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let environment = AnalysisEnvironment::discover_with_sdk_locator([main.as_path()], |_| None);
    let project = [(main.as_path(), &parsed.program, source)];
    let mut libraries = ExternalLibraries::load(&environment, &project, |_| false);
    let target_path = |libraries: &ExternalLibraries| match libraries
        .resolve_target(&environment, &main, None, "dependency.dart")
        .unwrap()
    {
        falcon_analyze::environment::ExternalTarget::Instance(index) => {
            libraries.libraries()[index].files[0].path.clone()
        }
        falcon_analyze::environment::ExternalTarget::Path(path) => path,
    };
    assert_eq!(target_path(&libraries), canonical_or_lexical(&first));
    fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&second, &link).unwrap();
    libraries.extend(&environment, &project, |_| false);
    assert_eq!(target_path(&libraries), canonical_or_lexical(&second));
}

#[test]
fn documentation_imports_split_shared_dependency_instances() {
    let (dir, _) = workspace();
    let flutter = canonical_or_lexical(&dir.path().join("flutter"));
    let shared_root = dir.path().join("packages/bar");
    let shared_path = canonical_or_lexical(&write(
        &shared_root,
        "lib/bar.dart",
        "/// @docImport 'package:foo/foo.dart';\nlibrary;\nclass Bar {}",
    ));
    let source = "import 'package:bar/bar.dart';";
    let apps = ["first", "second"].map(|name| {
        let package_root = dir.path().join(format!("packages/{name}/foo"));
        write(&package_root, "lib/foo.dart", "class Foo {}");
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                        {{"name": "foo", "rootUri": "file://{}", "packageUri": "lib/"}},
                        {{"name": "bar", "rootUri": "file://{}", "packageUri": "lib/"}}
                    ]}}"#,
                flutter.display(),
                canonical_or_lexical(&package_root).display(),
                canonical_or_lexical(&shared_root).display(),
            ),
        );
        canonical_or_lexical(&write(dir.path(), &format!("{name}/lib/main.dart"), source))
    });
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for order in [[0, 1], [1, 0]] {
        let paths = order.map(|index| apps[index].as_path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, fixture_sdk);
        let roots = paths.map(|path| (path, &program, source));
        let is_project = |path: &Path| apps.iter().any(|app| app == path);
        for incremental in [false, true] {
            let externals = if incremental {
                let mut externals = ExternalLibraries::load(&environment, &roots[..1], is_project);
                externals.extend(&environment, &roots[1..], is_project);
                externals
            } else {
                ExternalLibraries::load(&environment, &roots, is_project)
            };
            assert_eq!(
                externals
                    .libraries()
                    .iter()
                    .filter(|library| library.files[0].path == shared_path)
                    .count(),
                2,
                "order={order:?}, incremental={incremental}",
            );
            let sources = paths.map(|path| IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            });
            let external = ExternalSources {
                libraries: &externals,
                environment: &environment,
            };
            let base = std::sync::Arc::new(IdentityIndex::build_external(external));
            for identities in [
                IdentityIndex::build(&sources, &[source; 2], &[], Some(external)),
                IdentityIndex::with_external(&sources, &[source; 2], &[], external, base),
            ] {
                let symbols = apps.each_ref().map(|app| {
                    SemanticModel::new(app, &identities, None)
                        .resolve_name("Bar")
                        .unwrap()
                });
                assert_ne!(
                    symbols[0], symbols[1],
                    "order={order:?}, incremental={incremental}",
                );
            }
        }
    }
}

#[test]
fn package_copies_keep_package_identities_when_a_declaration_moves_between_versions() {
    let (dir, _) = workspace();
    let flutter = canonical_or_lexical(&dir.path().join("flutter"));
    let source = "import 'package:foo/foo.dart';";
    let apps = [
        ("old", "class Foo {}", None),
        ("new", "export 'src/foo.dart';", Some("class Foo {}")),
    ]
    .map(|(name, library, moved)| {
        let package = dir.path().join(format!("packages/{name}"));
        write(&package, "lib/foo.dart", library);
        if let Some(moved) = moved {
            write(&package, "lib/src/foo.dart", moved);
        }
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                    {{"name": "foo", "rootUri": "file://{}", "packageUri": "lib/"}}
                ]}}"#,
                flutter.display(),
                canonical_or_lexical(&package).display(),
            ),
        );
        canonical_or_lexical(&write(dir.path(), &format!("{name}/lib/main.dart"), source))
    });
    let falcon_dart_parser::ParseResult { program, .. } = parse(source);
    let paths = apps.each_ref().map(PathBuf::as_path);
    let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, fixture_sdk);
    let roots = paths.map(|path| (path, &program, source));
    let externals = ExternalLibraries::load(&environment, &roots, |path| paths.contains(&path));
    let sources = paths.map(|path| IdentitySource {
        path,
        program: &program,
        has_parse_errors: false,
    });
    let identities = IdentityIndex::build(
        &sources,
        &[source, source],
        &[],
        Some(ExternalSources {
            libraries: &externals,
            environment: &environment,
        }),
    );
    let [old, new] = paths.map(|path| {
        SemanticModel::new(path, &identities, None)
            .resolve_name("Foo")
            .unwrap()
    });
    assert!(old.is_package("foo", "Foo"), "{old:?}");
    assert!(new.is_package("foo", "Foo"), "{new:?}");
    assert_ne!(old, new);
}

#[test]
fn duplicate_sdk_copies_choose_the_same_physical_authority_in_every_load_order() {
    use falcon_analyze::{MemberKind, MemberResult};

    let (dir, first) = workspace();
    write(
        dir.path(),
        "flutter/bin/cache/dart-sdk/lib/core/core.dart",
        "class Object {} class int {} class SdkType { int get value => 1; }",
    );
    write(
        dir.path(),
        "z_flutter/bin/cache/dart-sdk/lib/libraries.json",
        r#"{"vm": {"libraries": {"core": {"uri": "core/core.dart"}}}}"#,
    );
    write(
        dir.path(),
        "z_flutter/bin/cache/dart-sdk/lib/core/core.dart",
        "class Object {} class int {} class SdkType { SdkType value() => this; }",
    );
    let flutter = canonical_or_lexical(&dir.path().join("z_flutter"));
    write(
        dir.path(),
        "second/.dart_tool/package_config.json",
        &format!(
            r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": []}}"#,
            flutter.display(),
        ),
    );
    let source = "import 'dart:core';";
    let second = canonical_or_lexical(&write(dir.path(), "second/lib/main.dart", source));
    let apps = [first, second];
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");

    for order in [[0, 1], [1, 0]] {
        let paths = order.map(|index| apps[index].as_path());
        let environment = AnalysisEnvironment::discover_with_sdk_locator(paths, fixture_sdk);
        let roots = paths.map(|path| (path, &program, source));
        let is_project = |path: &Path| apps.iter().any(|app| app == path);
        for incremental in [false, true] {
            let externals = if incremental {
                let mut externals = ExternalLibraries::load(&environment, &roots[..1], is_project);
                externals.extend(&environment, &roots[1..], is_project);
                externals
            } else {
                ExternalLibraries::load(&environment, &roots, is_project)
            };
            assert_eq!(
                externals
                    .libraries()
                    .iter()
                    .filter(|library| library.origin == ExternalOrigin::Sdk("dart:core".into()))
                    .count(),
                2
            );
            let sources = paths.map(|path| IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            });
            let external = ExternalSources {
                libraries: &externals,
                environment: &environment,
            };
            let base = std::sync::Arc::new(IdentityIndex::build_external(external));
            for identities in [
                IdentityIndex::build(&sources, &[source, source], &[], Some(external)),
                IdentityIndex::with_external(&sources, &[source, source], &[], external, base),
            ] {
                let mut types = TypeIndex::from_resolved_files(
                    paths.map(|path| {
                        (
                            path,
                            LibrarySource {
                                program: &program,
                                has_parse_errors: false,
                                has_unresolved_parts: false,
                            },
                        )
                    }),
                    &identities,
                );
                types.add_external(&externals, &identities);
                let files = paths.map(|path| (path.to_path_buf(), &program));
                let signatures =
                    SignatureIndex::build(&files, Some(&externals), &identities, &types);
                for app in &apps {
                    let model = SemanticModel::new(app, &identities, Some(&types));
                    let identity = model.resolve_name("SdkType").unwrap();
                    assert!(identity.is_sdk("dart:core", "SdkType"));
                    let expected_path = canonical_or_lexical(
                        &dir.path()
                            .join("z_flutter/bin/cache/dart-sdk/lib/core/core.dart"),
                    );
                    assert_eq!(
                        identities.declaration_library(&identity),
                        Some(identities.library(identities.file(&expected_path).unwrap())),
                    );
                    assert_eq!(
                        types.member_lookup_identity(&identity, "value"),
                        MemberResult::Found(MemberKind::Method),
                        "order={order:?}, incremental={incremental}",
                    );
                    let receiver = ResolvedType::interface_identity(identity);
                    let (signature, _) = signatures
                        .resolved_member(&receiver, "value", &model)
                        .unwrap();
                    assert_eq!(signature.return_type, receiver);
                    assert!(signature.positional.is_empty());
                    assert!(
                        signatures
                            .resolved_field(&receiver, "value", &model)
                            .is_none()
                    );
                }
            }
        }
    }
}

#[test]
fn shared_dependency_keeps_first_loaded_sdk_types_in_signatures() {
    for divergent in [false, true] {
        assert_shared_dependency_sdk_types(divergent);
    }
}

fn assert_shared_dependency_sdk_types(divergent: bool) {
    let dir = tempfile::tempdir().unwrap();
    let shared_root = dir.path().join("packages/shared");
    let shared_path = canonical_or_lexical(&write(
        &shared_root,
        "lib/shared.dart",
        if divergent {
            "import 'dart:io';\nimport 'package:foo/foo.dart';\nIoType makeIo() => IoType();"
        } else {
            "import 'dart:io';\nIoType makeIo() => IoType();"
        },
    ));
    let source = "import 'package:shared/shared.dart';";
    let apps = ["first", "second"].map(|name| {
        let flutter = dir.path().join(format!("{name}_flutter"));
        let foo_root = dir.path().join(format!("packages/{name}/foo"));
        write(&foo_root, "lib/foo.dart", "class Foo {}");
        write(
            &flutter,
            "bin/cache/dart-sdk/lib/libraries.json",
            r#"{"vm": {"libraries": {
                "core": {"uri": "core/core.dart"},
                "io": {"uri": "io/io.dart"}
            }}}"#,
        );
        write(
            &flutter,
            "bin/cache/dart-sdk/lib/core/core.dart",
            "class Object {}",
        );
        write(
            &flutter,
            "bin/cache/dart-sdk/lib/io/io.dart",
            &format!("library dart.io;\nclass IoType {{ void {name}Only() {{}} }}"),
        );
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                    {{"name": "shared", "rootUri": "file://{}", "packageUri": "lib/"}},
                    {{"name": "foo", "rootUri": "file://{}", "packageUri": "lib/"}}
                ]}}"#,
                canonical_or_lexical(&flutter).display(),
                canonical_or_lexical(&shared_root).display(),
                canonical_or_lexical(&foo_root).display(),
            ),
        );
        canonical_or_lexical(&write(dir.path(), &format!("{name}/lib/main.dart"), source))
    });
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let environment = AnalysisEnvironment::discover_with_sdk_locator(
        apps.iter().map(PathBuf::as_path),
        fixture_sdk,
    );
    for order in [[0, 1], [1, 0]] {
        let paths = order.map(|index| apps[index].as_path());
        let roots = paths.map(|path| (path, &program, source));
        let is_project = |path: &Path| apps.iter().any(|app| app == path);
        for incremental in [false, true] {
            let externals = if incremental {
                let mut externals = ExternalLibraries::load(&environment, &roots[..1], is_project);
                externals.extend(&environment, &roots[1..], is_project);
                externals
            } else {
                ExternalLibraries::load(&environment, &roots, is_project)
            };
            assert_eq!(
                externals
                    .libraries()
                    .iter()
                    .filter(|library| library.files[0].path == shared_path)
                    .count(),
                if divergent { 2 } else { 1 },
            );
            let io_path = environment
                .sdk(paths[0])
                .unwrap()
                .library("dart:io")
                .unwrap();
            let io_libraries: Vec<_> = externals
                .libraries()
                .iter()
                .filter(|library| library.origin == ExternalOrigin::Sdk("dart:io".into()))
                .collect();
            assert_eq!(io_libraries.len(), 1);
            assert_eq!(io_libraries[0].files[0].path.as_path(), io_path);
            let sources = paths.map(|path| IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            });
            let external = ExternalSources {
                libraries: &externals,
                environment: &environment,
            };
            let base = std::sync::Arc::new(IdentityIndex::build_external(external));
            for identities in [
                IdentityIndex::build(&sources, &[source; 2], &[], Some(external)),
                IdentityIndex::with_external(&sources, &[source; 2], &[], external, base.clone()),
            ] {
                let mut types = TypeIndex::from_resolved_files(
                    paths.map(|path| {
                        (
                            path,
                            LibrarySource {
                                program: &program,
                                has_parse_errors: false,
                                has_unresolved_parts: false,
                            },
                        )
                    }),
                    &identities,
                );
                types.add_external(&externals, &identities);
                let files = paths.map(|path| (path.to_path_buf(), &program));
                let signatures =
                    SignatureIndex::build(&files, Some(&externals), &identities, &types);
                let shared = SemanticModel::new(&shared_path, &identities, Some(&types));
                let io_type = shared.resolve_name("IoType").unwrap();
                assert!(io_type.is_sdk("dart:io", "IoType"));
                for index in [&identities, base.as_ref()] {
                    assert_eq!(
                        index.declaration_library(&io_type),
                        Some(index.library(index.file(io_path).unwrap())),
                        "order={order:?}, incremental={incremental}",
                    );
                }
                let mut functions = Vec::with_capacity(apps.len());
                for app in &apps {
                    let model = SemanticModel::new(app, &identities, Some(&types));
                    let function = model.resolve_value("makeIo").unwrap();
                    assert!(function.is_package("shared", "makeIo"));
                    assert_eq!(
                        signatures.function(&function).unwrap().return_type,
                        ResolvedType::interface_identity(io_type.clone()),
                        "order={order:?}, incremental={incremental}",
                    );
                    functions.push(function);
                }
                assert_eq!(functions[0] == functions[1], !divergent);
            }
        }
    }
}

#[test]
fn shared_dependency_doc_imports_use_first_loaded_sdk_availability() {
    use falcon_analyze::{DocName, NameIdentity};

    let dir = tempfile::tempdir().unwrap();
    let shared_root = dir.path().join(".pub-cache/hosted/pub.dev/shared-1.0.0");
    let shared_path = canonical_or_lexical(&write(
        &shared_root,
        "lib/shared.dart",
        "/// @docImport 'dart:io';\nlibrary;\nclass Shared {}",
    ));
    let source = "import 'package:shared/shared.dart';";
    let apps = ["present", "absent"].map(|name| {
        let flutter = dir.path().join(format!("{name}_flutter"));
        write(
            &flutter,
            "bin/cache/dart-sdk/lib/libraries.json",
            if name == "present" {
                r#"{"vm": {"libraries": {
                    "core": {"uri": "core/core.dart"},
                    "io": {"uri": "io/io.dart"}
                }}}"#
            } else {
                r#"{"vm": {"libraries": {"core": {"uri": "core/core.dart"}}}}"#
            },
        );
        write(
            &flutter,
            "bin/cache/dart-sdk/lib/core/core.dart",
            "class Object {}",
        );
        if name == "present" {
            write(
                &flutter,
                "bin/cache/dart-sdk/lib/io/io.dart",
                "library dart.io;\nclass File {}",
            );
        }
        write(
            dir.path(),
            &format!("{name}/.dart_tool/package_config.json"),
            &format!(
                r#"{{"configVersion": 2, "flutterRoot": "file://{}", "packages": [
                    {{"name": "shared", "rootUri": "file://{}", "packageUri": "lib/"}}
                ]}}"#,
                canonical_or_lexical(&flutter).display(),
                canonical_or_lexical(&shared_root).display(),
            ),
        );
        canonical_or_lexical(&write(dir.path(), &format!("{name}/lib/main.dart"), source))
    });
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for discovery_order in [[0, 1], [1, 0]] {
        let environment = AnalysisEnvironment::discover_with_sdk_locator(
            discovery_order.map(|index| apps[index].as_path()),
            fixture_sdk,
        );
        for load_order in [[0, 1], [1, 0]] {
            let paths = load_order.map(|index| apps[index].as_path());
            let roots = paths.map(|path| (path, &program, source));
            let is_project = |path: &Path| apps.iter().any(|app| app == path);
            for incremental in [false, true] {
                let externals = if incremental {
                    let mut externals =
                        ExternalLibraries::load(&environment, &roots[..1], is_project);
                    externals.extend(&environment, &roots[1..], is_project);
                    externals
                } else {
                    ExternalLibraries::load(&environment, &roots, is_project)
                };
                let sources = paths.map(|path| IdentitySource {
                    path,
                    program: &program,
                    has_parse_errors: false,
                });
                let external = ExternalSources {
                    libraries: &externals,
                    environment: &environment,
                };
                let base = std::sync::Arc::new(IdentityIndex::build_external(external));
                for identities in [
                    IdentityIndex::build(&sources, &[source; 2], &[], Some(external)),
                    IdentityIndex::with_external(
                        &sources,
                        &[source; 2],
                        &[],
                        external,
                        base.clone(),
                    ),
                ] {
                    let library = identities.library(identities.file(&shared_path).unwrap());
                    assert_eq!(identities.library_name(library, "File"), DocName::Absent);
                    assert_eq!(
                        identities.doc_name(library, "File"),
                        if load_order[0] == 0 {
                            DocName::Declaration {
                                identity: Some(DeclarationIdentity::sdk("dart:io", "File")),
                                kind: NameIdentity::Type,
                            }
                        } else {
                            DocName::Absent
                        },
                        "discovery={discovery_order:?}, load={load_order:?}, incremental={incremental}",
                    );
                    if load_order[0] == 0 {
                        let identity = DeclarationIdentity::sdk("dart:io", "File");
                        let io_path = environment
                            .sdk(paths[0])
                            .unwrap()
                            .library("dart:io")
                            .unwrap();
                        assert_eq!(
                            identities.declaration_library(&identity),
                            Some(identities.library(identities.file(io_path).unwrap())),
                        );
                    }
                }
            }
        }
    }
}
