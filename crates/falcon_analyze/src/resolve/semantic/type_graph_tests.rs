use super::*;

fn parsed() -> &'static Arc<crate::ParsedFile> {
    static PARSED: std::sync::LazyLock<Arc<crate::ParsedFile>> = std::sync::LazyLock::new(|| {
        Arc::new(crate::ParsedFile {
            source: "".into(),
            tree: falcon_dart_parser::parse("").tree,
        })
    });
    &PARSED
}

fn fanout(depth: usize, leaf: ResolvedType) -> ResolvedType {
    let mut ty = leaf;
    for _ in 0..depth {
        ty = ResolvedType::Record {
            occurrence: Default::default(),
            positional: vec![ty.clone(), ty.clone(), ty].into(),
            named: Arc::from([]),
            nullable: false,
        };
    }
    ty
}

fn unshared(depth: usize) -> ResolvedType {
    if depth == 0 {
        return ResolvedType::Dynamic;
    }
    ResolvedType::Record {
        occurrence: Default::default(),
        positional: (0..3).map(|_| unshared(depth - 1)).collect(),
        named: Arc::from([]),
        nullable: false,
    }
}

fn hash(ty: &ResolvedType) -> u64 {
    let mut state = std::collections::hash_map::DefaultHasher::new();
    ty.hash(&mut state);
    state.finish()
}

#[test]
fn flat_interfaces_and_shared_children_do_not_allocate_memos() {
    let left = ResolvedType::interface_identity(DeclarationIdentity::OBJECT);
    let right = ResolvedType::interface_identity(DeclarationIdentity::OBJECT);
    let mut pairs = FxHashSet::default();
    assert!(left.eq_memoized(&right, &mut pairs));
    assert_eq!(pairs.capacity(), 0);
    assert!(!left.eq_memoized(&right.with_nullable(true), &mut pairs));
    assert_eq!(pairs.capacity(), 0);
    let mut hashes = FxHashMap::default();
    left.structural_hash(&mut hashes);
    assert_eq!(hashes.capacity(), 0);
    let shared = fanout(17, ResolvedType::Dynamic);
    assert!(shared.eq_memoized(&shared.clone(), &mut pairs));
    assert_eq!(pairs.capacity(), 0);
}

#[test]
fn independent_deep_graphs_compare_and_hash_once_per_node() {
    let left = fanout(17, ResolvedType::Dynamic);
    let right = fanout(17, ResolvedType::Dynamic);
    let mut pairs = FxHashSet::default();
    assert!(left.eq_memoized(&right, &mut pairs));
    assert!(pairs.len() <= 3 * 17);
    let mut hashes = FxHashMap::default();
    left.structural_hash(&mut hashes);
    assert!(hashes.len() <= 3 * 17);
    assert!(left == right);
    assert_eq!(hash(&left), hash(&right));
}

#[test]
fn structural_equality_and_hash_ignore_sharing_topology() {
    let shared = fanout(4, ResolvedType::Dynamic);
    let expanded = unshared(4);
    assert!(shared == expanded);
    assert_eq!(hash(&shared), hash(&expanded));
    let mut pairs = FxHashSet::default();
    assert!(shared.eq_memoized(&shared, &mut pairs));
    assert!(pairs.is_empty());
}

#[test]
fn sdk_free_unrelatedness_preserves_deep_shared_types() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let signatures = SignatureIndex::default();
    for depth in [17, 80] {
        let left = fanout(depth, ResolvedType::Dynamic);
        let right = fanout(depth, ResolvedType::Dynamic);
        assert!(signatures.extension_type_erasure(&left, &model).is_some());
        assert_eq!(
            signatures.relation(
                &left,
                crate::TypeRelation::RuntimeUnrelatedTo(&right),
                &model
            ),
            Truth::No
        );
        let related = fanout(depth, ResolvedType::Void);
        assert_eq!(
            signatures.relation(
                &left,
                crate::TypeRelation::RuntimeUnrelatedTo(&related),
                &model
            ),
            Truth::No
        );
        assert_eq!(
            signatures.relation(&left, crate::TypeRelation::SubtypeOf(&related), &model),
            Truth::Yes
        );
    }
    let first = model.core_type("int", false);
    let second = model.core_type("int", false);
    let (
        ResolvedType::Interface {
            arguments: first, ..
        },
        ResolvedType::Interface {
            arguments: second, ..
        },
    ) = (&first, &second)
    else {
        panic!("core types must be interfaces")
    };
    assert!(Arc::ptr_eq(first, second));
}

