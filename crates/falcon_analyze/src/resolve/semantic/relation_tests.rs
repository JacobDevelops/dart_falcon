use super::*;
use crate::resolve::IdentitySource;
use falcon_dart_parser::parse;

fn with_relations(source: &str, test: impl FnOnce(&mut SignatureIndex, &SemanticModel<'_>)) {
    super::tests::with_model_at(
        Path::new("/project/relations.dart"),
        source,
        |program, model| {
            let mut signatures = SignatureIndex::from_program(program, model);
            test(&mut signatures, model);
        },
    );
}

#[test]
fn declaration_relations_prove_complete_misses_and_ancestry() {
    with_relations(
        "class Base<T> {} class Child extends Base<int> {} class Other {}",
        |signatures, model| {
            let base = model.resolve_name("Base").unwrap();
            let child = model.resolve_type_name(&["Child"]).unwrap();
            let other = model.resolve_name("Other").unwrap();
            for (target, expected) in [(&base, Truth::Yes), (&other, Truth::No)] {
                assert_eq!(
                    signatures.relation(&child, TypeRelation::SubtypeOfDeclaration(target), model),
                    expected,
                );
            }
            assert_eq!(
                signatures.relation(
                    &child.with_nullable(true),
                    TypeRelation::SubtypeOfDeclaration(&base),
                    model,
                ),
                Truth::Yes,
            );
        },
    );
}

#[test]
fn declaration_relations_keep_missing_edges_and_cycles_unknown() {
    with_relations(
        "class Incomplete extends Missing {} class Target {} \
         class A extends B {} class B extends A {}",
        |signatures, model| {
            let target = model.resolve_name("Target").unwrap();
            for name in ["Incomplete", "A", "B"] {
                let ty = model.resolve_type_name(&[name]).unwrap();
                assert_eq!(
                    signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(&target), model),
                    Truth::Unknown,
                    "{name}",
                );
            }
            let a = model.resolve_type_name(&["A"]).unwrap();
            let b = model.resolve_name("B").unwrap();
            assert_eq!(
                signatures.relation(&a, TypeRelation::SubtypeOfDeclaration(&b), model),
                Truth::Unknown,
            );
            let identity = model.resolve_name("A").unwrap();
            assert_eq!(
                signatures.relation(&a, TypeRelation::SubtypeOfDeclaration(&identity), model),
                Truth::Unknown,
            );
        },
    );
}

#[test]
fn declaration_relations_preserve_non_interface_uncertainty_and_bounds() {
    with_relations(
        "class Base {} class Other {} typedef Fn = int Function(); typedef Rec = (int, String);",
        |signatures, model| {
            let base = model.resolve_name("Base").unwrap();
            for ty in [
                ResolvedType::UNKNOWN,
                ResolvedType::DYNAMIC,
                ResolvedType::VOID,
                ResolvedType::NEVER,
                ResolvedType::NULL,
                model.resolve_type_name(&["Fn"]).unwrap(),
                model.resolve_type_name(&["Rec"]).unwrap(),
            ] {
                assert_eq!(
                    signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(&base), model),
                    Truth::Unknown,
                );
            }
            let id = TypeParameterId {
                owner: ParameterOwner::Sdk("relation-test"),
                ordinal: 0,
                name: "T".into(),
            };
            let bound = Arc::new(ResolvedType::interface_identity(base.clone()));
            let parameter = ResolvedType {
                kind: ResolvedTypeKind::TypeParameter {
                    id: id.clone(),
                    bound: Arc::clone(&bound),
                    nullable: false,
                },
            };
            let non_null = ResolvedType {
                kind: ResolvedTypeKind::NonNullTypeParameter { id, bound },
            };
            let other = model.resolve_name("Other").unwrap();
            for ty in [&parameter, &non_null] {
                assert_eq!(
                    signatures.relation(ty, TypeRelation::SubtypeOfDeclaration(&base), model),
                    Truth::Yes,
                );
                assert_eq!(
                    signatures.relation(ty, TypeRelation::SubtypeOfDeclaration(&other), model),
                    Truth::No,
                );
            }
        },
    );
}

