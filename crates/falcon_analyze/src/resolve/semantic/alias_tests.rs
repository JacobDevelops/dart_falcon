use super::*;
use crate::resolve::IdentitySource;
use falcon_dart_parser::parse;

fn with_model(source: &str, test: impl FnOnce(&SemanticModel<'_>)) {
    let parsed = parse(source);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let parsed = Arc::new(crate::ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    });
    let path = Path::new("/project/aliases.dart");
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        None,
    );
    test(&SemanticModel::new(path, &parsed, &identities, None));
}

#[test]
fn retained_type_occurrences_survive_body_shifts_without_conflating_alias_uses() {
    fn parsed(source: String) -> Arc<crate::ParsedFile> {
        let result = parse(&source);
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        Arc::new(crate::ParsedFile {
            source: source.into_boxed_str(),
            tree: result.tree,
        })
    }

    fn facts(
        parsed: &Arc<crate::ParsedFile>,
        identities: &IdentityIndex,
    ) -> (ResolvedType, Vec<Arc<AliasProvenance>>) {
        let model = SemanticModel::new(Path::new("/project/aliases.dart"), parsed, identities, None);
        let mut parameter = None;
        let mut aliases = Vec::with_capacity(2);
        for declaration in parsed.program().declarations() {
            match declaration {
                TopLevelDecl::Class(class) => {
                    let mut scope = TypeParameterScope::default();
                    scope.push(class.type_params(), &model);
                    parameter = scope.lookup("T");
                }
                TopLevelDecl::Function(function) if function.name().name() != "initState" => {
                    let ty = model.resolve_type_in(
                        function.return_type().unwrap(),
                        &TypeParameterScope::default(),
                    );
                    let ResolvedTypeKind::Function { alias, .. } = ty.kind else {
                        panic!("expected alias function type");
                    };
                    aliases.push(alias.0.expect("alias source provenance"));
                }
                _ => {}
            }
        }
        (parameter.unwrap(), aliases)
    }

    let source = |body| {
        format!(
            "typedef Builder = int Function(); void initState() {{ {body} }} \
             Builder first() => () => 1; Builder second() => () => 2; \
             class Box<T> {{ T identity(T value) => value; }}"
        )
    };
    let original = parsed(source(""));
    let path = Path::new("/project/aliases.dart");
    let mut identities = IdentityIndex::build(
        &[IdentitySource {
            path,
            parsed: &original,
            has_parse_errors: false,
        }],
        &[],
        &[],
        None,
    );
    let expected = facts(&original, &identities);
    assert_ne!(expected.1[0], expected.1[1]);
    let file = identities.file(path).unwrap();
    for body in [
        "final items = <String>[];",
        "final items = <int>[1, 2, 3]; // a longer body\n",
        "",
    ] {
        let edited = parsed(source(body));
        assert_eq!(
            original.declaration_signature(),
            edited.declaration_signature()
        );
        identities.refresh_file_body(file, &edited);
        assert_eq!(facts(&edited, &identities), expected);
        // Retained templates still resolve against their original parsed file.
        assert_eq!(facts(&original, &identities), expected);
    }
}

