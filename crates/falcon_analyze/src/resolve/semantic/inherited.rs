//! Inherited interface lookup selects an existing signature that is a subtype
//! of every candidate, not merely any declaration with the same name. In
//! particular, a class's own valid override cannot repair this inherited lookup.
//! Implicit Object edges used for subtyping are not extra inherited candidates:
//! only the effective superclass/constraints, mixins, and interfaces participate.
//! An unavailable signature remains uncertain through combination, rather than
//! allowing a known candidate to hide it from downstream interfaces.

use super::*;

#[derive(Debug, Clone, Default)]
pub(super) struct OverrideShape {
    required_positional: usize,
    required_named: FxHashSet<String>,
    bounds: Vec<ResolvedType>,
}

impl OverrideShape {
    pub(super) fn new(
        params: &FormalParamList,
        type_params: &[TypeParam],
        scope: &TypeParameterScope,
    ) -> Self {
        Self {
            required_positional: params
                .positional()
                .filter(|param| !matches!(param.kind, ParamKind::OptionalPositional))
                .count(),
            required_named: params
                .named()
                .filter(|param| matches!(param.kind, ParamKind::Named { required: true }))
                .map(|param| param.name.name.clone())
                .collect(),
            bounds: type_params
                .iter()
                .map(|param| match scope.lookup(&param.name.name) {
                    Some(ResolvedType::TypeParameter { bound, .. }) => bound.as_ref().clone(),
                    _ => ResolvedType::Unknown,
                })
                .collect(),
        }
    }
}

#[derive(Clone)]
struct InheritedMember {
    declaring_type: DeclarationIdentity,
    kind: MemberKind,
    positional: Vec<ResolvedType>,
    named: FxHashMap<String, ResolvedType>,
    return_type: ResolvedType,
    call_parameters: Vec<TypeParameterId>,
    shape: OverrideShape,
    known: bool,
}

#[derive(Default)]
pub(super) struct OverrideInferenceLookup {
    visiting: FxHashSet<DeclarationIdentity>,
    memo: FxHashMap<DeclarationIdentity, Vec<InheritedMember>>,
}

impl SignatureIndex {
    /// The return type of an overridden getter/field or method on an already
    /// instantiated supertype. `None` means no resolved declaration, whereas
    /// `Some(Unknown)` retains a declaration whose return type is not known.
    pub fn instantiated_override_return(
        &self,
        receiver: &ResolvedType,
        name: &str,
        getter: bool,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        if getter {
            let (ty, substitutions) = self.resolved_field(receiver, name, model)?;
            Some(model.substitute(&ty, &substitutions))
        } else {
            let (signature, substitutions) = self.resolved_member(receiver, name, model)?;
            Some(model.substitute(&signature.return_type, &substitutions))
        }
    }

    /// Select the same combined interface signature used by override
    /// inference, then recover its declaring type's canonical instantiation.
    pub(super) fn combined_instance_owner(
        &self,
        receiver: &ResolvedType,
        name: &str,
        access: InstanceAccess,
        model: &SemanticModel<'_>,
    ) -> Option<ResolvedType> {
        let mut receiver = receiver;
        while let ResolvedType::TypeParameter { bound, .. }
        | ResolvedType::NonNullTypeParameter { bound, .. } = receiver
        {
            receiver = bound;
        }
        let ResolvedType::Interface {
            identity,
            arguments,
            occurrence,
            ..
        } = receiver
        else {
            return None;
        };
        let candidates = self.inherited_candidates(
            identity,
            name,
            matches!(access, InstanceAccess::Write),
            model,
            &mut FxHashSet::from_iter([identity.clone()]),
            &mut FxHashMap::default(),
        );
        let candidates =
            self.substitute_inherited_members(candidates, identity, arguments, occurrence, model);
        let member = self
            .combine_inherited(candidates, model)
            .into_iter()
            .next()?;
        if !member.known
            || (matches!(access, InstanceAccess::Invoke)
                && !matches!(member.kind, MemberKind::Method | MemberKind::Operator))
        {
            return None;
        }
        self.instantiated_identity(receiver, &member.declaring_type, model)
    }