#[test]
fn owner_parameter_mentions_search_shared_nodes_and_bounds() {
    let id = TypeParameterId {
        owner: ParameterOwner::Source {
            file: None,
            offset: 0,
        },
        ordinal: 0,
        name: "T".into(),
    };
    let absent = TypeParameterId {
        ordinal: 1,
        ..id.clone()
    };
    let parameter = ResolvedType::TypeParameter {
        id: id.clone(),
        bound: Arc::new(ResolvedType::Dynamic),
        nullable: false,
    };
    let graph = fanout(17, parameter);
    assert!(mentions_any(&graph, std::slice::from_ref(&id)));
    assert!(!mentions_any(&graph, std::slice::from_ref(&absent)));
    let bounded = ResolvedType::TypeParameter {
        id: absent,
        bound: Arc::new(graph),
        nullable: false,
    };
    assert!(mentions_any(&bounded, &[id]));
}

#[test]
fn substitution_interns_independently_allocated_equal_siblings() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let id = TypeParameterId {
        owner: ParameterOwner::Sdk("structural-substitution"),
        ordinal: 0,
        name: "T".into(),
    };
    let parameter = ResolvedType::TypeParameter {
        id: id.clone(),
        bound: Arc::new(ResolvedType::Dynamic),
        nullable: false,
    };
    let source = ResolvedType::Record {
        occurrence: Default::default(),
        positional: (0..3).map(|_| fanout(17, parameter.clone())).collect(),
        named: empty_named_types(),
        nullable: false,
    };
    let substitutions = HashMap::from([(id, ResolvedType::Void)]);
    let result = model.substitute(&source, &substitutions);
    let ResolvedType::Record { positional, .. } = result else {
        unreachable!()
    };
    let children: Vec<_> = positional
        .iter()
        .map(|child| {
            let ResolvedType::Record { positional, .. } = child else {
                unreachable!()
            };
            positional
        })
        .collect();
    assert!(Arc::ptr_eq(children[0], children[1]));
    assert!(Arc::ptr_eq(children[1], children[2]));
    assert!(positional[0] == fanout(17, ResolvedType::Void));
}

#[test]
fn substitution_memos_shared_sources_and_skips_irrelevant_subgraphs() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let id = TypeParameterId {
        owner: ParameterOwner::Sdk("substitution-source-memo"),
        ordinal: 0,
        name: "T".into(),
    };
    let substitutions = HashMap::from([(id.clone(), ResolvedType::Void)]);
    let parameter = ResolvedType::TypeParameter {
        id,
        bound: Arc::new(ResolvedType::Dynamic),
        nullable: false,
    };
    let source = fanout(24, parameter);
    let mut memo = SubstitutionMemo::default();
    let result = model.substitute_memoized(&source, &substitutions, &mut memo);
    assert_eq!(memo.sources.len(), 24);
    assert_eq!(memo.relevant.len(), 23);
    assert!(memo.hashes.len() <= 3 * 24 + 1);
    assert!(result == fanout(24, ResolvedType::Void));

    let source = fanout(24, ResolvedType::Dynamic);
    let mut memo = SubstitutionMemo::default();
    let result = model.substitute_memoized(&source, &substitutions, &mut memo);
    assert_eq!(memo.relevant.len(), 23);
    assert!(memo.sources.is_empty());
    assert!(memo.hashes.is_empty());
    assert!(memo.structural.is_empty());
    let (
        ResolvedType::Record {
            positional: original,
            ..
        },
        ResolvedType::Record {
            positional: unchanged,
            ..
        },
    ) = (&source, &result)
    else {
        unreachable!()
    };
    assert!(Arc::ptr_eq(original, unchanged));
}