#[test]
fn exact_function_shapes_support_alpha_binders_and_named_variance() {
    with_model(
        "typedef F = T Function<T extends num>(T value);
         typedef G = U Function<U extends num>(U value);
         typedef H = U Function<U extends String>(U value);
         typedef Two = T Function<T extends num, U>(T value);
         typedef Plain = void Function();
         typedef Optional = void Function({int value});
         typedef Required = void Function({required int value});",
        |model| {
            let signatures = SignatureIndex::default();
            let resolve = |name| model.resolve_type_name(&[name]).unwrap();
            assert_eq!(
                signatures.relation(
                    &resolve("F"),
                    crate::TypeRelation::SubtypeOf(&resolve("G")),
                    model
                ),
                Truth::Yes
            );
            assert_eq!(
                signatures.relation(
                    &resolve("G"),
                    crate::TypeRelation::SubtypeOf(&resolve("F")),
                    model
                ),
                Truth::Yes
            );
            assert_eq!(
                signatures.relation(
                    &resolve("F"),
                    crate::TypeRelation::SubtypeOf(&resolve("H")),
                    model
                ),
                Truth::No
            );
            assert_eq!(
                signatures.relation(
                    &resolve("F"),
                    crate::TypeRelation::SubtypeOf(&resolve("Two")),
                    model
                ),
                Truth::No
            );
            assert_eq!(
                signatures.relation(
                    &resolve("Optional"),
                    crate::TypeRelation::SubtypeOf(&resolve("Plain")),
                    model
                ),
                Truth::Yes
            );
            assert_eq!(
                signatures.relation(
                    &resolve("Required"),
                    crate::TypeRelation::SubtypeOf(&resolve("Plain")),
                    model
                ),
                Truth::No
            );
            assert_eq!(
                signatures.relation(
                    &resolve("Plain"),
                    crate::TypeRelation::SubtypeOf(&resolve("Optional")),
                    model
                ),
                Truth::No
            );
            let known = resolve("Plain");
            let empty = FunctionTypeShape::empty();
            assert!(Arc::ptr_eq(
                &empty,
                &FunctionTypeShape::new(Vec::new(), Vec::new())
            ));
            let mut unknown = known.clone();
            let ResolvedType {
                kind: ResolvedTypeKind::Function { shape, .. },
            } = &mut unknown
            else {
                panic!("missing function")
            };
            *shape = None;
            assert_ne!(known, unknown);
            assert_ne!(
                known.structural_hash(&mut FxHashMap::default()),
                unknown.structural_hash(&mut FxHashMap::default())
            );
            assert_eq!(
                signatures.relation(&unknown, crate::TypeRelation::SubtypeOf(&unknown), model),
                Truth::Unknown
            );
            let mut unknown_slot = resolve("Plain");
            let ResolvedType {
                kind: ResolvedTypeKind::Function { return_type, .. },
            } = &mut unknown_slot
            else {
                panic!("missing function")
            };
            *return_type = Arc::new(ResolvedType::UNKNOWN);
            assert_eq!(
                signatures.relation(
                    &unknown_slot,
                    crate::TypeRelation::SubtypeOf(&unknown_slot),
                    model
                ),
                Truth::Unknown
            );
        },
    );
}

#[test]
fn function_shapes_preserve_declared_binders_and_required_names() {
    with_model(
        "typedef F = void Function<T extends num, U>({required T item, U? other});",
        |model| {
            let ty = model.resolve_type_name(&["F"]).unwrap();
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        shape: Some(shape), ..
                    },
            } = &ty
            else {
                panic!("missing exact function shape")
            };
            assert_eq!(shape.required_named.as_ref(), &["item"]);
            assert_eq!(shape.type_parameters.len(), 2);
            let ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { id, bound, .. },
            } = &shape.type_parameters[0]
            else {
                panic!("missing declared binder")
            };
            assert_eq!(id.name.as_ref(), "T");
            assert!(bound.interface("dart:core", "num"));
            let ResolvedType {
                kind: ResolvedTypeKind::TypeParameter { id, .. },
            } = &shape.type_parameters[1]
            else {
                panic!("missing unused binder")
            };
            assert_eq!(id.name.as_ref(), "U");
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        shape: Some(nullable_shape),
                        ..
                    },
            } = ty.with_nullable(true)
            else {
                panic!("lost nullable function shape")
            };
            assert!(Arc::ptr_eq(shape, &nullable_shape));
        },
    );
}

