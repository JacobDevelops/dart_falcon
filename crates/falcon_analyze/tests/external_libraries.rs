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
        let (program, errors) = parse(source);
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
    let (program, errors) = parse(&source);
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
    let (program, errors) = parse(source);
    assert!(errors.is_empty());
    let (other_program, errors) = parse(other_source);
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
    let (program, errors) = parse(source);
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
    let (program, errors) = parse(source);
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
    let (program, errors) = parse(source);
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
    let (_, errors) = parse(&dep_source);
    assert!(!errors.is_empty());
    let source = fs::read_to_string(&app).unwrap();
    let (program, errors) = parse(&source);
    assert!(errors.is_empty());
    let environment = AnalysisEnvironment::discover_with_sdk_locator([app.as_path()], fixture_sdk);
    let roots = [(app.as_path(), &program, source.as_str())];
    let externals = ExternalLibraries::load(&environment, &roots, |path| path == app);
    let file = externals
        .libraries()
        .iter()
        .flat_map(|library| &library.files)
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
        .flat_map(|library| &library.files)
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
    let (program, errors) = parse(&source);
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
    let (program, errors) = parse(source);
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

    let ctx = falcon_analyze::AnalyzeContext::new(&file, "void main() {}", &config)
        .with_environment(Some(&environment));
    assert_eq!(ctx.package_language_version, Some(package_version));
    assert_eq!(ctx.language_version(), package_version);

    let source = "// @dart = 2.12\nvoid main() {}";
    let overridden = falcon_analyze::AnalyzeContext::new(&file, source, &config)
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
    let (program, errors) = parse(source);
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
    let expected = parse("const expected = 42;").0;
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
    let (program, errors) = parse(source);
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
    let (program, errors) = parse(source);
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
                .flat_map(|library| &library.files)
                .any(|file| file.path.ends_with(name)),
            "{name}"
        );
    }
    assert!(!environment.has_sdk());
}

#[test]
fn opening_external_files_evicts_the_disk_copy_and_allows_reloading_after_close() {
    let (dir, app) = workspace();
    let dependency = canonical_or_lexical(&dir.path().join("cache/dep/lib/dep.dart"));
    let source = fs::read_to_string(&app).unwrap();
    let (program, _) = parse(&source);
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
            .flat_map(|library| &library.files)
            .any(|file| file.path == dependency)
    );
    external.extend(&environment, &roots, |path| path == app);
    assert!(
        external
            .libraries()
            .iter()
            .flat_map(|library| &library.files)
            .any(|file| file.path == dependency)
    );
}

#[test]
fn external_snapshots_reload_dependency_edits_and_removal_explicitly() {
    let (dir, app) = workspace();
    let dependency = dir.path().join("cache/dep/lib/dep.dart");
    let source = fs::read_to_string(&app).unwrap();
    let (program, _) = parse(&source);
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
    let (program, _) = parse(&source);
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
    let (program, _) = parse(&source);
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
    let (program, _) = parse(source);
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
    let (program, _) = parse(&source);
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
    let part = write(
        dir.path(),
        "cache/dep/lib/leaf_part.dart",
        "part of leaf; class LeafPart {}",
    );
    let source = "import 'package:dep/bridge.dart';";
    let (program, errors) = parse(source);
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
                .flat_map(|library| &library.files)
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
                .flat_map(|library| &library.files)
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
        .flat_map(|library| &library.files)
        .flat_map(|file| &file.facts.declarations)
        .any(|declaration| {
            matches!(declaration,
            falcon_syntax::ast::TopLevelDecl::Class(class) if class.name.name == name)
        })
}