    /// Missing declarations make an inherited signature uncertain only on paths
    /// that have not already supplied a nearer declaration of this member.
    pub(super) fn override_inference_unresolved(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        setter: bool,
        model: &SemanticModel<'_>,
        lookup: &mut OverrideInferenceLookup,
    ) -> bool {
        lookup.visiting.clear();
        lookup.visiting.insert(identity.clone());
        self.supertypes
            .get(identity)
            .is_some_and(|(_, supertypes)| {
                supertypes.iter().any(|supertype| {
                    self.override_graph_unresolved(
                        supertype,
                        name,
                        setter,
                        model,
                        &mut lookup.visiting,
                    )
                })
            })
    }

    fn override_graph_unresolved(
        &self,
        supertype: &ResolvedType,
        name: &str,
        setter: bool,
        model: &SemanticModel<'_>,
        visiting: &mut FxHashSet<DeclarationIdentity>,
    ) -> bool {
        let ResolvedType::Interface { identity, .. } = supertype else {
            return true;
        };
        if identity == &DeclarationIdentity::OBJECT || !visiting.insert(identity.clone()) {
            return false;
        }
        let visible = !name.starts_with('_')
            || match identity {
                DeclarationIdentity::Project { library, .. } => model.library == Some(*library),
                _ => false,
            };
        if visible
            && self.member_entry(identity, name).is_some_and(|entry| {
                entry.facts.iter().any(|fact| {
                    !fact.is_static
                        && if setter {
                            fact.kind.has_setter()
                        } else {
                            fact.kind != MemberKind::Setter
                        }
                })
            })
        {
            return false;
        }
        if let DeclarationIdentity::Sdk { library, name } = identity
            && self.declaration(identity).is_none()
            && !SDK_INTERFACE_MEMBERS
                .iter()
                .any(|entry| entry.0 == library.as_str() && entry.1 == name.as_str())
        {
            return true;
        }
        let Some((_, supertypes)) = self.supertypes.get(identity) else {
            return !self.member_entries.contains_key(identity);
        };
        supertypes.iter().any(|supertype| {
            self.override_graph_unresolved(supertype, name, setter, model, visiting)
        })
    }

    /// Native combined interface signature used by declaration override inference.
    /// Extension methods are deliberately absent from this lookup.
    pub(super) fn override_inference_signature(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        setter: bool,
        model: &SemanticModel<'_>,
        lookup: &mut OverrideInferenceLookup,
    ) -> Option<(
        Vec<ResolvedType>,
        FxHashMap<String, ResolvedType>,
        ResolvedType,
    )> {
        lookup.visiting.clear();
        lookup.memo.clear();
        lookup.visiting.insert(identity.clone());
        let candidates = self.inherited_candidates(
            identity,
            name,
            setter,
            model,
            &mut lookup.visiting,
            &mut lookup.memo,
        );
        let getter = setter && candidates.is_empty();
        let candidates = if getter {
            lookup.memo.clear();
            self.inherited_candidates(
                identity,
                name,
                false,
                model,
                &mut lookup.visiting,
                &mut lookup.memo,
            )
        } else {
            candidates
        };
        let member = self
            .combine_inherited(candidates, model)
            .into_iter()
            .next()?;
        if !member.known {
            let signature = self.member_entry(identity, name)?.signature.as_ref();
            return Some((
                signature.map_or_else(
                    || vec![ResolvedType::Unknown],
                    |signature| vec![ResolvedType::Unknown; signature.positional.len()],
                ),
                signature.map_or_else(FxHashMap::default, |signature| {
                    signature
                        .named
                        .keys()
                        .map(|name| (name.clone(), ResolvedType::Unknown))
                        .collect()
                }),
                ResolvedType::Unknown,
            ));
        }
        if getter {
            return member.kind.has_getter().then(|| {
                (
                    vec![member.return_type],
                    FxHashMap::default(),
                    ResolvedType::Void,
                )
            });
        }
        let entry = self.member_entry(identity, name)?;
        let parameters = entry
            .signature
            .as_ref()
            .map_or(&[][..], |signature| signature.call_parameters.as_slice());
        if parameters.len() != member.call_parameters.len() {
            return None;
        }
        if parameters.is_empty() {
            return Some((member.positional, member.named, member.return_type));
        }
        let substitutions = member
            .call_parameters
            .iter()
            .zip(parameters)
            .enumerate()
            .map(|(index, (from, to))| {
                (
                    from.clone(),
                    ResolvedType::TypeParameter {
                        id: to.clone(),
                        bound: Arc::new(
                            entry
                                .override_shape
                                .as_ref()
                                .and_then(|shape| shape.bounds.get(index))
                                .cloned()
                                .unwrap_or(ResolvedType::Unknown),
                        ),
                        nullable: false,
                    },
                )
            })
            .collect();
        Some((
            member
                .positional
                .iter()
                .map(|ty| model.substitute(ty, &substitutions))
                .collect(),
            member
                .named
                .iter()
                .map(|(name, ty)| (name.clone(), model.substitute(ty, &substitutions)))
                .collect(),
            model.substitute(&member.return_type, &substitutions),
        ))
    }