#[test]
fn function_shape_bounds_substitute_without_replacing_declared_binders() {
    with_model("", |model| {
        let outer = TypeParameterId {
            owner: ParameterOwner::Sdk("shape-outer"),
            ordinal: 0,
            name: "X".into(),
        };
        let binder = TypeParameterId {
            owner: ParameterOwner::Sdk("shape-inner"),
            ordinal: 0,
            name: "X".into(),
        };
        let shape = FunctionTypeShape::new(
            vec!["item".into()],
            vec![ResolvedType {
                kind: ResolvedTypeKind::TypeParameter {
                    id: binder.clone(),
                    bound: Arc::new(ResolvedType {
                        kind: ResolvedTypeKind::TypeParameter {
                            id: outer.clone(),
                            bound: Arc::new(ResolvedType::DYNAMIC),
                            nullable: false,
                        },
                    }),
                    nullable: false,
                },
            }],
        );
        assert!(Arc::ptr_eq(&shape, &shape.map_bounds(Clone::clone)));
        let origin = Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, 1)),
        });
        let function = ResolvedType {
            kind: ResolvedTypeKind::Function {
                shape: Some(shape),
                alias: FunctionTypeAlias(Some(Arc::clone(&origin))),
                optional_positional: 0,
                return_type: Arc::new(ResolvedType::VOID),
                positional: empty_type_arguments(),
                named: empty_named_types(),
                nullable: false,
            },
        };
        let substitutions = FxHashMap::from_iter([(outer.clone(), ResolvedType::VOID)]);
        assert_eq!(substitutions.get(&outer), Some(&ResolvedType::VOID));
        assert!(!substitutions.contains_key(&binder));
        let result = model.substitute(&function, &substitutions);
        let repeated = model.substitute(&function, &substitutions);
        assert!(SubstitutionKey(&result) == SubstitutionKey(&repeated));
        let ResolvedType {
            kind: ResolvedTypeKind::Function { alias, .. },
        } = &result
        else {
            panic!("lost substituted function")
        };
        assert_eq!(alias.identity(), Some(&DeclarationIdentity::OBJECT));
        assert!(matches!(alias.0.as_deref(),
            Some(AliasProvenance::Substituted { source }) if Arc::ptr_eq(source, &origin)));
        let ResolvedType {
            kind: ResolvedTypeKind::Function {
                shape: Some(shape), ..
            },
        } = &result
        else {
            panic!("lost substituted shape")
        };
        assert_eq!(shape.required_named.as_ref(), &["item"]);
        assert!(matches!(&shape.type_parameters[0],
            ResolvedType { kind: ResolvedTypeKind::TypeParameter { id, bound, .. } } if *id == binder && **bound == ResolvedType::VOID));
        let joined = inference_join(
            &result,
            &result.with_nullable(true),
            &SignatureIndex::default(),
            model,
        );
        let ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    shape: Some(joined_shape),
                    ..
                },
        } = joined
        else {
            panic!("lost joined shape")
        };
        assert_eq!(shape, &joined_shape);
    });
}

#[test]
fn deep_alias_fanout_shares_templates_without_a_depth_limit() {
    let mut source = String::with_capacity(2048);
    for level in 0..40 {
        source.push_str(&format!(
            "typedef A{level} = (A{}, A{}, A{});\n",
            level + 1,
            level + 1,
            level + 1
        ));
    }
    source.push_str("typedef A40 = int;");
    with_model(&source, |model| {
        let mut ty = model.resolve_type_name(&["A0"]).unwrap();
        for _ in 0..40 {
            let ResolvedType {
                kind: ResolvedTypeKind::Record { positional, .. },
            } = &ty
            else {
                panic!("lost alias depth")
            };
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        positional: first, ..
                    },
            } = &positional[0]
            else {
                assert!(positional[0].interface("dart:core", "int"));
                ty = ty.record_positional(0).unwrap();
                continue;
            };
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Record {
                        positional: second, ..
                    },
            } = &positional[1]
            else {
                panic!("unequal branches")
            };
            assert!(Arc::ptr_eq(first, second));
            ty = ty.record_positional(0).unwrap();
        }
        assert!(ty.interface("dart:core", "int"));
    });
}

#[test]
fn cyclic_aliases_terminate_and_do_not_poison_unrelated_aliases() {
    with_model(
        "typedef F = void Function(F, F, F); typedef G = H; typedef H = G; typedef Good = int;",
        |model| {
            assert_eq!(model.resolve_type_name(&["F"]), Some(ResolvedType::UNKNOWN));
            assert_eq!(model.resolve_type_name(&["G"]), Some(ResolvedType::UNKNOWN));
            assert!(
                model
                    .resolve_type_name(&["Good"])
                    .unwrap()
                    .interface("dart:core", "int")
            );
        },
    );
}

#[test]
fn alias_template_panic_leaves_the_same_cache_resolvable() {
    with_model("typedef Good = int;", |model| {
        let identity = model.resolve_name("Good").unwrap();
        let alias = model.identities.type_alias(&identity).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            alias.templates.resolve(false, || {
                panic!("interrupted alias expansion");
            })
        }));
        assert!(result.is_err());
        let first = model.alias_template(alias).unwrap();
        let second = model.alias_template(alias).unwrap();
        assert!(first.1.interface("dart:core", "int"));
        assert!(Arc::ptr_eq(&first, &second));
        assert!(
            model
                .resolve_type_name(&["Good"])
                .unwrap()
                .interface("dart:core", "int")
        );
    });
}

