use super::super::sdk_hierarchy::SDK_HIERARCHY;
use super::*;

impl SignatureIndex {
    pub(super) fn add_sdk_signatures(&mut self) {
        for name in ["TestOn", "Timeout", "Tags", "OnPlatform"] {
            self.type_facts
                .entry(DeclarationIdentity::package("test", name))
                .or_default()
                .declaration = Some(DeclarationFacts {
                annotations: Vec::new(),
                library_target: true,
                constructors: Vec::new(),
                static_consts: Vec::new(),
                extension_representation: None,
                is_abstract: false,
                has_subclass_in_library: false,
            });
        }
        let add = |index: &mut Self,
                   library: &'static str,
                   type_name: &'static str,
                   arity: usize,
                   member: &str,
                   positional: Vec<ResolvedType>,
                   return_type: ResolvedType| {
            let parameters: Vec<_> = (0..arity)
                .map(|ordinal| sdk_parameter(type_name, ordinal))
                .collect();
            let identity = DeclarationIdentity::sdk(library, type_name);
            let entry = index
                .member_entries
                .entry(identity)
                .or_default()
                .entry(member.to_string())
                .or_default();
            entry.is_instance_method = true;
            entry.signature = Some(ResolvedSignature {
                owner_parameters: parameters,
                shape: Some(FunctionTypeShape::empty()),
                positional,
                ..ResolvedSignature::returning(return_type)
            });
        };
        let parameter = |type_name: &'static str, ordinal: usize| ResolvedType::TypeParameter {
            id: sdk_parameter(type_name, ordinal),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let object = DeclarationIdentity::sdk("dart:core", "Object");
        for (name, kind, parameters) in [
            ("==", MemberKind::Operator, vec!["other".to_string()]),
            (
                "noSuchMethod",
                MemberKind::Method,
                vec!["invocation".to_string()],
            ),
            ("hashCode", MemberKind::Getter, Vec::new()),
            ("runtimeType", MemberKind::Getter, Vec::new()),
            ("toString", MemberKind::Method, Vec::new()),
        ] {
            self.member_entries
                .entry(object.clone())
                .or_default()
                .entry(name.to_string())
                .or_default()
                .facts
                .push(MemberFacts {
                    declaring_type: object.clone(),
                    kind,
                    is_static: false,
                    is_abstract: false,
                    parameter_contracts: Some(if parameters.is_empty() {
                        empty_parameter_contracts()
                    } else {
                        parameters
                            .iter()
                            .map(|name| ParameterContract {
                                name: name.clone(),
                                kind: ParamKind::Positional,
                                is_covariant: false,
                                default_value_code: None,
                            })
                            .collect()
                    }),
                    positional_parameter_names: parameters,
                    annotations: Vec::new(),
                });
        }
        for (library, name, arity, member, positional, return_type) in [
            (
                "dart:core",
                "List",
                1,
                "add",
                vec![parameter("List", 0)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "List",
                1,
                "[]=",
                vec![ResolvedType::Dynamic, parameter("List", 0)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "Set",
                1,
                "add",
                vec![parameter("Set", 0)],
                ResolvedType::interface_identity(DeclarationIdentity::sdk("dart:core", "bool")),
            ),
            (
                "dart:collection",
                "Queue",
                1,
                "add",
                vec![parameter("Queue", 0)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "Map",
                2,
                "[]=",
                vec![parameter("Map", 0), parameter("Map", 1)],
                ResolvedType::Void,
            ),
            (
                "dart:core",
                "String",
                0,
                "toUpperCase",
                Vec::new(),
                ResolvedType::interface_identity(DeclarationIdentity::sdk("dart:core", "String")),
            ),
        ] {
            add(self, library, name, arity, member, positional, return_type);
        }
        for (library, name) in [
            ("dart:core", "List"),
            ("dart:core", "Set"),
            ("dart:collection", "Queue"),
        ] {
            let parameter_id = sdk_parameter(name, 0);
            let parameter = parameter(name, 0);
            let identity = DeclarationIdentity::sdk(library, name);
            let supertype = ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::sdk("dart:core", "Iterable"),
                arguments: vec![parameter.clone()].into(),
                nullable: false,
                extension_type: false,
            };
            self.type_facts
                .entry(identity.clone())
                .or_default()
                .supertypes = Some((vec![parameter_id], vec![supertype.clone()]));
            self.type_facts
                .entry(identity)
                .or_default()
                .member_supertypes = Some(vec![supertype]);
        }
        let iterable = DeclarationIdentity::sdk("dart:core", "Iterable");
        let map_parameter = sdk_parameter("Iterable.map", 0);
        let map_type = parameter("Iterable.map", 0);
        let entry = self
            .member_entries
            .entry(iterable)
            .or_default()
            .entry("map".to_string())
            .or_default();
        entry.is_instance_method = true;
        entry.signature = Some(ResolvedSignature {
            owner_parameters: vec![sdk_parameter("Iterable", 0)],
            call_parameters: vec![map_parameter],
            shape: Some(FunctionTypeShape::new(Vec::new(), vec![map_type.clone()])),
            positional: vec![ResolvedType::Function {
                shape: Some(FunctionTypeShape::empty()),
                alias: FunctionTypeAlias::default(),
                optional_positional: 0,
                return_type: Arc::new(map_type.clone()),
                positional: vec![parameter("Iterable", 0)].into(),
                named: Vec::new().into(),
                nullable: false,
            }],
            ..ResolvedSignature::returning(ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::sdk("dart:core", "Iterable"),
                arguments: vec![map_type].into(),
                nullable: false,
                extension_type: false,
            })
        });

        let widget = DeclarationIdentity::flutter("Widget");
        let key = ResolvedType::interface_identity(DeclarationIdentity::flutter("Key"))
            .with_nullable(true);
        for name in [
            "StatelessWidget",
            "StatefulWidget",
            "InheritedWidget",
            "ProxyWidget",
            "RenderObjectWidget",
            "Text",
            "Container",
            "Builder",
            "ElevatedButton",
            "GestureDetector",
            "MaterialApp",
            "SizedBox",
        ] {
            let identity = DeclarationIdentity::flutter(name);
            let supertype = ResolvedType::interface_identity(widget.clone());
            self.type_facts
                .entry(identity.clone())
                .or_default()
                .supertypes = Some((Vec::new(), vec![supertype.clone()]));
            self.type_facts
                .entry(identity.clone())
                .or_default()
                .superclass = Some(supertype.clone());
            self.type_facts
                .entry(identity.clone())
                .or_default()
                .member_supertypes = Some(vec![supertype]);
            self.type_facts
                .entry(identity)
                .or_default()
                .constructors
                .insert(
                    "new".to_string(),
                    ResolvedSignature {
                        named: HashMap::from([("key".to_string(), key.clone())]),
                        ..ResolvedSignature::returning(ResolvedType::Unknown)
                    },
                );
        }
        self.type_facts
            .entry(widget)
            .or_default()
            .constructors
            .insert(
                "new".to_string(),
                ResolvedSignature {
                    named: HashMap::from([("key".to_string(), key)]),
                    ..ResolvedSignature::returning(ResolvedType::Unknown)
                },
            );
        self.add_sdk_hierarchy();
        for (identity, name, arity) in super::identity::fallback_types() {
            let facts = self.type_facts.entry(identity).or_default();
            if facts.supertypes.is_none() {
                facts.fallback_type_parameters = Some(
                    (0..arity)
                        .map(|ordinal| sdk_parameter(name, ordinal))
                        .collect(),
                );
            }
        }
    }

    /// Seeds [`SDK_HIERARCHY`] plus the stream/file members whose results
    /// resource-tracking rules need to type (`Stream.listen`,
    /// `StreamController.stream`, `File.openWrite`).
    fn add_sdk_hierarchy(&mut self) {
        let arity = |identity: &DeclarationIdentity| {
            SDK_HIERARCHY
                .iter()
                .find(|entry| entry.semantic && identity.is_sdk(entry.library, entry.name))
                .map_or(0, |entry| entry.arity)
        };
        let sdk = |library: &'static str, name: &'static str, arguments: Vec<ResolvedType>| {
            ResolvedType::Interface {
                occurrence: Default::default(),
                identity: DeclarationIdentity::sdk(library, name),
                arguments: arguments.into(),
                nullable: false,
                extension_type: false,
            }
        };
        for entry in SDK_HIERARCHY.iter().filter(|entry| entry.semantic) {
            let parameters: Vec<_> = (0..entry.arity)
                .map(|ordinal| sdk_parameter(entry.name, ordinal))
                .collect();
            let resolved = entry
                .supertypes
                .iter()
                .map(|identity| {
                    let arguments = if entry.arity > 0 && arity(identity) == entry.arity {
                        parameters
                            .iter()
                            .map(|id| ResolvedType::TypeParameter {
                                id: id.clone(),
                                bound: Arc::new(ResolvedType::Dynamic),
                                nullable: false,
                            })
                            .collect()
                    } else if identity.is_sdk("dart:core", "Comparable") {
                        // Every listed implementer is `Comparable<Self>`.
                        vec![sdk(entry.library, entry.name, Vec::new())]
                    } else {
                        Vec::new()
                    };
                    ResolvedType::Interface {
                        occurrence: Default::default(),
                        identity: identity.clone(),
                        arguments: arguments.into(),
                        nullable: false,
                        extension_type: false,
                    }
                })
                .chain(
                    entry
                        .supertypes
                        .is_empty()
                        .then(|| sdk("dart:core", "Object", Vec::new())),
                )
                .collect();
            self.type_facts
                .entry(DeclarationIdentity::sdk(entry.library, entry.name))
                .or_default()
                .supertypes
                .get_or_insert((parameters, resolved));
        }
        let element = |owner: &'static str| ResolvedType::TypeParameter {
            id: sdk_parameter(owner, 0),
            bound: Arc::new(ResolvedType::Dynamic),
            nullable: false,
        };
        let listen = self
            .member_entries
            .entry(DeclarationIdentity::sdk("dart:async", "Stream"))
            .or_default()
            .entry("listen".to_string())
            .or_default();
        listen.is_instance_method = true;
        listen.signature = Some(ResolvedSignature {
            owner_parameters: vec![sdk_parameter("Stream", 0)],
            positional: vec![ResolvedType::Function {
                shape: Some(FunctionTypeShape::empty()),
                alias: FunctionTypeAlias::default(),
                optional_positional: 0,
                return_type: Arc::new(ResolvedType::Void),
                positional: vec![element("Stream")].into(),
                named: Vec::new().into(),
                nullable: true,
            }],
            ..ResolvedSignature::returning(sdk(
                "dart:async",
                "StreamSubscription",
                vec![element("Stream")],
            ))
        });
        self.member_entries
            .entry(DeclarationIdentity::sdk("dart:async", "StreamController"))
            .or_default()
            .entry("stream".to_string())
            .or_default()
            .read = Some(sdk(
            "dart:async",
            "Stream",
            vec![element("StreamController")],
        ));
        let open_write = self
            .member_entries
            .entry(DeclarationIdentity::sdk("dart:io", "File"))
            .or_default()
            .entry("openWrite".to_string())
            .or_default();
        open_write.is_instance_method = true;
        open_write.signature = Some(ResolvedSignature::returning(sdk(
            "dart:io",
            "IOSink",
            Vec::new(),
        )));
    }
}