    /// Whether the analyzer's combined inherited interface contains this member.
    /// Getter and method conflicts, or signatures with no most-specific existing
    /// candidate, do not produce an inherited member even if the owner declares
    /// a signature that validly overrides all of them.
    pub fn has_combined_inherited_member(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        kind: MemberKind,
        model: &SemanticModel<'_>,
    ) -> bool {
        let setter = kind == MemberKind::Setter;
        let mut visiting = FxHashSet::default();
        visiting.insert(identity.clone());
        // Name, setter mode, and library visibility are fixed for this lookup.
        // Cache before owner substitution so distinct generic paths stay distinct.
        let mut memo = FxHashMap::default();
        let candidates =
            self.inherited_candidates(identity, name, setter, model, &mut visiting, &mut memo);
        self.combine_inherited(candidates, model)
            .iter()
            .any(|member| match kind {
                MemberKind::Method | MemberKind::Operator => member.kind == kind,
                MemberKind::Setter => member.kind.has_setter(),
                _ => member.kind.has_getter(),
            })
    }

    fn inherited_candidates(
        &self,
        identity: &DeclarationIdentity,
        name: &str,
        setter: bool,
        model: &SemanticModel<'_>,
        visiting: &mut FxHashSet<DeclarationIdentity>,
        memo: &mut FxHashMap<DeclarationIdentity, Vec<InheritedMember>>,
    ) -> Vec<InheritedMember> {
        let Some((_, supertypes)) = self.supertypes.get(identity) else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        // Class-like declarations append an implicit Object edge; extension
        // types retain only explicit superinterfaces. InheritanceManager3 starts
        // with the effective superclass (or mixin constraints), then applies
        // mixins and interfaces; Object is not an extra candidate.
        let supertypes = if identity != &DeclarationIdentity::OBJECT
            && self.type_kind(identity) != Some(TypeKind::ExtensionType)
        {
            if let Some(&inherited_object) = self.inherited_object.get(identity) {
                let (object, explicit) = supertypes.split_last().expect("implicit Object edge");
                if inherited_object {
                    candidates.extend(
                        self.interface_candidates(object, name, setter, model, visiting, memo),
                    );
                }
                explicit
            } else {
                supertypes.as_slice()
            }
        } else {
            supertypes.as_slice()
        };
        for (index, supertype) in supertypes.iter().enumerate() {
            let inherited =
                self.interface_candidates(supertype, name, setter, model, visiting, memo);
            // A declared mixin member replaces the superclass signature before
            // implemented interfaces are combined (InheritanceManager3).
            if self
                .mixin_applications
                .get(identity)
                .is_some_and(|range| range.contains(&index))
                && inherited.iter().any(|member| {
                    matches!(supertype, ResolvedType::Interface { identity, .. }
                        if member.declaring_type == *identity)
                })
            {
                candidates.clear();
            }
            candidates.extend(inherited);
        }
        candidates
    }