#[test]
fn cloned_identities_rebuild_alias_templates() {
    with_model("typedef Good = int;", |model| {
        let identity = model.resolve_name("Good").unwrap();
        let alias = model.identities.type_alias(&identity).unwrap();
        let original = model.alias_template(alias).unwrap();
        let identities = model.identities.clone();
        let cloned_model = SemanticModel::new(
            Path::new("/project/aliases.dart"),
            identities.parsed_file(alias.file),
            &identities,
            None,
        );
        let cloned_alias = identities.type_alias(&identity).unwrap();
        let cloned = cloned_model.alias_template(cloned_alias).unwrap();
        assert!(!Arc::ptr_eq(&original, &cloned));
        assert_eq!(original, cloned);
        assert!(Arc::ptr_eq(
            &cloned,
            &cloned_model.alias_template(cloned_alias).unwrap()
        ));
    });
}

#[test]
fn concurrent_alias_roots_share_the_published_template() {
    with_model(
        "typedef Root<T> = List<Leaf<T>>; typedef Leaf<T> = T;",
        |model| {
            let identity = model.resolve_name("Root").unwrap();
            let alias = model.identities.type_alias(&identity).unwrap();
            let barrier = std::sync::Barrier::new(2);
            let templates = std::thread::scope(|scope| {
                let spawn = || {
                    scope.spawn(|| {
                        barrier.wait();
                        model.alias_template(alias).unwrap()
                    })
                };
                let first = spawn();
                let second = spawn();
                (first.join().unwrap(), second.join().unwrap())
            });
            assert!(Arc::ptr_eq(&templates.0, &templates.1));
            assert!(templates.0.1.interface("dart:core", "List"));
            assert!(Arc::ptr_eq(
                &templates.0,
                &model.alias_template(alias).unwrap()
            ));
        },
    );
}

#[test]
fn competing_cold_alias_computations_return_the_installed_winner() {
    let templates = AliasTemplates::default();
    let barrier = std::sync::Barrier::new(2);
    let results = std::thread::scope(|scope| {
        let spawn = || {
            scope.spawn(|| {
                templates
                    .resolve(false, || {
                        barrier.wait();
                        Arc::new((Vec::new(), ResolvedType::DYNAMIC))
                    })
                    .unwrap()
            })
        };
        let first = spawn();
        let second = spawn();
        (first.join().unwrap(), second.join().unwrap())
    });
    assert!(Arc::ptr_eq(&results.0, &results.1));
    let typed = templates
        .resolve(true, || Arc::new((Vec::new(), ResolvedType::VOID)))
        .unwrap();
    assert_eq!(typed.1, ResolvedType::VOID);
    assert!(!Arc::ptr_eq(&results.0, &typed));
}

#[test]
fn invalid_publication_wins_over_a_competing_candidate_and_invalidates_its_parent() {
    let root = Arc::new(AliasTemplates::default());
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let (sender, receiver) = std::sync::mpsc::channel();
    let thread_root = Arc::clone(&root);
    let thread_barrier = Arc::clone(&barrier);
    let thread = std::thread::spawn(move || {
        assert!(
            thread_root
                .resolve(false, || {
                    thread_barrier.wait();
                    assert!(
                        thread_root
                            .resolve(false, || panic!("recursive build"))
                            .is_none()
                    );
                    Arc::new((Vec::new(), ResolvedType::UNKNOWN))
                })
                .is_none()
        );
        sender.send(()).unwrap();
    });
    let parent = AliasTemplates::default();
    assert!(
        parent
            .resolve(false, || {
                assert!(
                    root.resolve(false, || {
                        barrier.wait();
                        receiver
                            .recv_timeout(std::time::Duration::from_secs(5))
                            .expect("invalid template must publish without waiting");
                        Arc::new((Vec::new(), ResolvedType::DYNAMIC))
                    })
                    .is_none()
                );
                Arc::new((Vec::new(), ResolvedType::DYNAMIC))
            })
            .is_none()
    );
    thread.join().unwrap();
    assert!(
        parent
            .resolve(false, || panic!("cached invalid parent"))
            .is_none()
    );
}

