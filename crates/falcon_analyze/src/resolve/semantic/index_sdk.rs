use super::*;

impl SignatureIndex {
    pub(super) fn add_sdk_signatures(&mut self) {
        for name in ["TestOn", "Timeout", "Tags", "OnPlatform"] {
            self.declarations.insert(
                DeclarationIdentity::package("test", name),
                DeclarationFacts {
                    annotations: Vec::new(),
                    library_target: true,
                    constructors: Vec::new(),
                    static_consts: Vec::new(),
                    extension_representation: None,
                    is_abstract: false,
                    has_subclass_in_library: false,
                },
            );
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
            self.supertypes.insert(
                identity.clone(),
                (vec![parameter_id], vec![supertype.clone()]),
            );
            self.member_supertypes.insert(identity, vec![supertype]);
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
            self.supertypes
                .insert(identity.clone(), (Vec::new(), vec![supertype.clone()]));
            self.superclasses
                .insert(identity.clone(), supertype.clone());
            self.member_supertypes
                .insert(identity.clone(), vec![supertype]);
            self.constructors.entry(identity).or_default().insert(
                "new".to_string(),
                ResolvedSignature {
                    named: HashMap::from([("key".to_string(), key.clone())]),
                    ..ResolvedSignature::returning(ResolvedType::Unknown)
                },
            );
        }
        self.constructors.entry(widget).or_default().insert(
            "new".to_string(),
            ResolvedSignature {
                named: HashMap::from([("key".to_string(), key)]),
                ..ResolvedSignature::returning(ResolvedType::Unknown)
            },
        );
        self.add_sdk_hierarchy();
        for (identity, name, arity) in super::identity::fallback_types() {
            if !self.supertypes.contains_key(&identity) {
                self.fallback_type_parameters.insert(
                    identity,
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
        let arity = |library: &str, name: &str| {
            SDK_HIERARCHY
                .iter()
                .find(|entry| entry.0 == library && entry.1 == name)
                .map_or(0, |entry| entry.2)
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
        for &(library, name, own_arity, supertypes) in SDK_HIERARCHY {
            let parameters: Vec<_> = (0..own_arity)
                .map(|ordinal| sdk_parameter(name, ordinal))
                .collect();
            let resolved = supertypes
                .iter()
                .map(|&(super_library, super_name)| {
                    let arguments =
                        if own_arity > 0 && arity(super_library, super_name) == own_arity {
                            parameters
                                .iter()
                                .map(|id| ResolvedType::TypeParameter {
                                    id: id.clone(),
                                    bound: Arc::new(ResolvedType::Dynamic),
                                    nullable: false,
                                })
                                .collect()
                        } else if (super_library, super_name) == ("dart:core", "Comparable") {
                            // Every listed implementer is `Comparable<Self>`.
                            vec![sdk(library, name, Vec::new())]
                        } else {
                            Vec::new()
                        };
                    sdk(super_library, super_name, arguments)
                })
                .chain(
                    supertypes
                        .is_empty()
                        .then(|| sdk("dart:core", "Object", Vec::new())),
                )
                .collect();
            self.supertypes
                .entry(DeclarationIdentity::sdk(library, name))
                .or_insert((parameters, resolved));
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

/// `(library, name, arity, supertypes)`; each supertype is `(library, name)`.
type SdkHierarchyEntry = (
    &'static str,
    &'static str,
    usize,
    &'static [(&'static str, &'static str)],
);

/// Curated SDK supertype graph: `(library, name, arity, supertypes)`.
///
/// Each entry lists every public supertype of its declaration other than
/// `Object`; `add_sdk_hierarchy` supplies that implicit root for leaf entries.
/// Generic supertypes with the same arity receive the declaration's own
/// type parameters; others are recorded raw.
pub(super) const SDK_HIERARCHY: &[SdkHierarchyEntry] = &[
    ("dart:core", "Function", 0, &[]),
    ("dart:core", "Record", 0, &[]),
    ("dart:core", "Type", 0, &[]),
    ("dart:core", "Symbol", 0, &[]),
    ("dart:core", "bool", 0, &[]),
    ("dart:core", "Comparable", 1, &[]),
    ("dart:core", "Pattern", 0, &[]),
    ("dart:core", "num", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "int", 0, &[("dart:core", "num")]),
    ("dart:core", "double", 0, &[("dart:core", "num")]),
    (
        "dart:core",
        "String",
        0,
        &[("dart:core", "Comparable"), ("dart:core", "Pattern")],
    ),
    ("dart:core", "RegExp", 0, &[("dart:core", "Pattern")]),
    ("dart:core", "BigInt", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "Duration", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "DateTime", 0, &[("dart:core", "Comparable")]),
    ("dart:core", "Uri", 0, &[]),
    ("dart:core", "StackTrace", 0, &[]),
    ("dart:core", "Enum", 0, &[]),
    ("dart:core", "Iterable", 1, &[]),
    ("dart:core", "Iterator", 1, &[]),
    ("dart:core", "Map", 2, &[]),
    ("dart:core", "MapEntry", 2, &[]),
    ("dart:core", "Exception", 0, &[]),
    (
        "dart:core",
        "FormatException",
        0,
        &[("dart:core", "Exception")],
    ),
    (
        "dart:core",
        "IntegerDivisionByZeroException",
        0,
        &[
            ("dart:core", "Exception"),
            ("dart:core", "UnsupportedError"),
        ],
    ),
    ("dart:core", "Error", 0, &[]),
    ("dart:core", "AssertionError", 0, &[("dart:core", "Error")]),
    ("dart:core", "TypeError", 0, &[("dart:core", "Error")]),
    ("dart:core", "ArgumentError", 0, &[("dart:core", "Error")]),
    (
        "dart:core",
        "RangeError",
        0,
        &[("dart:core", "ArgumentError")],
    ),
    (
        "dart:core",
        "IndexError",
        0,
        &[("dart:core", "ArgumentError"), ("dart:core", "RangeError")],
    ),
    ("dart:core", "StateError", 0, &[("dart:core", "Error")]),
    (
        "dart:core",
        "UnsupportedError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "UnimplementedError",
        0,
        &[("dart:core", "UnsupportedError")],
    ),
    (
        "dart:core",
        "ConcurrentModificationError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "OutOfMemoryError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "StackOverflowError",
        0,
        &[("dart:core", "Error")],
    ),
    (
        "dart:core",
        "NoSuchMethodError",
        0,
        &[("dart:core", "Error")],
    ),
    ("dart:core", "Sink", 1, &[]),
    ("dart:core", "StringSink", 0, &[]),
    ("dart:async", "Future", 1, &[]),
    ("dart:async", "Completer", 1, &[]),
    ("dart:async", "Timer", 0, &[]),
    ("dart:async", "Stream", 1, &[]),
    ("dart:async", "StreamSubscription", 1, &[]),
    ("dart:async", "EventSink", 1, &[("dart:core", "Sink")]),
    ("dart:async", "StreamConsumer", 1, &[]),
    (
        "dart:async",
        "StreamSink",
        1,
        &[
            ("dart:async", "EventSink"),
            ("dart:async", "StreamConsumer"),
        ],
    ),
    (
        "dart:async",
        "StreamController",
        1,
        &[("dart:async", "StreamSink")],
    ),
    (
        "dart:async",
        "TimeoutException",
        0,
        &[("dart:core", "Exception")],
    ),
    ("dart:async", "AsyncError", 0, &[("dart:core", "Error")]),
    ("dart:io", "FileSystemEntity", 0, &[]),
    ("dart:io", "File", 0, &[("dart:io", "FileSystemEntity")]),
    (
        "dart:io",
        "Directory",
        0,
        &[("dart:io", "FileSystemEntity")],
    ),
    ("dart:io", "Link", 0, &[("dart:io", "FileSystemEntity")]),
    (
        "dart:io",
        "IOSink",
        0,
        &[("dart:async", "StreamSink"), ("dart:core", "StringSink")],
    ),
    (
        "dart:io",
        "Socket",
        0,
        &[("dart:async", "Stream"), ("dart:io", "IOSink")],
    ),
    ("dart:io", "IOException", 0, &[("dart:core", "Exception")]),
    ("dart:io", "OSError", 0, &[("dart:core", "Exception")]),
    (
        "dart:io",
        "FileSystemException",
        0,
        &[("dart:io", "IOException")],
    ),
    ("dart:io", "HttpException", 0, &[("dart:io", "IOException")]),
    (
        "dart:io",
        "SocketException",
        0,
        &[("dart:io", "IOException")],
    ),
];

pub(super) fn sdk_parameter(owner: &'static str, ordinal: usize) -> TypeParameterId {
    TypeParameterId {
        owner: ParameterOwner::Sdk(owner),
        ordinal,
        name: format!("{owner}#{ordinal}").into(),
    }
}