    fn interface_candidates(
        &self,
        ty: &ResolvedType,
        name: &str,
        setter: bool,
        model: &SemanticModel<'_>,
        visiting: &mut FxHashSet<DeclarationIdentity>,
        memo: &mut FxHashMap<DeclarationIdentity, Vec<InheritedMember>>,
    ) -> Vec<InheritedMember> {
        let ResolvedType::Interface {
            identity,
            arguments,
            occurrence,
            ..
        } = ty
        else {
            return Vec::new();
        };
        if let Some(members) = memo.get(identity) {
            return self.substitute_inherited_members(
                members.clone(),
                identity,
                arguments,
                occurrence,
                model,
            );
        }
        if !visiting.insert(identity.clone()) {
            return Vec::new();
        }
        let mut members = Vec::new();
        let visible = !name.starts_with('_')
            || match identity {
                DeclarationIdentity::Project { library, .. } => model.library == Some(*library),
                _ => false,
            };
        if visible {
            if let Some(entry) = self.member_entry(identity, name) {
                members.reserve(entry.facts.len());
                for fact in entry.facts.iter().filter(|fact| {
                    !fact.is_static
                        && if setter {
                            fact.kind.has_setter()
                        } else {
                            fact.kind != MemberKind::Setter
                        }
                }) {
                    let signature = match fact.kind {
                        MemberKind::Method | MemberKind::Operator => entry.signature.as_ref(),
                        _ => None,
                    };
                    let mut shape = entry.override_shape.clone().unwrap_or_default();
                    let (positional, named, return_type, call_parameters, known) =
                        if let Some(signature) = signature {
                            (
                                signature.positional.clone(),
                                signature
                                    .named
                                    .iter()
                                    .map(|(name, ty)| (name.clone(), ty.clone()))
                                    .collect(),
                                signature.return_type.clone(),
                                signature.call_parameters.clone(),
                                entry.override_shape.is_some(),
                            )
                        } else if setter {
                            shape.required_positional = 1;
                            (
                                vec![entry.write.clone().unwrap_or(ResolvedType::Unknown)],
                                FxHashMap::default(),
                                ResolvedType::Void,
                                Vec::new(),
                                true,
                            )
                        } else {
                            (
                                Vec::new(),
                                FxHashMap::default(),
                                entry.read.clone().unwrap_or(ResolvedType::Unknown),
                                Vec::new(),
                                !matches!(fact.kind, MemberKind::Method | MemberKind::Operator),
                            )
                        };
                    members.push(InheritedMember {
                        declaring_type: identity.clone(),
                        kind: fact.kind,
                        positional,
                        named,
                        return_type,
                        call_parameters,
                        shape,
                        known,
                    });
                }
            } else if let DeclarationIdentity::Sdk {
                library,
                name: type_name,
            } = identity
            {
                // Reuse the existing SDK-free declaration facts, without inventing
                // signature types for unloaded libraries.
                if let Some((.., facts)) = SDK_INTERFACE_MEMBERS
                    .iter()
                    .find(|entry| entry.0 == library.as_str() && entry.1 == type_name.as_str())
                {
                    for &(_, kind, _) in facts.iter().filter(|fact| {
                        fact.0 == name
                            && if setter {
                                fact.1.has_setter()
                            } else {
                                fact.1 != MemberKind::Setter
                            }
                    }) {
                        members.push(InheritedMember {
                            declaring_type: identity.clone(),
                            kind,
                            positional: Vec::new(),
                            named: FxHashMap::default(),
                            return_type: ResolvedType::Unknown,
                            call_parameters: Vec::new(),
                            shape: OverrideShape::default(),
                            known: false,
                        });
                    }
                }
            }
        }
        if members.is_empty() {
            let inherited =
                self.inherited_candidates(identity, name, setter, model, visiting, memo);
            members = self.combine_inherited(inherited, model);
        }
        visiting.remove(identity);
        // A cycle-truncated result depends on the current path, so only memoize
        // complete acyclic interfaces. Completed indexes carry this graph fact.
        if self
            .cyclic_supertypes
            .as_ref()
            .is_some_and(|cyclic| !cyclic.contains(identity))
        {
            memo.insert(identity.clone(), members.clone());
        }
        self.substitute_inherited_members(members, identity, arguments, occurrence, model)
    }

    fn substitute_inherited_members(
        &self,
        mut members: Vec<InheritedMember>,
        identity: &DeclarationIdentity,
        arguments: &[ResolvedType],
        occurrence: &FunctionTypeAlias,
        model: &SemanticModel<'_>,
    ) -> Vec<InheritedMember> {
        if members.is_empty() {
            return members;
        }
        let substitutions = self
            .supertypes
            .get(identity)
            .map(|(params, _)| self.owner_substitutions(params, arguments, Some(occurrence)));
        if let Some(substitutions) = substitutions.filter(|substitutions| !substitutions.is_empty())
        {
            for member in &mut members {
                member.return_type = model.substitute(&member.return_type, &substitutions);
                for ty in member
                    .positional
                    .iter_mut()
                    .chain(member.named.values_mut())
                    .chain(&mut member.shape.bounds)
                {
                    *ty = model.substitute(ty, &substitutions);
                }
            }
        }
        members
    }