#[test]
fn opposing_alias_roots_do_not_wait_and_invalidate_cached_parents() {
    let roots = [
        Arc::new(AliasTemplates::default()),
        Arc::new(AliasTemplates::default()),
    ];
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let (sender, receiver) = std::sync::mpsc::channel();
    let threads: Vec<_> = (0..2)
        .map(|index| {
            let root = Arc::clone(&roots[index]);
            let other = Arc::clone(&roots[1 - index]);
            let barrier = Arc::clone(&barrier);
            let sender = sender.clone();
            std::thread::spawn(move || {
                let result = root.resolve(false, || {
                    barrier.wait();
                    assert!(
                        other
                            .resolve(false, || {
                                assert!(
                                    root.resolve(false, || panic!("recursive build")).is_none()
                                );
                                Arc::new((Vec::new(), ResolvedType::UNKNOWN))
                            })
                            .is_none()
                    );
                    Arc::new((Vec::new(), ResolvedType::UNKNOWN))
                });
                sender.send(result.is_none()).unwrap();
            })
        })
        .collect();
    drop(sender);
    for _ in 0..2 {
        assert!(
            receiver
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("opposing alias roots must finish without waiting")
        );
    }
    for thread in threads {
        thread.join().unwrap();
    }
    for root in roots {
        let parent = AliasTemplates::default();
        assert!(
            parent
                .resolve(false, || {
                    assert!(root.resolve(false, || panic!("cached cycle")).is_none());
                    Arc::new((Vec::new(), ResolvedType::UNKNOWN))
                })
                .is_none()
        );
        assert!(
            parent
                .resolve(false, || panic!("cached invalid parent"))
                .is_none()
        );
    }
}

#[test]
fn cached_alias_cycles_invalidate_enclosing_model_templates() {
    with_model(
        "typedef A = B; typedef B = A; typedef Parent = List<A>;",
        |model| {
            assert_eq!(model.resolve_type_name(&["A"]), Some(ResolvedType::UNKNOWN));
            assert_eq!(
                model.resolve_type_name(&["Parent"]),
                Some(ResolvedType::UNKNOWN)
            );
            let identity = model.resolve_name("Parent").unwrap();
            let alias = model.identities.type_alias(&identity).unwrap();
            assert!(model.alias_template(alias).is_none());
        },
    );
}

#[test]
fn recursive_bounds_reverse_variance_in_function_parameters() {
    with_model("", |model| {
        let id = TypeParameterId {
            owner: ParameterOwner::Sdk("bound-test"),
            ordinal: 0,
            name: "T".into(),
        };
        let parameter = ResolvedType {
            kind: ResolvedTypeKind::TypeParameter {
                id: id.clone(),
                bound: Arc::new(ResolvedType::DYNAMIC),
                nullable: false,
            },
        };
        let bound = ResolvedType {
            kind: ResolvedTypeKind::Function {
                shape: Some(FunctionTypeShape::empty()),
                alias: Default::default(),
                optional_positional: 0,
                return_type: Arc::new(parameter.clone()),
                positional: vec![parameter.clone()].into(),
                named: vec![("value".to_owned(), parameter)].into(),
                nullable: false,
            },
        };
        let bounds = FxHashMap::from_iter([(id.clone(), bound)]);
        let result = instantiate_parameter_bound(&id, &bounds, &mut HashSet::new(), model);
        let ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    return_type,
                    positional,
                    named,
                    ..
                },
        } = result
        else {
            panic!("expected function bound")
        };
        assert_eq!(*return_type, ResolvedType::DYNAMIC);
        assert_eq!(positional.as_ref(), &[ResolvedType::NEVER]);
        assert_eq!(named[0].1, ResolvedType::NEVER);
    });
}

#[test]
fn cloned_type_parameters_share_their_bound() {
    let ty = ResolvedType {
        kind: ResolvedTypeKind::TypeParameter {
            id: TypeParameterId {
                owner: ParameterOwner::Sdk("share-test"),
                ordinal: 0,
                name: "T".into(),
            },
            bound: Arc::new(ResolvedType::DYNAMIC),
            nullable: false,
        },
    };
    let ResolvedType {
        kind:
            ResolvedTypeKind::TypeParameter {
                bound: original,
                id: original_id,
                ..
            },
    } = &ty
    else {
        unreachable!()
    };
    let ResolvedType {
        kind:
            ResolvedTypeKind::TypeParameter {
                bound: cloned,
                id: cloned_id,
                ..
            },
    } = ty.with_nullable(true)
    else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(original, &cloned));
    assert!(Arc::ptr_eq(&original_id.name, &cloned_id.name));
}