#[test]
fn flat_irrelevant_substitution_roots_do_not_allocate_memos() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let substitutions = HashMap::from([(
        TypeParameterId {
            owner: ParameterOwner::Sdk("irrelevant-root"),
            ordinal: 0,
            name: "T".into(),
        },
        ResolvedType::Void,
    )]);
    let integer = model.core_type("int", false);
    let sources = [
        ResolvedType::Function {
            shape: Some(FunctionTypeShape::empty()),
            alias: FunctionTypeAlias::default(),
            return_type: Arc::new(integer.clone()),
            positional: vec![integer.clone()].into(),
            named: empty_named_types(),
            optional_positional: 0,
            nullable: false,
        },
        ResolvedType::Interface {
            occurrence: Default::default(),
            identity: DeclarationIdentity::sdk("dart:core", "List"),
            arguments: vec![integer].into(),
            nullable: false,
            extension_type: false,
        },
    ];
    for source in &sources {
        let mut memo = SubstitutionMemo::default();
        let result = model.substitute_memoized(source, &substitutions, &mut memo);
        assert!(SubstitutionSourceKey::of(source) == SubstitutionSourceKey::of(&result));
        assert_eq!(memo.relevant.capacity(), 0);
        assert_eq!(memo.sources.capacity(), 0);
        assert_eq!(memo.hashes.capacity(), 0);
        assert_eq!(memo.structural.capacity(), 0);
    }
}

#[test]
fn substitution_memoizes_shared_irrelevant_children_after_relevant_siblings() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let id = TypeParameterId {
        owner: ParameterOwner::Sdk("mixed-substitution-fanout"),
        ordinal: 0,
        name: "T".into(),
    };
    let substitutions = HashMap::from([(id.clone(), ResolvedType::Void)]);
    let record = |positional: Vec<ResolvedType>| ResolvedType::Record {
        occurrence: Default::default(),
        positional: positional.into(),
        named: empty_named_types(),
        nullable: false,
    };
    let irrelevant = record(vec![model.core_type("int", false); 256]);
    let mut source = ResolvedType::TypeParameter {
        id,
        bound: Arc::new(ResolvedType::Dynamic),
        nullable: false,
    };
    for _ in 0..24 {
        source = record(vec![source, irrelevant.clone()]);
    }
    let mut memo = SubstitutionMemo::default();
    let result = model.substitute_memoized(&source, &substitutions, &mut memo);
    let irrelevant_key = SubstitutionSourceKey::of(&irrelevant).unwrap();
    assert_eq!(memo.relevant.get(&irrelevant_key), Some(&false));
    assert_eq!(memo.relevant.len(), 24);
    assert_eq!(memo.sources.len(), 24);
    let mut result = &result;
    for _ in 0..24 {
        let ResolvedType::Record { positional, .. } = result else {
            panic!("lost mixed fanout record")
        };
        assert!(SubstitutionSourceKey::of(&positional[1]) == Some(irrelevant_key));
        result = &positional[0];
    }
    assert_eq!(result, &ResolvedType::Void);
}

#[test]
fn substitution_keys_preserve_distinct_function_origins() {
    let function = |offset| ResolvedType::Function {
        shape: Some(FunctionTypeShape::empty()),
        optional_positional: 0,
        alias: FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, offset)),
        }))),
        return_type: Arc::new(ResolvedType::Dynamic),
        positional: empty_type_arguments(),
        named: empty_named_types(),
        nullable: false,
    };
    let left = fanout(17, function(1));
    let right = fanout(17, function(2));
    assert!(left == right);
    assert!(SubstitutionKey(&left) != SubstitutionKey(&right));
    assert!(SubstitutionKey(&left) == SubstitutionKey(&left.clone()));
}