    fn combine_inherited(
        &self,
        candidates: Vec<InheritedMember>,
        model: &SemanticModel<'_>,
    ) -> Vec<InheritedMember> {
        if candidates.len() < 2 {
            return candidates;
        }
        let method = |kind| matches!(kind, MemberKind::Method | MemberKind::Operator);
        if candidates
            .iter()
            .any(|member| method(member.kind) != method(candidates[0].kind))
        {
            return Vec::new();
        }
        // An interface exposes one combined signature, not every equivalent
        // path to it. Keeping all equivalent candidates grows exponentially
        // across repeated diamonds even when the traversal itself is memoized.
        // Unknown signatures compare compatibly with every same-kind candidate.
        // Prefer one so later combinations cannot turn that uncertainty into a
        // conflict merely because a known candidate happened to come first.
        let selected = candidates
            .iter()
            .position(|candidate| !candidate.known)
            .or_else(|| {
                candidates.iter().position(|candidate| {
                    candidates
                        .iter()
                        .all(|other| self.inherited_signature_subtype(candidate, other, model))
                })
            });
        selected
            .and_then(|index| candidates.into_iter().nth(index))
            .into_iter()
            .collect()
    }

    fn inherited_signature_subtype(
        &self,
        sub: &InheritedMember,
        sup: &InheritedMember,
        model: &SemanticModel<'_>,
    ) -> bool {
        if std::ptr::eq(sub, sup) || !sub.known || !sup.known {
            return true;
        }
        if sub.call_parameters.len() != sup.call_parameters.len()
            || sub.shape.required_positional > sup.shape.required_positional
            || sub.positional.len() < sup.positional.len()
            || !sub
                .shape
                .required_named
                .is_subset(&sup.shape.required_named)
            || !sup.named.keys().all(|name| sub.named.contains_key(name))
        {
            return false;
        }
        let compatible = |left: &ResolvedType, right: &ResolvedType| {
            self.subtype(left, right, model) != Truth::No
        };
        // In function subtyping, a void result accepts any result type.
        let return_compatible = |left: &ResolvedType, right: &ResolvedType| {
            matches!(right, ResolvedType::Void) || compatible(left, right)
        };
        if sub.call_parameters.is_empty() {
            return return_compatible(&sub.return_type, &sup.return_type)
                && sub
                    .positional
                    .iter()
                    .zip(&sup.positional)
                    .all(|(sub, sup)| compatible(sup, sub))
                && sup
                    .named
                    .iter()
                    .all(|(name, sup)| compatible(sup, &sub.named[name]));
        }
        // Generic signatures are compared after alpha-renaming their method
        // parameters; owner parameters were substituted along each graph edge.
        let substitutions = sub
            .call_parameters
            .iter()
            .zip(&sup.call_parameters)
            .enumerate()
            .map(|(index, (from, to))| {
                (
                    from.clone(),
                    ResolvedType::TypeParameter {
                        id: to.clone(),
                        bound: Arc::new(
                            sup.shape
                                .bounds
                                .get(index)
                                .cloned()
                                .unwrap_or(ResolvedType::Unknown),
                        ),
                        nullable: false,
                    },
                )
            })
            .collect();
        for (left, right) in sub.shape.bounds.iter().zip(&sup.shape.bounds) {
            let left = model.substitute(left, &substitutions);
            if !compatible(&left, right) || !compatible(right, &left) {
                return false;
            }
        }
        return_compatible(
            &model.substitute(&sub.return_type, &substitutions),
            &sup.return_type,
        ) && sub
            .positional
            .iter()
            .zip(&sup.positional)
            .all(|(sub, sup)| compatible(sup, &model.substitute(sub, &substitutions)))
            && sup.named.iter().all(|(name, sup)| {
                compatible(sup, &model.substitute(&sub.named[name], &substitutions))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolve::IdentitySource;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    #[test]
    fn extension_type_inheritance_uses_only_explicit_superinterfaces() {
        let path = PathBuf::from("/project/lib/main.dart");
        let (program, errors) = parse(
            "extension type Nullable(Object? value) {}
             extension type Described(Object? value) implements Nullable {
               String describe() => '';
             }
             extension type Child(Object? value) implements Described {}",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let index =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let model = SemanticModel::new(&path, &identities, Some(&types));
        let nullable = model.resolve_name("Nullable").unwrap();
        assert!(index.supertypes[&nullable].1.is_empty());
        assert!(!index.has_combined_inherited_member(
            &nullable,
            "toString",
            MemberKind::Method,
            &model,
        ));
        assert!(index.has_combined_inherited_member(
            &model.resolve_name("Child").unwrap(),
            "describe",
            MemberKind::Method,
            &model,
        ));
    }

    #[test]
    fn override_headers_settle_across_files_before_initializers() {
        let early_path = PathBuf::from("/project/lib/early.dart");
        let base_path = PathBuf::from("/project/lib/base.dart");
        let (early, errors) = parse(
            "import 'base.dart'; final top = Early().measured;
             class Early extends Middle {
               late final measured = measure(''); measure(text) => 0;
             }
             class FieldSetter extends FieldBase { set value(next) {} }",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let (base, errors) = parse(
            "abstract class Middle extends Base { measure(text); }
             abstract class Base { int measure(String text); }
             class FieldBase { var value = 0; }",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[
                IdentitySource {
                    path: &early_path,
                    program: &early,
                    has_parse_errors: false,
                },
                IdentitySource {
                    path: &base_path,
                    program: &base,
                    has_parse_errors: false,
                },
            ],
            &[],
        );
        let types = TypeIndex::from_program(&early);
        for files in [
            vec![(early_path.clone(), &early), (base_path.clone(), &base)],
            vec![(base_path.clone(), &base), (early_path.clone(), &early)],
        ] {
            let index = SignatureIndex::from_project_files(&files, &identities, &types);
            let model = SemanticModel::new(&early_path, &identities, Some(&types));
            let owner = model.resolve_name("Early").unwrap();
            assert_eq!(
                index.field(&owner, "measured").unwrap().1,
                model.core_type("int", false)
            );
            assert_eq!(
                index.value_type(&model.resolve_value("top").unwrap()),
                Some(&model.core_type("int", false)),
            );
            let setter = model.resolve_name("FieldSetter").unwrap();
            assert_eq!(
                index.member_entry(&setter, "value").unwrap().write,
                Some(model.core_type("int", false)),
            );
        }
    }

    #[test]
    fn override_inference_precedes_initializer_inference() {
        let path = PathBuf::from("/project/lib/main.dart");
        let source = "abstract class A { Object m(); void take(int x); T echo<T>(T x); set value(int x); int operator +(int x); }
            abstract class B { int m(); void take(num x); }
            final top = D().y;
            class D extends Middle { late final y = measure(''); measure(t) => 0; }
            abstract class Middle extends Base { measure(t); }
            abstract class Base { int measure(String text); }
            class C implements A, B {
              m() => 1; take(x) {} echo<U>(x) => x;
              set value(x) {} operator +(x) => x;
            }
            extension Describe on Object { String describe(int x) => ''; }
            class E { describe(x) => 1; }
            class GetterBase { int get value => 0; }
            class GetterSetter extends GetterBase { set value(next) {} }
            class FieldSetter extends FieldBase { set value(next) {} }
            class FieldBase { var value = 0; }
            class Unavailable extends MissingBase { method(value) {} }
            abstract class PartialBase { MissingResult method(int value); }
            class Partial extends PartialBase { method(value) {} }
            class KnownBase { void mixed(int value) {} }
            class Mixed extends KnownBase implements MissingInterface { mixed(value) {} }
            abstract class Repo extends MissingNotifier { void load(String id); }
            class ApiRepo extends Repo { load(id) {} }
            abstract class UnusedGenericBase { void run<T>(int value); }
            class UnusedGeneric extends UnusedGenericBase { run<U>(value) {} }
            abstract class IntBase { int conflict(); }
            class Left implements IntBase { conflict() => 0; }
            abstract class Conflict implements Left, Right { conflict(); }
            class Right implements StringBase { conflict() => ''; }
            abstract class StringBase { String conflict(); }
            class GenericEmpty<T> {}
            class UnknownArgument extends GenericEmpty<Missing> { helper(value) {} }
            class SdkChild { helper(value) {} }
            class Version implements Comparable<Version> {
              final int major; Version(this.major);
              compareTo(other) => major - other.major;
            }";
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let mut index =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let model = SemanticModel::new(&path, &identities, Some(&types));
        let c = model.resolve_name("C").unwrap();
        assert_eq!(
            index
                .member_entry(&c, "m")
                .unwrap()
                .signature
                .as_ref()
                .unwrap()
                .return_type,
            model.core_type("int", false)
        );
        assert_eq!(
            index
                .member_entry(&c, "take")
                .unwrap()
                .signature
                .as_ref()
                .unwrap()
                .positional[0],
            model.core_type("num", false)
        );
        let echo = index
            .member_entry(&c, "echo")
            .unwrap()
            .signature
            .as_ref()
            .unwrap();
        assert!(
            matches!(&echo.positional[0], ResolvedType::TypeParameter { id, .. } if *id == echo.call_parameters[0])
        );
        assert_eq!(echo.return_type, echo.positional[0]);
        assert_eq!(
            index.member_entry(&c, "value").unwrap().write,
            Some(model.core_type("int", false))
        );
        assert_eq!(
            index
                .member_entry(&c, "+")
                .unwrap()
                .signature
                .as_ref()
                .unwrap()
                .positional[0],
            model.core_type("int", false)
        );
        assert_eq!(
            index
                .member_entry(&c, "+")
                .unwrap()
                .signature
                .as_ref()
                .unwrap()
                .return_type,
            model.core_type("int", false)
        );
        let d = model.resolve_name("D").unwrap();
        assert_eq!(
            index.field(&d, "y").unwrap().1,
            model.core_type("int", false)
        );
        assert_eq!(
            index.value_type(&model.resolve_value("top").unwrap()),
            Some(&model.core_type("int", false))
        );
        let e = model.resolve_name("E").unwrap();
        assert_eq!(
            index
                .member_entry(&e, "describe")
                .unwrap()
                .signature
                .as_ref()
                .unwrap()
                .positional[0],
            ResolvedType::Dynamic
        );
        for name in ["GetterSetter", "FieldSetter"] {
            let owner = model.resolve_name(name).unwrap();
            assert_eq!(
                index.member_entry(&owner, "value").unwrap().write,
                Some(model.core_type("int", false))
            );
        }
        let unavailable = model.resolve_name("Unavailable").unwrap();
        assert_eq!(
            index.member(&unavailable, "method").unwrap().positional[0],
            ResolvedType::Unknown
        );
        let partial = model.resolve_name("Partial").unwrap();
        assert_eq!(
            index.member(&partial, "method").unwrap().positional[0],
            model.core_type("int", false)
        );
        let unused = model.resolve_name("UnusedGeneric").unwrap();
        assert_eq!(
            index.member(&unused, "run").unwrap().positional[0],
            model.core_type("int", false)
        );
        let conflict = model.resolve_name("Conflict").unwrap();
        assert_eq!(
            index.member(&conflict, "conflict").unwrap().return_type,
            ResolvedType::Dynamic
        );
        let unknown_argument = model.resolve_name("UnknownArgument").unwrap();
        assert_eq!(
            index
                .member(&unknown_argument, "helper")
                .unwrap()
                .positional[0],
            ResolvedType::Dynamic
        );
        let version = model.resolve_name("Version").unwrap();
        assert_eq!(
            index.member(&version, "compareTo").unwrap().positional[0],
            ResolvedType::Unknown
        );
        let mixed = model.resolve_name("Mixed").unwrap();
        assert_eq!(
            index.member(&mixed, "mixed").unwrap().positional[0],
            ResolvedType::Unknown
        );
        let api_repo = model.resolve_name("ApiRepo").unwrap();
        assert_eq!(
            index.member(&api_repo, "load").unwrap().positional[0],
            model.core_type("String", false)
        );
        let sdk_child = model.resolve_name("SdkChild").unwrap();
        index.supertypes.insert(
            sdk_child.clone(),
            (
                Vec::new(),
                vec![ResolvedType::Interface {
                    identity: DeclarationIdentity::sdk("dart:collection", "Unindexed"),
                    arguments: Vec::new().into(),
                    occurrence: Default::default(),
                    nullable: false,
                    extension_type: false,
                }],
            ),
        );
        index.infer_override_types(&program, &model);
        assert_eq!(
            index.member(&sdk_child, "helper").unwrap().positional[0],
            ResolvedType::Unknown
        );
    }

    #[test]
    fn unknown_inherited_signature_survives_combination_in_either_order() {
        for interfaces in ["Cmp, Unavailable", "Unavailable, Cmp"] {
            let path = PathBuf::from("/project/lib/main.dart");
            let source = format!(
                "abstract class Cmp {{ int compareTo(int other); }}
                 abstract class Q {{ int compareTo(String other); }}
                 abstract class Getter {{ int get compareTo; }}
                 abstract class Unavailable {{ int compareTo(Object? other); }}
                 abstract class P implements {interfaces} {{}}
                 abstract class C extends P implements Q {{}}
                 abstract class D extends Q implements P {{}}
                 abstract class KnownConflict implements Cmp, Q {{}}
                 abstract class KindConflict implements P, Getter {{}}"
            );
            let (program, errors) = parse(&source);
            assert!(errors.is_empty(), "{errors:?}");
            let identities = IdentityIndex::from_project_files(
                &[IdentitySource {
                    path: &path,
                    program: &program,
                    has_parse_errors: false,
                }],
                &[],
            );
            let types = TypeIndex::from_program(&program);
            let mut index = SignatureIndex::from_project_files(
                &[(path.clone(), &program)],
                &identities,
                &types,
            );
            let model = SemanticModel::new(&path, &identities, Some(&types));
            // Model a declaration whose member exists but whose complete
            // signature metadata is unavailable, without relying on SDK facts.
            index
                .member_entries
                .get_mut(&model.resolve_name("Unavailable").unwrap())
                .unwrap()
                .get_mut("compareTo")
                .unwrap()
                .override_shape = None;
            let p = ResolvedType::Interface {
                identity: model.resolve_name("P").unwrap(),
                arguments: Vec::new().into(),
                occurrence: Default::default(),
                nullable: false,
                extension_type: false,
            };
            let mut visiting = FxHashSet::default();
            let mut memo = FxHashMap::default();
            let members = index.interface_candidates(
                &p,
                "compareTo",
                false,
                &model,
                &mut visiting,
                &mut memo,
            );
            assert_eq!(members.len(), 1, "{interfaces}");
            assert!(!members[0].known, "{interfaces}");
            for (name, expected) in [
                ("C", true),
                ("D", true),
                ("KnownConflict", false),
                ("KindConflict", false),
            ] {
                assert_eq!(
                    index.has_combined_inherited_member(
                        &model.resolve_name(name).unwrap(),
                        "compareTo",
                        MemberKind::Method,
                        &model,
                    ),
                    expected,
                    "{name}: {interfaces}",
                );
            }
        }
    }

    #[test]
    fn repeated_diamonds_cache_one_unsubstituted_interface_signature() {
        let path = PathBuf::from("/project/lib/main.dart");
        let (program, errors) = parse(
            "abstract class Base<T> { T make(); }
             abstract class L1<T> implements Base<T> {}
             abstract class R1<T> implements Base<T> {}
             abstract class L2<T> implements L1<T>, R1<T> {}
             abstract class R2<T> implements L1<T>, R1<T> {}
             abstract class L3<T> implements L2<T>, R2<T> {}
             abstract class R3<T> implements L2<T>, R2<T> {}
             abstract class Leaf<T> implements L3<T>, R3<T> {}",
        );
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let index =
            SignatureIndex::from_project_files(&[(path.clone(), &program)], &identities, &types);
        let model = SemanticModel::new(&path, &identities, Some(&types));
        let leaf = ResolvedType::Interface {
            identity: model.resolve_name("Leaf").unwrap(),
            arguments: vec![model.core_type("int", false)].into(),
            occurrence: Default::default(),
            nullable: false,
            extension_type: false,
        };
        let mut visiting = FxHashSet::default();
        let mut memo = FxHashMap::default();
        let members =
            index.interface_candidates(&leaf, "make", false, &model, &mut visiting, &mut memo);
        assert_eq!(members.len(), 1);
        assert!(members[0].return_type.interface("dart:core", "int"));
        assert!(memo.values().all(|members| members.len() <= 1));
        assert!(memo.contains_key(&model.resolve_name("Base").unwrap()));
        let cached = memo.len();

        // Block every graph node: a memo hit must not re-enter the diamond,
        // and its owner substitution must still use this edge's arguments.
        visiting.extend(memo.keys().cloned());
        let second = ResolvedType::Interface {
            identity: model.resolve_name("Leaf").unwrap(),
            arguments: vec![model.core_type("String", false)].into(),
            occurrence: Default::default(),
            nullable: false,
            extension_type: false,
        };
        let members =
            index.interface_candidates(&second, "make", false, &model, &mut visiting, &mut memo);
        assert_eq!(members.len(), 1);
        assert!(members[0].return_type.interface("dart:core", "String"));
        assert_eq!(memo.len(), cached);

        visiting.clear();
        memo.clear();
        assert!(
            index
                .interface_candidates(&leaf, "absent", false, &model, &mut visiting, &mut memo)
                .is_empty()
        );
        assert!(memo.values().all(Vec::is_empty));
        assert!(memo.contains_key(&model.resolve_name("Base").unwrap()));
    }
}