#[test]
fn facade_preserves_each_resolved_type_relation_mode() {
    with_relations(
        "class Base {} class Child extends Base {} \
         typedef Fn = int Function(); typedef Rec = (int, String);",
        |signatures, model| {
            let resolve = |name| model.resolve_type_name(&[name]).unwrap();
            let int = resolve("int");
            let cases = [
                (int.clone(), int.clone(), Truth::Yes, Truth::No, Truth::No),
                (
                    resolve("Child"),
                    resolve("Base"),
                    Truth::Yes,
                    Truth::No,
                    Truth::No,
                ),
                (
                    resolve("Fn"),
                    resolve("Fn"),
                    Truth::Yes,
                    Truth::No,
                    Truth::No,
                ),
                (
                    resolve("Rec"),
                    resolve("Rec"),
                    Truth::Yes,
                    Truth::No,
                    Truth::No,
                ),
                (
                    ResolvedType::NULL,
                    int.clone(),
                    Truth::No,
                    Truth::Unknown,
                    Truth::Yes,
                ),
                (
                    ResolvedType::UNKNOWN,
                    int,
                    Truth::Unknown,
                    Truth::Unknown,
                    Truth::Unknown,
                ),
            ];
            for (left, right, subtype, unrelated, runtime_unrelated) in cases {
                for (relation, expected) in [
                    (TypeRelation::SubtypeOf(&right), subtype),
                    (TypeRelation::UnrelatedTo(&right), unrelated),
                    (TypeRelation::RuntimeUnrelatedTo(&right), runtime_unrelated),
                ] {
                    assert_eq!(signatures.relation(&left, relation, model), expected);
                }
            }
        },
    );
}

#[test]
fn enum_missing_implicit_edge_prevents_negative_declaration_proof() {
    with_relations(
        "class Contract {} class Other {} enum E implements Contract { value }",
        |signatures, model| {
            let ty = model.resolve_type_name(&["E"]).unwrap();
            let identity = model.resolve_name("E").unwrap();
            let contract = model.resolve_name("Contract").unwrap();
            let other = model.resolve_name("Other").unwrap();
            let enum_identity = DeclarationIdentity::sdk("dart:core", "Enum");
            for (target, expected) in [
                (&contract, Truth::Yes),
                (&other, Truth::Unknown),
                (&enum_identity, Truth::Unknown),
            ] {
                assert_eq!(
                    signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(target), model),
                    expected,
                );
            }
            // Existing graph callers retain their historical completeness policy.
            assert_eq!(
                signatures.supertype_matches(&identity, &|ancestor| ancestor == &other),
                Truth::No,
            );
            // Supply the recorded implicit SDK branch to exercise the
            // completeness boundary independently of installed SDK sources.
            signatures
                .type_facts
                .get_mut(&identity)
                .unwrap()
                .supertypes
                .as_mut()
                .unwrap()
                .1
                .push(ResolvedType::interface_identity(enum_identity.clone()));
            signatures
                .type_facts
                .entry(enum_identity.clone())
                .or_default()
                .supertypes = Some((
                Vec::new(),
                vec![ResolvedType::interface_identity(
                    DeclarationIdentity::OBJECT,
                )],
            ));
            for (target, expected) in [(&enum_identity, Truth::Yes), (&other, Truth::No)] {
                assert_eq!(
                    signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(target), model),
                    expected,
                );
            }
        },
    );
}

#[test]
fn extension_declaration_relations_do_not_follow_runtime_representation() {
    with_relations(
        "class Other {} extension type Id(int value) {}",
        |signatures, model| {
            let ty = model.resolve_type_name(&["Id"]).unwrap();
            let identity = model.resolve_name("Id").unwrap();
            let other = model.resolve_name("Other").unwrap();
            let int = DeclarationIdentity::sdk("dart:core", "int");
            for (target, expected) in [
                (&identity, Truth::Yes),
                (&other, Truth::No),
                (&int, Truth::No),
            ] {
                assert_eq!(
                    signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(target), model),
                    expected,
                );
            }
        },
    );
}