#[test]
fn substitution_source_keys_preserve_scalar_context_and_alias_origins() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let id = TypeParameterId {
        owner: ParameterOwner::Sdk("substitution-source-context"),
        ordinal: 0,
        name: "T".into(),
    };
    let parameter = ResolvedType::TypeParameter {
        id: id.clone(),
        bound: Arc::new(fanout(24, ResolvedType::Dynamic)),
        nullable: false,
    };
    let positional: Arc<[ResolvedType]> = vec![parameter.clone()].into();
    let named = empty_named_types();
    let interface = ResolvedType::Interface {
        occurrence: Default::default(),
        identity: DeclarationIdentity::sdk("dart:core", "List"),
        arguments: Arc::clone(&positional),
        nullable: false,
        extension_type: false,
    };
    let mut other_identity = interface.clone();
    let ResolvedType::Interface { identity, .. } = &mut other_identity else {
        unreachable!()
    };
    *identity = DeclarationIdentity::sdk("dart:core", "Set");
    let mut extension = interface.clone();
    let ResolvedType::Interface { extension_type, .. } = &mut extension else {
        unreachable!()
    };
    *extension_type = true;
    let record = ResolvedType::Record {
        occurrence: Default::default(),
        positional: Arc::clone(&positional),
        named: Arc::clone(&named),
        nullable: false,
    };
    let provenance = |offset| {
        FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, offset)),
        })))
    };
    let function = ResolvedType::Function {
        shape: Some(FunctionTypeShape::empty()),
        alias: provenance(1),
        optional_positional: 0,
        return_type: Arc::new(parameter.clone()),
        positional,
        named,
        nullable: false,
    };
    let mut optional = function.clone();
    let ResolvedType::Function {
        optional_positional,
        ..
    } = &mut optional
    else {
        unreachable!()
    };
    *optional_positional = 1;
    let mut unaliased = function.clone();
    let ResolvedType::Function { alias, .. } = &mut unaliased else {
        unreachable!()
    };
    *alias = FunctionTypeAlias::default();
    let mut other_origin = function.clone();
    let ResolvedType::Function { alias, .. } = &mut other_origin else {
        unreachable!()
    };
    *alias = provenance(2);
    let mut contextual_interface = interface.clone();
    let ResolvedType::Interface { occurrence, .. } = &mut contextual_interface else {
        unreachable!()
    };
    *occurrence = provenance(3);
    let mut contextual_record = record.clone();
    let ResolvedType::Record { occurrence, .. } = &mut contextual_record else {
        unreachable!()
    };
    *occurrence = provenance(3);
    let mut contextual_function = unaliased.clone();
    let ResolvedType::Function { alias, .. } = &mut contextual_function else {
        unreachable!()
    };
    *alias = alias.projected_from(&provenance(3));
    let cases = vec![
        interface.clone(),
        interface.with_nullable(true),
        other_identity,
        extension,
        contextual_interface,
        record.clone(),
        record.with_nullable(true),
        contextual_record,
        function.clone(),
        function.with_nullable(true),
        optional,
        unaliased,
        other_origin,
        contextual_function,
    ];
    let keys: FxHashSet<_> = cases.iter().filter_map(SubstitutionSourceKey::of).collect();
    assert_eq!(keys.len(), cases.len());
    let substitutions = HashMap::from([(id, ResolvedType::Void)]);
    let expected: Vec<_> = cases
        .iter()
        .map(|ty| model.substitute(ty, &substitutions))
        .collect();
    let source = ResolvedType::Record {
        occurrence: Default::default(),
        positional: cases.into(),
        named: empty_named_types(),
        nullable: false,
    };
    let ResolvedType::Record { positional, .. } = model.substitute(&source, &substitutions) else {
        unreachable!()
    };
    for (actual, expected) in positional.iter().zip(&expected) {
        assert!(SubstitutionKey(actual) == SubstitutionKey(expected));
    }
    let mut memo = SubstitutionMemo::default();
    assert!(matches!(
        model.substitute_memoized(&parameter, &substitutions, &mut memo),
        ResolvedType::Void
    ));
    assert!(memo.hashes.is_empty());
    assert!(memo.structural.is_empty());
}