#[test]
fn generic_alias_fanout_substitution_retains_shared_nodes() {
    let mut source = String::with_capacity(2048);
    for level in 0..24 {
        source.push_str(&format!(
            "typedef A{level}<T> = (A{}<T>, A{}<T>, A{}<T>);\n",
            level + 1,
            level + 1,
            level + 1
        ));
    }
    source.push_str("typedef A24<T> = T;");
    with_model(&source, |model| {
        let identity = model.resolve_name("A0").unwrap();
        let alias = model.identities.type_alias(&identity).unwrap();
        let mut ty = model.expand_alias(
            &identity,
            alias,
            &[model.core_type("int", false)],
            false,
            None,
        );
        for _ in 0..24 {
            let ResolvedType {
                kind: ResolvedTypeKind::Record { .. },
            } = &ty
            else {
                panic!("lost generic alias depth")
            };
            ty = ty.record_positional(0).unwrap();
        }
        assert!(ty.interface("dart:core", "int"));
    });
}

#[test]
fn function_and_interface_alias_fanout_retains_shared_children() {
    for function in [false, true] {
        let mut source = String::with_capacity(2048);
        source.push_str("class Triple<T, U, V> {}\n");
        for level in 0..24 {
            let next = level + 1;
            if function {
                source.push_str(&format!(
                    "typedef A{level} = A{next} Function(A{next}, A{next});\n"
                ));
            } else {
                source.push_str(&format!(
                    "typedef A{level} = Triple<A{next}, A{next}, A{next}>;\n"
                ));
            }
        }
        source.push_str("typedef A24 = int;");
        with_model(&source, |model| {
            let mut ty = model.resolve_type_name(&["A0"]).unwrap();
            for _ in 0..24 {
                ty = match &ty {
                    ResolvedType {
                        kind: ResolvedTypeKind::Function { return_type, .. },
                    } => return_type.as_ref().clone(),
                    ResolvedType {
                        kind: ResolvedTypeKind::Interface { .. },
                    } => ty.type_argument(0).unwrap(),
                    _ => panic!("lost alias nesting"),
                };
            }
            assert!(ty.interface("dart:core", "int"));
        });
    }
}

#[test]
fn generic_function_alias_fanout_preserves_origins_and_shared_descendants() {
    let mut source = String::with_capacity(2048);
    for level in 0..24 {
        let next = level + 1;
        source.push_str(&format!(
            "typedef A{level}<T> = A{next}<T> Function(A{next}<T>, A{next}<T>);\n"
        ));
    }
    source.push_str("typedef A24<T> = T;");
    with_model(&source, |model| {
        let identity = model.resolve_name("A0").unwrap();
        let alias = model.identities.type_alias(&identity).unwrap();
        let ty = model.expand_alias(
            &identity,
            alias,
            &[model.core_type("int", false)],
            false,
            None,
        );
        let ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    return_type,
                    positional,
                    ..
                },
        } = &ty
        else {
            panic!("lost generic function alias")
        };
        assert!(SubstitutionKey(return_type) != SubstitutionKey(&positional[0]));
        assert!(SubstitutionKey(&positional[0]) != SubstitutionKey(&positional[1]));
        let (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type: left, ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type: right, ..
                    },
            },
        ) = (return_type.as_ref(), &positional[0])
        else {
            panic!("lost generic function children")
        };
        let (
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        positional: left, ..
                    },
            },
            ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        positional: right, ..
                    },
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("lost generic function grandchildren")
        };
        assert!(Arc::ptr_eq(left, right));

        let mut visited = FxHashSet::default();
        let mut pending = vec![&ty];
        while let Some(node) = pending.pop() {
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type,
                        positional,
                        named,
                        ..
                    },
            } = node
            else {
                assert!(node.interface("dart:core", "int"));
                continue;
            };
            if !visited.insert(SubstitutionSourceKey::of(node).unwrap()) {
                continue;
            }
            pending.push(return_type);
            pending.extend(positional.iter());
            pending.extend(named.iter().map(|(_, ty)| ty));
        }
        assert!(visited.len() <= 3 * 24);
        let mut leaf = &ty;
        for _ in 0..24 {
            let ResolvedType {
                kind: ResolvedTypeKind::Function { return_type, .. },
            } = leaf
            else {
                panic!("lost generic function depth")
            };
            leaf = return_type;
        }
        assert!(leaf.interface("dart:core", "int"));
    });
}

