use super::*;
use crate::resolve::IdentitySource;
use falcon_dart_parser::parse;

fn with_model(source: &str, test: impl FnOnce(&SemanticModel<'_>)) {
    let falcon_dart_parser::ParseResult {
        program, errors, ..
    } = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let path = Path::new("/project/aliases.dart");
    let identities = IdentityIndex::build(
        &[IdentitySource {
            path,
            program: &program,
            has_parse_errors: false,
        }],
        &[source],
        &[],
        None,
    );
    test(&SemanticModel::new(path, &identities, None));
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
            let ResolvedType::Record { positional, .. } = &ty else {
                panic!("lost alias depth")
            };
            let ResolvedType::Record {
                positional: first, ..
            } = &positional[0]
            else {
                assert!(positional[0].interface("dart:core", "int"));
                ty = ty.record_positional(0).unwrap();
                continue;
            };
            let ResolvedType::Record {
                positional: second, ..
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
            assert_eq!(model.resolve_type_name(&["F"]), Some(ResolvedType::Unknown));
            assert_eq!(model.resolve_type_name(&["G"]), Some(ResolvedType::Unknown));
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
fn recursive_bounds_reverse_variance_in_function_parameters() {
    with_model("", |model| {
        let id = TypeParameterId {
            owner: ParameterOwner::Sdk("bound-test"),
            ordinal: 0,
            name: "T".into(),
        };
        let parameter = ResolvedType::TypeParameter {
            id: id.clone(),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let bound = ResolvedType::Function {
            alias: Default::default(),
            optional_positional: 0,
            return_type: Arc::new(parameter.clone()),
            positional: vec![parameter.clone()].into(),
            named: vec![("value".to_owned(), parameter)].into(),
            nullable: false,
        };
        let bounds = HashMap::from([(id.clone(), bound)]);
        let result = instantiate_parameter_bound(&id, &bounds, &mut HashSet::new(), model);
        let ResolvedType::Function {
            return_type,
            positional,
            named,
            ..
        } = result
        else {
            panic!("expected function bound")
        };
        assert_eq!(*return_type, ResolvedType::Dynamic);
        assert_eq!(positional.as_ref(), &[ResolvedType::Never]);
        assert_eq!(named[0].1, ResolvedType::Never);
    });
}

#[test]
fn cloned_type_parameters_share_their_bound() {
    let ty = ResolvedType::TypeParameter {
        id: TypeParameterId {
            owner: ParameterOwner::Sdk("share-test"),
            ordinal: 0,
            name: "T".into(),
        },
        bound: Arc::new(ResolvedType::Dynamic),
        nullable: false,
    };
    let ResolvedType::TypeParameter {
        bound: original,
        id: original_id,
        ..
    } = &ty
    else {
        unreachable!()
    };
    let ResolvedType::TypeParameter {
        bound: cloned,
        id: cloned_id,
        ..
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
            let ResolvedType::Record { .. } = &ty else {
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
                    ResolvedType::Function { return_type, .. } => return_type.as_ref().clone(),
                    ResolvedType::Interface { .. } => ty.type_argument(0).unwrap(),
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
        let ResolvedType::Function {
            return_type,
            positional,
            ..
        } = &ty
        else {
            panic!("lost generic function alias")
        };
        assert!(SubstitutionKey(return_type) != SubstitutionKey(&positional[0]));
        assert!(SubstitutionKey(&positional[0]) != SubstitutionKey(&positional[1]));
        let (
            ResolvedType::Function {
                return_type: left, ..
            },
            ResolvedType::Function {
                return_type: right, ..
            },
        ) = (return_type.as_ref(), &positional[0])
        else {
            panic!("lost generic function children")
        };
        let (
            ResolvedType::Function {
                positional: left, ..
            },
            ResolvedType::Function {
                positional: right, ..
            },
        ) = (left.as_ref(), right.as_ref())
        else {
            panic!("lost generic function grandchildren")
        };
        assert!(Arc::ptr_eq(left, right));

        let mut visited = FxHashSet::default();
        let mut pending = vec![&ty];
        while let Some(node) = pending.pop() {
            let ResolvedType::Function {
                return_type,
                positional,
                named,
                ..
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
            let ResolvedType::Function { return_type, .. } = leaf else {
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
        let reference = |id: &TypeParameterId| ResolvedType::TypeParameter {
            id: id.clone(),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let bounds = HashMap::from([
            (
                a.clone(),
                ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:core", "List"),
                    arguments: vec![reference(&b)].into(),
                    nullable: false,
                    extension_type: false,
                },
            ),
            (
                b.clone(),
                ResolvedType::Function {
                    alias: Default::default(),
                    optional_positional: 0,
                    return_type: Arc::new(ResolvedType::Void),
                    positional: vec![reference(&a)].into(),
                    named: Arc::from([]),
                    nullable: false,
                },
            ),
        ]);
        let a_bound = instantiate_parameter_bound(&a, &bounds, &mut HashSet::new(), model);
        assert_eq!(a_bound.arguments(), &[ResolvedType::Dynamic]);
        let b_bound = instantiate_parameter_bound(&b, &bounds, &mut HashSet::new(), model);
        let ResolvedType::Function { positional, .. } = b_bound else {
            panic!("expected function bound")
        };
        assert_eq!(positional.as_ref(), &[ResolvedType::Never]);
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
                        Some(ResolvedType::Unknown)
                    );
                }
                for name in order {
                    assert_eq!(
                        model.resolve_type_name(&[name]),
                        Some(ResolvedType::Unknown)
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
        let reference = |id: &TypeParameterId| ResolvedType::TypeParameter {
            id: id.clone(),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let bounds = HashMap::from([
            (
                a.clone(),
                ResolvedType::Interface {
                    occurrence: Default::default(),
                    identity: DeclarationIdentity::sdk("dart:core", "Comparable"),
                    arguments: vec![reference(&a)].into(),
                    nullable: false,
                    extension_type: false,
                },
            ),
            (
                b.clone(),
                ResolvedType::Function {
                    alias: Default::default(),
                    optional_positional: 0,
                    return_type: Arc::new(ResolvedType::Void),
                    positional: vec![reference(&a)].into(),
                    named: Arc::from([]),
                    nullable: false,
                },
            ),
        ]);
        let result = instantiate_parameter_bound(&b, &bounds, &mut HashSet::new(), model);
        let ResolvedType::Function { positional, .. } = result else {
            panic!("expected function bound")
        };
        assert!(positional[0].interface("dart:core", "Comparable"));
        assert_eq!(positional[0].arguments(), &[ResolvedType::Dynamic]);
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