#[test]
fn substituted_alias_origins_only_share_the_same_source_use() {
    let source = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
        identity: DeclarationIdentity::OBJECT,
        origin: Some((None, 1)),
    })));
    let other = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
        identity: DeclarationIdentity::OBJECT,
        origin: Some((None, 2)),
    })));
    let first = source.substituted();
    let second = source.substituted();
    assert!(!first.same_origin(&second));
    assert!(first.same_origin(&first.clone()));
    assert!(first.same_substitution_source(&second));
    assert!(!first.same_substitution_source(&source));
    assert!(!first.same_substitution_source(&other.substituted()));
    assert!(!source.same_substitution_source(&other));
    assert!(!first.same_origin(&source));
    assert!(!first.same_origin(&other.substituted()));
    assert!(!source.same_origin(&other));
}

#[test]
fn projected_alias_origins_compare_shared_provenance_dags_linearly() {
    let source = || {
        Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, 1)),
        })
    };
    let mut left = source();
    let mut right = source();
    for _ in 0..24 {
        left = Arc::new(AliasProvenance::Projected {
            source: Arc::clone(&left),
            owner: left,
        });
        right = Arc::new(AliasProvenance::Projected {
            source: Arc::clone(&right),
            owner: right,
        });
    }
    let mut seen = FxHashSet::default();
    assert!(AliasProvenance::same_origin_memoized(
        &left, &right, &mut seen
    ));
    assert_eq!(seen.len(), 24);
}

#[test]
fn inference_join_memoizes_contextual_record_fanout() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let signatures = SignatureIndex::default();
    let provenance = |offset| {
        FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, offset)),
        })))
    };
    let mut graph = ResolvedType::Function {
        shape: Some(FunctionTypeShape::empty()),
        alias: provenance(1),
        return_type: Arc::new(ResolvedType::Void),
        positional: empty_type_arguments(),
        named: empty_named_types(),
        optional_positional: 0,
        nullable: false,
    };
    for _ in 0..24 {
        graph = ResolvedType::Record {
            occurrence: Default::default(),
            positional: vec![graph.clone(), graph.clone(), graph].into(),
            named: empty_named_types(),
            nullable: false,
        };
    }
    let left = graph.clone().projected_from(&provenance(2));
    let right = graph.projected_from(&provenance(3));
    let mut memo = InferenceJoinMemo::default();
    let mut joined = inference_join_memoized(&left, &right, &signatures, &model, &mut memo);
    assert_eq!(memo.shapes.values().map(Vec::len).sum::<usize>(), 25);
    // The root was projected before this pass; only its 23 nested records
    // need provenance-presence queries during recursive joins.
    assert_eq!(memo.provenance.len(), 23);
    assert!(memo.hashes.len() <= 24 * 3 + 1);
    for _ in 0..24 {
        let ResolvedType::Record { positional, .. } = &joined else {
            panic!("lost record shape")
        };
        let keys: Vec<_> = positional.iter().map(SubstitutionSourceKey::of).collect();
        assert!(keys[0] == keys[1]);
        assert!(keys[0] == keys[2]);
        joined = joined.record_positional(0).unwrap();
    }
    let ResolvedType::Function { alias, .. } = joined else {
        panic!("lost function descendant")
    };
    assert_eq!(alias.identity(), None);
}

#[test]
fn inference_join_memoizes_interface_fanout() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let signatures = SignatureIndex::default();
    let fanout = |mut ty: ResolvedType| {
        for _ in 0..24 {
            ty = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::OBJECT,
                arguments: vec![ty.clone(), ty.clone(), ty].into(),
                nullable: false,
                extension_type: false,
            };
        }
        ty
    };
    let left = fanout(ResolvedType::Void);
    let right = fanout(ResolvedType::Dynamic);
    let mut memo = InferenceJoinMemo::default();
    let mut joined = inference_join_memoized(&left, &right, &signatures, &model, &mut memo);
    assert_eq!(memo.shapes.values().map(Vec::len).sum::<usize>(), 24);
    assert!(memo.provenance.is_empty());
    for _ in 0..24 {
        let ResolvedType::Interface { arguments, .. } = &joined else {
            panic!("lost interface shape")
        };
        let keys: Vec<_> = arguments.iter().map(SubstitutionSourceKey::of).collect();
        assert!(keys[0] == keys[1]);
        assert!(keys[0] == keys[2]);
        joined = joined.type_argument(0).unwrap();
    }
    assert_eq!(joined, ResolvedType::Void);
}