/// `(library, name, members)`; each member is `(name, kind, positional
/// parameter names)`.
type SdkInterfaceMembersEntry = (
    &'static str,
    &'static str,
    &'static [(&'static str, MemberKind, &'static [&'static str])],
);

/// Complete instance-member lists of SDK interfaces (beyond `Object`) whose
/// members [`SignatureIndex::inherited_member_facts`] may report; every listed
/// member is abstract. Any other SDK supertype makes inherited facts unknown.
pub(super) const SDK_INTERFACE_MEMBERS: &[SdkInterfaceMembersEntry] = &[(
    "dart:core",
    "Type",
    &[
        ("hashCode", MemberKind::Getter, &[]),
        ("==", MemberKind::Operator, &["other"]),
        ("toString", MemberKind::Method, &[]),
    ],
)];

pub(super) fn sdk_parameter(owner: &'static str, ordinal: usize) -> TypeParameterId {
    TypeParameterId {
        owner: ParameterOwner::Sdk(owner),
        ordinal,
        name: format!("{owner}#{ordinal}").into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_hierarchy_projects_only_its_original_sdk_rows() {
        let mut signatures = SignatureIndex::default();
        signatures.add_sdk_hierarchy();
        assert_eq!(
            SDK_HIERARCHY.iter().filter(|entry| entry.semantic).count(),
            62
        );
        for entry in SDK_HIERARCHY {
            assert!(!entry.supertypes.contains(&DeclarationIdentity::OBJECT));
            let identity = DeclarationIdentity::sdk(entry.library, entry.name);
            let facts = signatures.type_facts.get(&identity);
            if !entry.semantic {
                assert!(
                    facts.is_none(),
                    "unexpected semantic graph for {identity:?}"
                );
                continue;
            }
            let (parameters, supertypes) = facts.unwrap().supertypes.as_ref().unwrap();
            assert_eq!(parameters.len(), entry.arity);
            let mut expected = entry.supertypes.to_vec();
            if expected.is_empty() {
                expected.push(DeclarationIdentity::OBJECT);
            }
            let actual: Vec<_> = supertypes
                .iter()
                .map(|ty| match ty {
                    ResolvedType::Interface { identity, .. } => identity.clone(),
                    _ => panic!("SDK supertype must be an interface"),
                })
                .collect();
            assert_eq!(actual, expected, "{identity:?}");
        }
        for (name, expected) in [
            ("String", &["Comparable", "Pattern"][..]),
            ("num", &["Comparable"][..]),
        ] {
            let identity = DeclarationIdentity::sdk("dart:core", name);
            let (_, supertypes) = signatures.type_facts[&identity]
                .supertypes
                .as_ref()
                .unwrap();
            assert_eq!(supertypes.len(), expected.len());
            for (ty, target) in supertypes.iter().zip(expected) {
                assert!(matches!(
                    ty,
                    ResolvedType::Interface { identity, .. }
                        if identity.is_sdk("dart:core", target)
                ));
            }
            assert!(matches!(
                &supertypes[0],
                ResolvedType::Interface { arguments, .. }
                    if matches!(arguments.as_ref(), [
                        ResolvedType::Interface { identity: argument, arguments, .. }
                    ] if argument == &identity && arguments.is_empty())
            ));
        }
    }

    #[test]
    fn semantic_sdk_edges_forward_generic_parameters() {
        let mut signatures = SignatureIndex::default();
        signatures.add_sdk_hierarchy();
        for (name, expected) in [
            ("EventSink", &[("dart:core", "Sink")][..]),
            (
                "StreamSink",
                &[
                    ("dart:async", "EventSink"),
                    ("dart:async", "StreamConsumer"),
                ][..],
            ),
        ] {
            let identity = DeclarationIdentity::sdk("dart:async", name);
            let (parameters, supertypes) = signatures.type_facts[&identity]
                .supertypes
                .as_ref()
                .unwrap();
            assert_eq!(parameters, &[sdk_parameter(name, 0)]);
            assert_eq!(supertypes.len(), expected.len());
            for (ty, (library, target)) in supertypes.iter().zip(expected) {
                assert!(matches!(
                    ty,
                    ResolvedType::Interface { identity, arguments, .. }
                        if identity.is_sdk(library, target)
                            && matches!(arguments.as_ref(), [
                                ResolvedType::TypeParameter { id, bound, nullable: false }
                            ] if id == &parameters[0]
                                && matches!(bound.as_ref(), ResolvedType::Dynamic))
                ));
            }
        }
    }

    #[test]
    fn shared_sdk_edges_do_not_override_source_or_duplicate_manual_collections() {
        let mut signatures = SignatureIndex::default();
        let string = DeclarationIdentity::sdk("dart:core", "String");
        signatures
            .type_facts
            .entry(string.clone())
            .or_default()
            .supertypes = Some((Vec::new(), Vec::new()));
        signatures.add_sdk_signatures();
        assert!(
            signatures
                .type_facts
                .get(&string)
                .unwrap()
                .supertypes
                .as_ref()
                .unwrap()
                .1
                .is_empty()
        );
        for name in ["List", "Set"] {
            let identity = DeclarationIdentity::sdk("dart:core", name);
            let (parameters, supertypes) = signatures
                .type_facts
                .get(&identity)
                .unwrap()
                .supertypes
                .as_ref()
                .unwrap();
            assert_eq!(parameters.len(), 1);
            assert_eq!(supertypes.len(), 1);
            assert!(matches!(
                &supertypes[0],
                ResolvedType::Interface { identity, arguments, .. }
                    if identity.is_sdk("dart:core", "Iterable") && arguments.len() == 1
            ));
        }
    }
}