#[test]
fn separate_generic_function_alias_uses_keep_distinct_origins() {
    with_model("typedef F<T> = T Function(T);", |model| {
        let identity = model.resolve_name("F").unwrap();
        let alias = model.identities.type_alias(&identity).unwrap();
        let arguments = [model.core_type("int", false)];
        let first = model.expand_alias(&identity, alias, &arguments, false, Some((None, 1)));
        let second = model.expand_alias(&identity, alias, &arguments, false, Some((None, 2)));
        assert!(first == second);
        assert!(SubstitutionKey(&first) != SubstitutionKey(&second));
    });
}

#[test]
fn mutually_recursive_bounds_replace_the_entire_component_by_variance() {
    with_model("", |model| {
        let parameter = |name: &str, ordinal| TypeParameterId {
            owner: ParameterOwner::Sdk("mutual-bound-test"),
            ordinal,
            name: name.into(),
        };
        let a = parameter("A", 0);
        let b = parameter("B", 1);
        let reference = |id: &TypeParameterId| ResolvedType {
            kind: ResolvedTypeKind::TypeParameter {
                id: id.clone(),
                bound: Arc::new(ResolvedType::DYNAMIC),
                nullable: false,
            },
        };
        let bounds = FxHashMap::from_iter([
            (
                a.clone(),
                ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "List"),
                        arguments: vec![reference(&b)].into(),
                        nullable: false,
                        extension_type: false,
                    },
                },
            ),
            (
                b.clone(),
                ResolvedType {
                    kind: ResolvedTypeKind::Function {
                        shape: Some(FunctionTypeShape::empty()),
                        alias: Default::default(),
                        optional_positional: 0,
                        return_type: Arc::new(ResolvedType::VOID),
                        positional: vec![reference(&a)].into(),
                        named: Arc::from([]),
                        nullable: false,
                    },
                },
            ),
        ]);
        let a_bound = instantiate_parameter_bound(&a, &bounds, &mut HashSet::new(), model);
        assert_eq!(a_bound.arguments(), &[ResolvedType::DYNAMIC]);
        let b_bound = instantiate_parameter_bound(&b, &bounds, &mut HashSet::new(), model);
        let ResolvedType {
            kind: ResolvedTypeKind::Function { positional, .. },
        } = b_bound
        else {
            panic!("expected function bound")
        };
        assert_eq!(positional.as_ref(), &[ResolvedType::NEVER]);
    });
}

#[test]
fn cyclic_alias_roots_are_unknown_independent_of_query_order() {
    for order in [["A", "B", "C"], ["C", "B", "A"]] {
        with_model(
            "typedef A = List<B>; typedef B = void Function(C); typedef C = A;",
            |model| {
                for name in order {
                    assert_eq!(
                        model.resolve_type_name(&[name]),
                        Some(ResolvedType::UNKNOWN)
                    );
                }
                for name in order {
                    assert_eq!(
                        model.resolve_type_name(&[name]),
                        Some(ResolvedType::UNKNOWN)
                    );
                }
            },
        );
    }
}