#[test]
fn ordinary_composite_projections_do_not_add_occurrence_context() {
    let owner = FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
        identity: DeclarationIdentity::OBJECT,
        origin: Some((None, 1)),
    })));
    let function = ResolvedType::Function {
        shape: Some(FunctionTypeShape::empty()),
        alias: Default::default(),
        return_type: Arc::new(ResolvedType::Void),
        positional: empty_type_arguments(),
        named: empty_named_types(),
        optional_positional: 0,
        nullable: false,
    };
    for ty in [
        function.clone(),
        ResolvedType::Interface {
            occurrence: Default::default(),
            identity: DeclarationIdentity::OBJECT,
            arguments: vec![function.clone()].into(),
            nullable: false,
            extension_type: false,
        },
        ResolvedType::Record {
            occurrence: Default::default(),
            positional: vec![function].into(),
            named: empty_named_types(),
            nullable: false,
        },
    ] {
        let projected = ty.clone().projected_from(&owner);
        assert!(SubstitutionSourceKey::of(&ty) == SubstitutionSourceKey::of(&projected));
    }
}

#[test]
fn separate_member_substitution_joins_drop_function_alias_annotations() {
    let identities = IdentityIndex::default();
    let model = SemanticModel::new(Path::new("graph.dart"), parsed(), &identities, None);
    let signatures = SignatureIndex::default();
    let id = TypeParameterId {
        owner: ParameterOwner::Sdk("member-substitution-join"),
        ordinal: 0,
        name: "T".into(),
    };
    let source = ResolvedType::Function {
        shape: Some(FunctionTypeShape::empty()),
        alias: FunctionTypeAlias(Some(Arc::new(AliasProvenance::Source {
            identity: DeclarationIdentity::OBJECT,
            origin: Some((None, 1)),
        }))),
        optional_positional: 0,
        return_type: Arc::new(ResolvedType::TypeParameter {
            id: id.clone(),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        }),
        positional: empty_type_arguments(),
        named: empty_named_types(),
        nullable: false,
    };
    let substitutions = HashMap::from([(id, ResolvedType::Void)]);
    // Independent member reads substitute the same cached signature, but
    // their instantiated alias annotations are not one shared type use.
    let first = model.substitute(&source, &substitutions);
    let second = model.substitute(&source, &substitutions);
    assert!(SubstitutionKey(&first) == SubstitutionKey(&second));
    for nullable in [false, true] {
        let joined = inference_join(&first, &second.with_nullable(nullable), &signatures, &model);
        let ResolvedType::Function {
            alias,
            nullable: actual_nullable,
            ..
        } = joined
        else {
            panic!("lost joined function shape")
        };
        assert!(alias.identity().is_none());
        assert_eq!(actual_nullable, nullable);
    }
    let shared = inference_join(&first, &first.clone(), &signatures, &model);
    let ResolvedType::Function { alias, .. } = shared else {
        panic!("lost shared function alias")
    };
    assert!(alias.identity().is_some());
}

#[test]
fn graph_equality_preserves_leaf_nullability_and_named_fields() {
    let ty = fanout(17, ResolvedType::Dynamic);
    assert!(ty != fanout(17, ResolvedType::Void));
    assert!(ty != ty.with_nullable(true));
    let named = |name: &str| ResolvedType::Record {
        occurrence: Default::default(),
        positional: Arc::from([]),
        named: vec![(name.to_owned(), ty.clone())].into(),
        nullable: false,
    };
    assert!(named("left") != named("right"));
}