#[test]
fn declaration_relations_follow_parts_and_complete_external_snapshots() {
    for part in [true, false] {
        let root_source = if part {
            "part 'base.dart'; class Other {}"
        } else {
            "import 'base.dart'; class Child extends Base {} class Other {}"
        };
        let base_source = if part {
            "part of 'app.dart'; class Base {} class Child extends Base {}"
        } else {
            "class Base {}"
        };
        let files: Vec<_> = [("app.dart", root_source), ("base.dart", base_source)]
            .map(|(name, source)| {
                let parsed = parse(source);
                assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
                (
                    Path::new("/project").join(name),
                    Arc::new(crate::ParsedFile {
                        source: source.into(),
                        tree: parsed.tree,
                    }),
                )
            })
            .into();
        let sources: Vec<_> = files
            .iter()
            .map(|(path, parsed)| IdentitySource {
                path,
                parsed,
                has_parse_errors: false,
            })
            .collect();
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let types = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    super::super::LibrarySource {
                        program: source.parsed.program(),
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let model = SemanticModel::new(&files[0].0, &files[0].1, &identities, Some(&types));
        let child = model.resolve_type_name(&["Child"]).unwrap();
        let base = model.resolve_name("Base").unwrap();
        let other = model.resolve_name("Other").unwrap();
        let signatures = if part {
            SignatureIndex::from_project_files(&files, &identities, &types)
        } else {
            let external = SignatureIndex::from_project_files(&files[1..], &identities, &types);
            SignatureIndex::with_external(&files[..1], &identities, &types, Arc::new(external))
        };
        for (target, expected) in [(&base, Truth::Yes), (&other, Truth::No)] {
            assert_eq!(
                signatures.relation(&child, TypeRelation::SubtypeOfDeclaration(target), &model),
                expected,
            );
        }
        if !part {
            let missing = SignatureIndex::with_external(
                &files[..1],
                &identities,
                &types,
                Arc::new(SignatureIndex::default()),
            );
            assert_eq!(
                missing.relation(&child, TypeRelation::SubtypeOfDeclaration(&other), &model),
                Truth::Unknown,
            );
        }
    }
}

#[test]
fn malformed_type_parameter_supertype_preserves_instantiated_positive() {
    with_relations(
        "class Base {} class C<T> extends T {}",
        |signatures, model| {
            let base = model.resolve_name("Base").unwrap();
            let identity = model.resolve_name("C").unwrap();
            let ty = ResolvedType {
                kind: ResolvedTypeKind::Interface {
                    occurrence: Default::default(),
                    identity: identity.clone(),
                    arguments: vec![ResolvedType::interface_identity(base.clone())].into(),
                    nullable: false,
                    extension_type: false,
                },
            };
            assert_eq!(
                signatures.supertype_matches(&identity, &|ancestor| ancestor.matches(&base)),
                Truth::Unknown,
            );
            assert_eq!(
                signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(&base), model),
                Truth::Yes,
            );
        },
    );
}

#[test]
fn declaration_negative_proofs_accept_complete_loaded_package_facts() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir_all(root.join(".dart_tool")).unwrap();
    std::fs::create_dir_all(root.join("lib")).unwrap();
    std::fs::create_dir_all(root.join("dependency/lib")).unwrap();
    std::fs::write(
        root.join(".dart_tool/package_config.json"),
        r#"{"configVersion":2,"packages":[{"name":"dependency","rootUri":"../dependency","packageUri":"lib/"}]}"#,
    )
    .unwrap();
    std::fs::write(root.join("dependency/lib/dependency.dart"), "class Base {}").unwrap();
    let source =
        "import 'package:dependency/dependency.dart'; class Child extends Base {} class Other {}";
    let path = root.join("lib/app.dart");
    std::fs::write(&path, source).unwrap();
    let path = crate::canonical_or_lexical(&path);
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = Arc::new(crate::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let program = parsed.program();
    let environment =
        crate::AnalysisEnvironment::discover_with_sdk_locator([path.as_path()], |_| None);
    let external = ExternalLibraries::load(
        &environment,
        &[(path.as_path(), program, source)],
        |candidate| candidate == path.as_path(),
    );
    assert!(!external.is_empty());
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
            super::super::LibrarySource {
                program,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        )],
        &identities,
    );
    types.add_external(&external, &identities);
    let model = SemanticModel::new(&path, &parsed, &identities, Some(&types));
    let base = model.resolve_name("Base").unwrap();
    assert!(matches!(base, DeclarationIdentity::Package { .. }));
    let child = model.resolve_type_name(&["Child"]).unwrap();
    let other = model.resolve_name("Other").unwrap();
    let files = [(path.clone(), parsed.clone())];
    let complete_external = SignatureIndex::build(&[], Some(&external), &identities, &types);
    for (external, expected) in [
        (complete_external, Truth::No),
        (SignatureIndex::default(), Truth::Unknown),
    ] {
        let signatures =
            SignatureIndex::with_external(&files, &identities, &types, Arc::new(external));
        assert_eq!(
            signatures.relation(&child, TypeRelation::SubtypeOfDeclaration(&base), &model),
            Truth::Yes,
        );
        assert_eq!(
            signatures.relation(&child, TypeRelation::SubtypeOfDeclaration(&other), &model),
            expected,
        );
    }
}