#[test]
fn external_parameter_bounds_are_instantiated_in_their_own_variance() {
    with_model("", |model| {
        let a = TypeParameterId {
            owner: ParameterOwner::Sdk("external-bound"),
            ordinal: 0,
            name: "A".into(),
        };
        let b = TypeParameterId {
            owner: ParameterOwner::Sdk("external-bound"),
            ordinal: 1,
            name: "B".into(),
        };
        let reference = |id: &TypeParameterId| ResolvedType {
            kind: ResolvedTypeKind::TypeParameter {
                id: id.clone(),
                bound: Arc::new(ResolvedType::DYNAMIC),
                nullable: false,
            },
        };
        let bounds = FxHashMap::from_iter([
            (
                a.clone(),
                ResolvedType {
                    kind: ResolvedTypeKind::Interface {
                        occurrence: Default::default(),
                        identity: DeclarationIdentity::sdk("dart:core", "Comparable"),
                        arguments: vec![reference(&a)].into(),
                        nullable: false,
                        extension_type: false,
                    },
                },
            ),
            (
                b.clone(),
                ResolvedType {
                    kind: ResolvedTypeKind::Function {
                        shape: Some(FunctionTypeShape::empty()),
                        alias: Default::default(),
                        optional_positional: 0,
                        return_type: Arc::new(ResolvedType::VOID),
                        positional: vec![reference(&a)].into(),
                        named: Arc::from([]),
                        nullable: false,
                    },
                },
            ),
        ]);
        let result = instantiate_parameter_bound(&b, &bounds, &mut HashSet::new(), model);
        let ResolvedType {
            kind: ResolvedTypeKind::Function { positional, .. },
        } = result
        else {
            panic!("expected function bound")
        };
        assert!(positional[0].interface("dart:core", "Comparable"));
        assert_eq!(positional[0].arguments(), &[ResolvedType::DYNAMIC]);
    });
}

#[test]
fn unresolved_external_types_do_not_poison_alias_templates() {
    with_model("typedef Legit<T> = List<Missing>;", |model| {
        let identity = model.resolve_name("Legit").unwrap();
        let alias = model.identities.type_alias(&identity).unwrap();
        let first = model.alias_template(alias).unwrap();
        let second = model.alias_template(alias).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert!(
            model
                .resolve_type_name(&["Legit"])
                .unwrap()
                .interface("dart:core", "List")
        );
    });
}

#[test]
fn substitution_source_keys_preserve_live_storage_and_scalar_context() {
    with_model("", |model| {
        let parameter_id = TypeParameterId {
            owner: ParameterOwner::Sdk("memo-source"),
            ordinal: 0,
            name: "T".into(),
        };
        let parameter = ResolvedType {
            kind: ResolvedTypeKind::TypeParameter {
                id: parameter_id.clone(),
                bound: Arc::new(ResolvedType::DYNAMIC),
                nullable: false,
            },
        };
        let source = ResolvedType {
            kind: ResolvedTypeKind::Function {
                return_type: Arc::new(parameter.clone()),
                positional: vec![parameter.clone()].into(),
                named: empty_named_types(),
                optional_positional: 0,
                nullable: false,
                alias: Default::default(),
                shape: None,
            },
        };
        let mut optional = source.clone();
        let ResolvedType {
            kind:
                ResolvedTypeKind::Function {
                    optional_positional,
                    ..
                },
        } = &mut optional
        else {
            unreachable!()
        };
        *optional_positional = 1;
        let distinct = ResolvedType {
            kind: ResolvedTypeKind::Function {
                return_type: Arc::new(parameter.clone()),
                positional: vec![parameter].into(),
                named: empty_named_types(),
                optional_positional: 0,
                nullable: false,
                alias: Default::default(),
                shape: None,
            },
        };
        let sources = [
            source.clone(),
            source.with_nullable(true),
            optional,
            distinct,
        ];
        let mut keys = FxHashSet::default();
        assert!(keys.insert(SubstitutionSourceKey::of(&source).unwrap()));
        assert!(!keys.insert(SubstitutionSourceKey::of(&sources[0]).unwrap()));
        for ty in &sources[1..] {
            assert!(keys.insert(SubstitutionSourceKey::of(ty).unwrap()));
        }

        let substitutions = FxHashMap::from_iter([(parameter_id, ResolvedType::VOID)]);
        let mut memo = SubstitutionMemo::default();
        for (index, ty) in sources.iter().enumerate() {
            let result = model.substitute_memoized(ty, &substitutions, &mut memo);
            let ResolvedType {
                kind:
                    ResolvedTypeKind::Function {
                        return_type,
                        positional,
                        nullable,
                        optional_positional,
                        ..
                    },
            } = result
            else {
                panic!("lost substituted function")
            };
            assert_eq!(*return_type, ResolvedType::VOID);
            assert_eq!(positional.as_ref(), &[ResolvedType::VOID]);
            assert_eq!(nullable, index == 1);
            assert_eq!(optional_positional, usize::from(index == 2));
        }
    });
}
