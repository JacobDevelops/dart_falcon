//! Cross-file *type* index — the shape of the project's named types.
//!
//! [`TypeIndex`] records named declarations, their supertypes, members,
//! immutability, and constructor constness. Member and subtype queries share a
//! borrowed ancestor walk; constructor names use `"new"` for unnamed constructors.
//!
//! Three sources feed it:
//!
//! 1. **Library-scoped declarations** — project and external class shapes are
//!    keyed by [`DeclarationIdentity`]. Ancestors resolve in the declaring
//!    library, so unrelated declarations with identical names remain independent.
//!    Name-keyed queries are retained only for degraded syntactic single-file
//!    analysis, where imports cannot be resolved.
//! 2. **A curated CORE table** — `dart:core` / `dart:collection` types falcon
//!    cannot see (`Object`, `Iterable`, `List`, `Map`, `String`, `num`, …) with
//!    their common members and the subtype edges between them.
//! 3. **Library part-completeness** — a type whose declaring library has an
//!    unresolved `part` is flagged `has_unresolved_parts`, which poisons any
//!    *proof of absence* through it (a member might live in the part we can't see).
//!
//! ## Soundness discipline (three-valued, poisoning)
//!
//! Both lookups are three-valued and **Unknown poisons**: a definite answer
//! ([`MemberResult::Absent`] / [`Truth::No`]) is returned
//! *only* when the entire supertype chain is resolved — every ancestor known, no
//! ambiguity, no unresolved external edge, no unresolved part. The instant
//! certainty is lost the answer degrades to `Unknown`. A consumer may thus miss a
//! fact, but must never be told a wrong one.

use std::collections::{HashMap, HashSet};
use std::ops::ControlFlow;
use std::sync::{Arc, OnceLock};

use falcon_syntax::ast::*;

use super::Truth;
use super::{DeclarationIdentity, IdentityIndex, QualifiedName};
use crate::environment::ExternalLibraries;
use std::path::Path;

/// The declaration kind of a named type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeKind {
    Class,
    Mixin,
    MixinClass,
    Enum,
    ExtensionType,
}

/// The kind of a type member (what a name resolves to on a type).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberKind {
    Method,
    Getter,
    Setter,
    Field { writable: bool, covariant: bool },
    Operator,
}

impl MemberKind {
    pub fn field(field: &FieldDecl) -> Self {
        Self::Field {
            writable: field.keyword == VarKeyword::Var,
            covariant: field.is_covariant,
        }
    }

    pub fn has_getter(self) -> bool {
        matches!(self, Self::Getter | Self::Field { .. })
    }

    pub fn has_setter(self) -> bool {
        matches!(self, Self::Setter | Self::Field { writable: true, .. })
    }
}

/// Three-valued result of a member lookup on a type.
///
/// [`MemberResult::Absent`] is only ever returned when the type's full
/// supertype chain is resolved and no ancestor declares the member; any
/// uncertainty (unknown ancestor, ambiguous name, unresolved part) degrades the
/// answer to [`MemberResult::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberResult {
    Found(MemberKind),
    Absent,
    Unknown,
}

/// A single user-declared type's recorded shape.
#[derive(Debug, Clone)]
struct TypeEntry {
    kind: TypeKind,
    /// Last-segment names from `extends` / `with` / `implements` / `on`.
    supertypes: Vec<String>,
    /// Own declared member names → their kind.
    members: HashMap<String, MemberKind>,
    /// Whether this declaration is annotated with the resolved package:meta
    /// `immutable` marker.
    is_immutable: bool,
    /// Direct superclass, excluding mixins and implemented interfaces.
    superclass: Option<String>,
    /// Declared constructors keyed by `"new"` for unnamed or their source name.
    constructors: HashMap<String, bool>,
    /// Whether every instance field can participate in a const constructor.
    all_instance_fields_final: bool,
    /// The declaring library has an unresolved `part`, so this type's member set
    /// may be incomplete — poisons any proof of absence through it.
    has_unresolved_parts: bool,
}

#[derive(Debug, Clone)]
struct IdentityEntry {
    kind: TypeKind,
    members: HashMap<String, MemberKind>,
    is_immutable: bool,
    has_unresolved_parts: bool,
    /// An unresolved marker may be an immutable annotation, so absence is not proven.
    annotations_unknown: bool,
    ancestors: Vec<Option<DeclarationIdentity>>,
    superclass: Option<Option<DeclarationIdentity>>,
}

/// One name's slot in the user-declaration table.
#[derive(Debug, Clone)]
enum Slot {
    Unique(Box<TypeEntry>),
    /// Two or more declarations share this name — every lookup poisons.
    Ambiguous,
}

/// A resolved view of a type name: a user entry, a core entry, or ambiguous.
#[derive(Clone, Copy)]
enum Resolved<'a> {
    User(&'a TypeEntry),
    Core(&'a CoreType),
    Ambiguous,
}

/// A file's parsed program plus the flags [`TypeIndex`] needs to record its
/// types soundly: whether the parse failed (declarations are then untrustworthy
/// and skipped) and whether its declaring library has an unresolved part.
pub struct LibrarySource<'a> {
    pub program: &'a Program,
    pub has_parse_errors: bool,
    pub has_unresolved_parts: bool,
}

/// Library-scoped type shapes with a degraded syntactic single-file mode.
#[derive(Debug, Clone, Default)]
pub struct TypeIndex {
    types: HashMap<String, Slot>,
    /// Project and loaded external shapes, keyed by their declaring library.
    declarations: HashMap<DeclarationIdentity, IdentityEntry>,
    has_sdk_sources: bool,
    /// Immutable external shapes shared by successive project snapshots.
    external: Option<Arc<TypeIndex>>,
}

impl TypeIndex {
    /// Build library-scoped shapes after import and declaration identities exist.
    /// Both project and external ingestion must use the same identity index.
    pub fn from_resolved_files<'a>(
        files: impl IntoIterator<Item = (&'a Path, LibrarySource<'a>)>,
        identities: &IdentityIndex,
    ) -> Self {
        let mut index = Self {
            has_sdk_sources: identities.has_sdk_sources(),
            ..Self::default()
        };
        for (path, file) in files {
            index.add_resolved_program(
                path,
                file.program,
                file.has_parse_errors,
                file.has_unresolved_parts,
                identities,
            );
        }
        index
    }

    /// Rebuild only project shapes, retaining the external declaration table.
    pub fn with_external<'a>(
        files: impl IntoIterator<Item = (&'a Path, LibrarySource<'a>)>,
        identities: &IdentityIndex,
        external: Arc<TypeIndex>,
    ) -> Self {
        let mut index = Self::from_resolved_files(files, identities);
        index.external = Some(external);
        index
    }

    fn declaration(&self, identity: &DeclarationIdentity) -> Option<&IdentityEntry> {
        self.declarations
            .get(identity)
            .or_else(|| self.external.as_ref()?.declaration(identity))
    }

    fn add_resolved_program(
        &mut self,
        path: &Path,
        program: &Program,
        has_parse_errors: bool,
        has_unresolved_parts: bool,
        identities: &IdentityIndex,
    ) {
        if has_parse_errors {
            return;
        }
        let Some(file) = identities.file(path) else {
            return;
        };
        let library = identities.library(file);
        let mut local = Self::default();
        local.add_program(program, has_unresolved_parts, false);
        self.declarations.reserve(local.types.len());
        let resolve = |ty: &DartType| match ty {
            DartType::Named(named) => QualifiedName::from_identifiers(&named.segments)
                .and_then(|name| identities.resolve_declaration(library, name)),
            _ => None,
        };
        for declaration in &program.declarations {
            let (name, superclass, first, second): (
                &str,
                Option<&DartType>,
                &[DartType],
                &[DartType],
            ) = match declaration {
                TopLevelDecl::Class(c) => (
                    &c.name.name,
                    c.extends.as_ref(),
                    &c.with_clause,
                    &c.implements,
                ),
                TopLevelDecl::Mixin(c) => (&c.name.name, None, &c.on_clause, &c.implements),
                TopLevelDecl::MixinClass(c) => (
                    &c.name.name,
                    c.extends.as_ref(),
                    &c.with_clause,
                    &c.implements,
                ),
                TopLevelDecl::Enum(c) => (&c.name.name, None, &c.with_clause, &c.implements),
                TopLevelDecl::ExtensionType(c) => (&c.name.name, None, &[], &c.implements),
                _ => continue,
            };
            if !identities.is_authoritative_declaration(library, name) {
                continue;
            }
            let Some(identity) = identities.resolve_declaration(library, name.into()) else {
                continue;
            };
            let Some(Slot::Unique(mut shape)) = local.types.remove(name) else {
                continue;
            };
            let annotations: &[Annotation] = match declaration {
                TopLevelDecl::Class(class) => &class.annotations,
                TopLevelDecl::MixinClass(class) => &class.annotations,
                _ => &[],
            };
            let mut annotations_unknown = false;
            shape.is_immutable = false;
            for annotation in annotations {
                match QualifiedName::from_identifiers(&annotation.name).and_then(|name| {
                    identities
                        .resolve_imported_member(library, name)
                        .or_else(|| identities.resolve_declaration(library, name))
                        .or_else(|| identities.resolve_value_declaration(library, name))
                }) {
                    Some(identity) => {
                        shape.is_immutable |=
                            identities.is_package_declaration(&identity, "meta", "immutable")
                                || identities.is_package_declaration(
                                    &identity,
                                    "meta",
                                    "Immutable",
                                );
                    }
                    None => {
                        annotations_unknown |= annotation
                            .name
                            .last()
                            .is_some_and(|name| matches!(&*name.name, "immutable" | "Immutable"))
                    }
                }
            }
            let mut resolved = Vec::with_capacity(
                first.len()
                    + second.len()
                    + usize::from(superclass.is_some())
                    + usize::from(shape.kind == TypeKind::Enum),
            );
            resolved.extend(
                superclass
                    .into_iter()
                    .chain(first)
                    .chain(second)
                    .map(resolve),
            );
            if shape.kind == TypeKind::Enum {
                resolved.push(Some(DeclarationIdentity::sdk("dart:core", "Enum")));
            }
            self.declarations.insert(
                identity,
                IdentityEntry {
                    kind: shape.kind,
                    members: shape.members,
                    is_immutable: shape.is_immutable,
                    has_unresolved_parts: shape.has_unresolved_parts,
                    annotations_unknown,
                    ancestors: resolved,
                    superclass: superclass.map(resolve),
                },
            );
        }
    }

    pub fn type_kind_identity(&self, identity: &DeclarationIdentity) -> Option<TypeKind> {
        if self.external.is_none()
            && self.declarations.is_empty()
            && matches!(identity, DeclarationIdentity::Project { .. })
        {
            return self.type_kind(identity.name());
        }
        self.declaration(identity).map(|entry| entry.kind)
    }

    pub fn member_lookup_identity(
        &self,
        identity: &DeclarationIdentity,
        member: &str,
    ) -> MemberResult {
        if self.declarations.is_empty() && !self.types.is_empty() {
            return self.member_lookup(identity.name(), member);
        }
        match self.walk_identity(identity, |_, entry, core| {
            entry
                .and_then(|entry| entry.members.get(member).copied())
                .or_else(|| core.and_then(|core| core_member(core, member)))
        }) {
            ControlFlow::Break(kind) => MemberResult::Found(kind),
            ControlFlow::Continue(true) => MemberResult::Absent,
            ControlFlow::Continue(false) => MemberResult::Unknown,
        }
    }

    pub fn is_subtype_identity(
        &self,
        sub: &DeclarationIdentity,
        super_: &DeclarationIdentity,
    ) -> Truth {
        if self.declarations.is_empty() && !self.types.is_empty() {
            return self.is_subtype(sub.name(), super_.name());
        }
        self.supertype_matches_identity(sub, |identity| identity == super_)
    }

    /// Whether `sub` or any of its supertypes satisfies `matches`; `No` only
    /// when the whole chain is resolved. In single-file mode, project names
    /// are matched as declarations of `sub`'s library.
    pub fn supertype_matches_identity(
        &self,
        sub: &DeclarationIdentity,
        mut matches: impl FnMut(&DeclarationIdentity) -> bool,
    ) -> Truth {
        let found = if self.declarations.is_empty() && !self.types.is_empty() {
            let DeclarationIdentity::Project { library, .. } = sub else {
                return Truth::Unknown;
            };
            self.walk_ancestors(sub.name(), |name, resolved| {
                let identity = match core_table().get_key_value(name) {
                    Some((name, _)) if matches!(resolved, Some(Resolved::Core(_))) => {
                        DeclarationIdentity::sdk("dart:core", name)
                    }
                    _ => DeclarationIdentity::Project {
                        library: *library,
                        name: name.to_string(),
                    },
                };
                matches(&identity).then_some(())
            })
        } else {
            self.walk_identity(sub, |identity, _, _| matches(identity).then_some(()))
        };
        match found {
            ControlFlow::Break(()) => Truth::Yes,
            ControlFlow::Continue(true) => Truth::No,
            ControlFlow::Continue(false) => Truth::Unknown,
        }
    }

    pub fn is_immutable_type_identity(&self, identity: &DeclarationIdentity) -> Truth {
        if self.declarations.is_empty() && !self.types.is_empty() {
            return self.is_immutable_type(identity.name());
        }
        let mut annotations_unknown = false;
        match self.walk_identity(identity, |_, entry, _| {
            if let Some(entry) = entry {
                annotations_unknown |= entry.annotations_unknown;
                return entry.is_immutable.then_some(());
            }
            None
        }) {
            ControlFlow::Break(()) => Truth::Yes,
            ControlFlow::Continue(true) if !annotations_unknown => Truth::No,
            ControlFlow::Continue(_) => Truth::Unknown,
        }
    }

    fn identity_core(&self, identity: &DeclarationIdentity) -> Option<&CoreType> {
        if self.has_sdk_sources {
            return None;
        }
        match identity {
            DeclarationIdentity::Sdk { library, name }
                if library.as_str() == "dart:core" || library.as_str() == "dart:collection" =>
            {
                core_table().get(name.as_str())
            }
            _ => None,
        }
    }

    fn walk_identity<'a, T>(
        &'a self,
        root: &'a DeclarationIdentity,
        mut find: impl FnMut(
            &DeclarationIdentity,
            Option<&IdentityEntry>,
            Option<&CoreType>,
        ) -> Option<T>,
    ) -> ControlFlow<T, bool> {
        let object = DeclarationIdentity::OBJECT;
        let mut visited = HashSet::new();
        let mut stack = vec![root];
        let mut complete = true;
        while let Some(identity) = stack.pop() {
            if !visited.insert(identity) {
                continue;
            }
            let entry = self.declaration(identity);
            let core = entry
                .is_none()
                .then(|| self.identity_core(identity))
                .flatten();
            if let Some(value) = find(identity, entry, core) {
                return ControlFlow::Break(value);
            }
            if let Some(entry) = entry {
                complete &= !entry.has_unresolved_parts;
                for ancestor in &entry.ancestors {
                    match ancestor {
                        Some(identity) => stack.push(identity),
                        None => complete = false,
                    }
                }
            } else if let Some(core) = core {
                stack.extend(&core.ancestor_identities);
            } else {
                // No resolved shape: not even known to be a class type, so
                // nothing (including `Object`) may be proven through it.
                complete = false;
                continue;
            }
            if identity != &DeclarationIdentity::OBJECT {
                stack.push(&object);
            }
        }
        ControlFlow::Continue(complete)
    }

    pub fn superclass_chain_is_immutable_identity(&self, identity: &DeclarationIdentity) -> Truth {
        if self.declarations.is_empty() && !self.types.is_empty() {
            return self.superclass_chain_is_immutable(identity.name());
        }
        let mut visited = HashSet::new();
        let mut current = identity;
        let mut uncertain = false;
        while visited.insert(current) {
            let Some(entry) = self.declaration(current) else {
                return if !uncertain && self.identity_core(current).is_some() {
                    Truth::No
                } else {
                    Truth::Unknown
                };
            };
            if entry.is_immutable {
                return Truth::Yes;
            }
            uncertain |= entry.annotations_unknown || entry.has_unresolved_parts;
            match &entry.superclass {
                Some(Some(identity)) => current = identity,
                Some(None) => return Truth::Unknown,
                None => return if uncertain { Truth::Unknown } else { Truth::No },
            }
        }
        Truth::Unknown
    }

    /// Record compact shapes from loaded external libraries using the same
    /// identity index as [`Self::from_resolved_files`]. Declarations from
    /// incomplete libraries remain unknown, without poisoning other libraries.
    pub fn add_external(&mut self, external: &ExternalLibraries, identities: &IdentityIndex) {
        self.has_sdk_sources = identities.has_sdk_sources();
        for library in external.libraries() {
            for file in &library.files {
                self.add_resolved_program(
                    &file.path,
                    &file.facts,
                    file.has_parse_errors,
                    library.has_unresolved_parts,
                    identities,
                );
            }
        }
    }

    /// Build a degraded, single-file index. A file that declares (or is) a `part`
    /// is treated as having unresolved parts, since the sibling files that could
    /// complete its types are not available in single-file mode.
    pub fn from_program(program: &Program) -> Self {
        let unresolved = !program.part_directives.is_empty() || program.part_of_directive.is_some();
        let mut index = Self::default();
        index.add_program(program, unresolved, true);
        index
    }

    /// Whether `name` resolves to exactly one known type (user or core). An
    /// ambiguous or unknown name is not a "known type".
    pub fn is_known_type(&self, name: &str) -> bool {
        matches!(
            self.resolve(name),
            Some(Resolved::User(_) | Resolved::Core(_))
        )
    }

    /// The declaration kind of a *user-declared* `name`, or `None` when no unique
    /// user declaration exists (unknown, or two user decls collide on the name).
    ///
    /// Unlike the three-valued lookups, this reports the presence of a user
    /// declaration even when its name shadows a core type: the declaration is a
    /// real syntactic fact, independent of the import-ambiguity that poisons
    /// proofs *through* the type.
    pub fn type_kind(&self, name: &str) -> Option<TypeKind> {
        match self.types.get(name) {
            Some(Slot::Unique(e)) => Some(e.kind),
            _ => None,
        }
    }

    /// Look up `member` on `type_name`, walking the supertype chain.
    ///
    /// Returns [`MemberResult::Found`] on the first ancestor declaring it,
    /// [`MemberResult::Absent`] only when the *entire* chain is resolved and
    /// none declares it, and [`MemberResult::Unknown`] whenever the chain has any
    /// unresolved edge (unknown ancestor, ambiguity, or unresolved part).
    pub fn member_lookup(&self, type_name: &str, member: &str) -> MemberResult {
        match self.walk_ancestors(type_name, |_, resolved| match resolved {
            Some(Resolved::User(entry)) => entry.members.get(member).copied(),
            Some(Resolved::Core(core)) => core_member(core, member),
            _ => None,
        }) {
            ControlFlow::Break(kind) => MemberResult::Found(kind),
            ControlFlow::Continue(true) => MemberResult::Absent,
            ControlFlow::Continue(false) => MemberResult::Unknown,
        }
    }

    /// Whether `sub` is a subtype of `super_`, walking `sub`'s ancestor chain.
    ///
    /// Returns [`Truth::Yes`] on a name match, [`Truth::No`]
    /// only when `sub`'s entire chain is resolved and never names `super_`, and
    /// [`Truth::Unknown`] whenever the chain has an unresolved edge.
    pub fn is_subtype(&self, sub: &str, super_: &str) -> Truth {
        if sub == super_ {
            return Truth::Yes;
        }
        match self.walk_ancestors(sub, |name, _| (name == super_).then_some(())) {
            ControlFlow::Break(()) => Truth::Yes,
            ControlFlow::Continue(true) => Truth::No,
            ControlFlow::Continue(false) => Truth::Unknown,
        }
    }

    /// Whether a type is directly or transitively immutable.
    pub fn is_immutable_type(&self, type_name: &str) -> Truth {
        match self.walk_ancestors(type_name, |_, resolved| {
            matches!(resolved, Some(Resolved::User(entry)) if entry.is_immutable).then_some(())
        }) {
            ControlFlow::Break(()) => Truth::Yes,
            ControlFlow::Continue(true) => Truth::No,
            ControlFlow::Continue(false) => Truth::Unknown,
        }
    }

    fn walk_ancestors<'a, T>(
        &'a self,
        root: &'a str,
        mut find: impl FnMut(&str, Option<Resolved<'a>>) -> Option<T>,
    ) -> ControlFlow<T, bool> {
        let mut visited = HashSet::new();
        let mut stack = vec![root];
        let mut complete = true;
        while let Some(name) = stack.pop() {
            if !visited.insert(name) {
                continue;
            }
            let resolved = self.resolve(name);
            if let Some(found) = find(name, resolved) {
                return ControlFlow::Break(found);
            }
            match resolved {
                None | Some(Resolved::Ambiguous) => complete = false,
                Some(Resolved::User(entry)) => {
                    complete &= !entry.has_unresolved_parts;
                    push_ancestors(
                        name,
                        entry.supertypes.iter().map(String::as_str),
                        &mut stack,
                    );
                }
                Some(Resolved::Core(core)) => {
                    push_ancestors(name, core.supertypes.iter().copied(), &mut stack);
                }
            }
        }
        ControlFlow::Continue(complete)
    }

    /// Whether a named constructor is declared or implicitly available as const.
    pub fn constructor_is_const(&self, type_name: &str, name: Option<&str>) -> Truth {
        let name = name.filter(|name| !name.is_empty()).unwrap_or("new");
        self.constructor_is_const_inner(type_name, name, &mut HashSet::new())
    }

    fn constructor_is_const_inner<'a>(
        &'a self,
        type_name: &'a str,
        name: &'a str,
        visited: &mut HashSet<(&'a str, &'a str)>,
    ) -> Truth {
        if !visited.insert((type_name, name)) {
            return Truth::Unknown;
        }
        match self.resolve(type_name) {
            None | Some(Resolved::Ambiguous) => Truth::Unknown,
            Some(Resolved::Core(_)) => (type_name == "Object" && name == "new").into(),
            Some(Resolved::User(entry)) => {
                if entry.has_unresolved_parts {
                    return Truth::Unknown;
                }
                if let Some(&is_const) = entry.constructors.get(name) {
                    return is_const.into();
                }
                if !entry.constructors.is_empty()
                    || name != "new"
                    || !entry.all_instance_fields_final
                {
                    return Truth::No;
                }
                let superclass = entry.superclass.as_deref().unwrap_or("Object");
                self.constructor_is_const_inner(superclass, "new", visited)
            }
        }
    }

    /// Whether the class or a class on its superclass chain (`extends` only,
    /// not mixins or interfaces) is annotated `@immutable`. `No` only when the
    /// whole chain is resolved project code without the annotation.
    pub fn superclass_chain_is_immutable(&self, type_name: &str) -> Truth {
        let mut visited = HashSet::new();
        let mut current = type_name;
        while visited.insert(current) {
            match self.resolve(current) {
                Some(Resolved::User(entry)) => {
                    if entry.is_immutable {
                        return Truth::Yes;
                    }
                    if entry.has_unresolved_parts {
                        return Truth::Unknown;
                    }
                    match entry.superclass.as_deref() {
                        Some(superclass) => current = superclass,
                        None => return Truth::No,
                    }
                }
                Some(Resolved::Core(_)) => return Truth::No,
                None | Some(Resolved::Ambiguous) => return Truth::Unknown,
            }
        }
        Truth::Unknown
    }

    // ── Internals ───────────────────────────────────────────────────────────────

    /// Resolve a syntactic single-file name against user declarations first,
    /// then the curated core table. Loaded external shapes are identity-only.
    ///
    /// A user declaration whose name also exists in the core table cannot be
    /// disambiguated from its core namesake (the index is name-keyed), so it
    /// resolves as [`Resolved::Ambiguous`] — every lookup through it poisons,
    /// exactly as two colliding user declarations would.
    fn resolve(&self, name: &str) -> Option<Resolved<'_>> {
        match self.types.get(name) {
            Some(Slot::Unique(e)) => {
                if core_table().contains_key(name) {
                    Some(Resolved::Ambiguous)
                } else {
                    Some(Resolved::User(e))
                }
            }
            Some(Slot::Ambiguous) => Some(Resolved::Ambiguous),
            None => core_table().get(name).map(Resolved::Core),
        }
    }

    /// Harvest every named-type declaration in a program.
    fn add_program(&mut self, program: &Program, has_unresolved_parts: bool, syntactic: bool) {
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) => {
                    let element_members = c.element_members();
                    let supertypes = if syntactic {
                        supertype_names(
                            c.extends
                                .as_ref()
                                .into_iter()
                                .chain(&c.with_clause)
                                .chain(&c.implements),
                        )
                    } else {
                        Vec::new()
                    };
                    self.add_type(
                        &c.name.name,
                        TypeEntry {
                            kind: TypeKind::Class,
                            supertypes,
                            members: collect_members(element_members),
                            is_immutable: syntactic
                                && has_immutable_annotation(&c.annotations, program),
                            superclass: if syntactic {
                                c.extends.as_ref().and_then(last_segment)
                            } else {
                                None
                            },
                            constructors: if syntactic {
                                collect_constructors(element_members)
                            } else {
                                HashMap::new()
                            },
                            all_instance_fields_final: syntactic
                                && all_instance_fields_final(element_members),
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::Mixin(m) => {
                    let supertypes = if syntactic {
                        supertype_names(m.on_clause.iter().chain(&m.implements))
                    } else {
                        Vec::new()
                    };
                    self.add_type(
                        &m.name.name,
                        TypeEntry {
                            kind: TypeKind::Mixin,
                            supertypes,
                            members: collect_members(EffectiveMembers::declared(&m.members)),
                            is_immutable: false,
                            superclass: None,
                            constructors: HashMap::new(),
                            all_instance_fields_final: syntactic
                                && all_instance_fields_final(EffectiveMembers::declared(
                                    &m.members,
                                )),
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::MixinClass(m) => {
                    let element_members = m.element_members();
                    let supertypes = if syntactic {
                        supertype_names(
                            m.extends
                                .as_ref()
                                .into_iter()
                                .chain(&m.with_clause)
                                .chain(&m.implements),
                        )
                    } else {
                        Vec::new()
                    };
                    self.add_type(
                        &m.name.name,
                        TypeEntry {
                            kind: TypeKind::MixinClass,
                            supertypes,
                            members: collect_members(element_members),
                            is_immutable: syntactic
                                && has_immutable_annotation(&m.annotations, program),
                            superclass: if syntactic {
                                m.extends.as_ref().and_then(last_segment)
                            } else {
                                None
                            },
                            constructors: if syntactic {
                                collect_constructors(element_members)
                            } else {
                                HashMap::new()
                            },
                            all_instance_fields_final: syntactic
                                && all_instance_fields_final(element_members),
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::Enum(e) => {
                    let element_members = e.element_members();
                    let supertypes = if syntactic {
                        let mut names = supertype_names(e.with_clause.iter().chain(&e.implements));
                        // Every enum is a subtype of `Enum` (source of `index` / `name`).
                        names.push("Enum".to_string());
                        names
                    } else {
                        Vec::new()
                    };
                    let mut members = collect_members(element_members);
                    // Enum constants are accessible names on the type.
                    for v in &e.variants {
                        members
                            .entry(v.name.name.clone())
                            .or_insert(MemberKind::Field {
                                writable: false,
                                covariant: false,
                            });
                    }
                    self.add_type(
                        &e.name.name,
                        TypeEntry {
                            kind: TypeKind::Enum,
                            supertypes,
                            members,
                            is_immutable: false,
                            superclass: None,
                            constructors: if syntactic {
                                collect_constructors(element_members)
                            } else {
                                HashMap::new()
                            },
                            all_instance_fields_final: syntactic
                                && all_instance_fields_final(element_members),
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::ExtensionType(e) => {
                    let element_members = e.element_members();
                    let supertypes = if syntactic {
                        supertype_names(e.implements.iter())
                    } else {
                        Vec::new()
                    };
                    self.add_type(
                        &e.name.name,
                        TypeEntry {
                            kind: TypeKind::ExtensionType,
                            supertypes,
                            members: collect_members(element_members),
                            is_immutable: false,
                            superclass: None,
                            constructors: if syntactic {
                                collect_constructors(element_members)
                            } else {
                                HashMap::new()
                            },
                            all_instance_fields_final: syntactic
                                && all_instance_fields_final(element_members),
                            has_unresolved_parts,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    /// Insert a user type, degrading to [`Slot::Ambiguous`] on any name clash.
    fn add_type(&mut self, name: &str, entry: TypeEntry) {
        match self.types.get(name) {
            None => {
                self.types
                    .insert(name.to_string(), Slot::Unique(Box::new(entry)));
            }
            Some(_) => {
                self.types.insert(name.to_string(), Slot::Ambiguous);
            }
        }
    }
}

/// Push a type's declared ancestors plus the implicit universal root `Object`
/// (unless the type *is* `Object`). The visited-set in the walk dedups, so the
/// unconditional `Object` push is safe and terminates.
fn push_ancestors<'a>(
    name: &str,
    supertypes: impl Iterator<Item = &'a str>,
    stack: &mut Vec<&'a str>,
) {
    stack.extend(supertypes);
    if name != "Object" {
        stack.push("Object");
    }
}

/// The last segment (`prefix.Type` → `Type`) of every *named* supertype; record,
/// function, void/dynamic/never supertypes contribute no name.
fn supertype_names<'a>(tys: impl Iterator<Item = &'a DartType>) -> Vec<String> {
    tys.filter_map(last_segment).collect()
}

/// The last name segment of a named type, or `None` for non-named types.
fn last_segment(ty: &DartType) -> Option<String> {
    match ty {
        DartType::Named(n) => n.segments.last().map(|id| id.name.clone()),
        _ => None,
    }
}

/// Whether `annotations` include package:meta's `@immutable` (imported, possibly
/// with a prefix, from meta or a Flutter library re-exporting it) or
/// `@Immutable(...)` (imported from meta).
pub fn has_immutable_annotation(annotations: &[Annotation], program: &Program) -> bool {
    annotations.iter().any(|annotation| {
        let Some(last) = annotation.name.last() else {
            return false;
        };
        let flutter_reexports = match &*last.name {
            "immutable" => true,
            "Immutable" => false,
            _ => return false,
        };
        let prefix = (annotation.name.len() > 1).then(|| annotation.name[0].name.as_str());
        program.imports.iter().any(|import| {
            (import.uri.value == "package:meta/meta.dart"
                || flutter_reexports
                    && matches!(
                        import.uri.value.as_str(),
                        "package:flutter/foundation.dart"
                            | "package:flutter/widgets.dart"
                            | "package:flutter/material.dart"
                    ))
                && import.as_name.as_ref().map(|name| name.name.as_str()) == prefix
        })
    })
}

fn collect_constructors(members: EffectiveMembers<'_>) -> HashMap<String, bool> {
    members
        .constructors()
        .map(|constructor| {
            (
                constructor
                    .constructor_name
                    .map(|name| name.name.clone())
                    .unwrap_or_else(|| "new".to_string()),
                constructor.is_const,
            )
        })
        .collect()
}

fn all_instance_fields_final(members: EffectiveMembers<'_>) -> bool {
    members.declared.iter().all(|member| {
        !matches!(member, ClassMember::Field(field) if !field.is_static && field.keyword == VarKeyword::Var)
    }) && members.declaring_params().all(|param| members.representation || param.is_final)
}

fn collect_members(members: EffectiveMembers<'_>) -> HashMap<String, MemberKind> {
    let mut map = HashMap::new();
    for member in members.declared {
        match member {
            ClassMember::Field(f) => {
                for d in &f.declarators {
                    map.insert(d.name.name.clone(), MemberKind::field(f));
                }
            }
            ClassMember::Method(m) => {
                map.insert(m.name.name.clone(), MemberKind::Method);
            }
            ClassMember::Getter(g) => {
                map.insert(g.name.name.clone(), MemberKind::Getter);
            }
            ClassMember::Setter(s) => {
                map.entry(s.name.name.clone()).or_insert(MemberKind::Setter);
            }
            ClassMember::Operator(o) => {
                map.insert(o.op.clone(), MemberKind::Operator);
            }
            ClassMember::Constructor(_)
            | ClassMember::PrimaryConstructorBody(_)
            | ClassMember::Error(_) => {}
        }
    }
    for param in members.declaring_params() {
        map.insert(
            param.name.name.clone(),
            MemberKind::Field {
                writable: !members.representation && !param.is_final,
                covariant: param.is_covariant,
            },
        );
    }
    map
}

// ── Curated core-library table ──────────────────────────────────────────────────

/// A curated core-library type: its subtype edges and its common members.
struct CoreType {
    supertypes: Vec<&'static str>,
    ancestor_identities: Vec<DeclarationIdentity>,
    members: Vec<(&'static str, MemberKind)>,
}

fn core_member(c: &CoreType, member: &str) -> Option<MemberKind> {
    c.members
        .iter()
        .find(|(n, _)| *n == member)
        .map(|(_, k)| *k)
}

/// The curated `dart:core` / `dart:collection` table falcon cannot otherwise see.
fn core_table() -> &'static HashMap<&'static str, CoreType> {
    static TABLE: OnceLock<HashMap<&'static str, CoreType>> = OnceLock::new();
    TABLE.get_or_init(build_core_table)
}

fn build_core_table() -> HashMap<&'static str, CoreType> {
    use MemberKind::{Getter, Method, Operator};
    let mut t: HashMap<&'static str, CoreType> = HashMap::new();

    let ty = |supertypes: &[&'static str], members: Vec<(&'static str, MemberKind)>| CoreType {
        supertypes: supertypes.to_vec(),
        ancestor_identities: supertypes
            .iter()
            .map(|name| DeclarationIdentity::sdk("dart:core", name))
            .collect(),
        members,
    };

    let object = ty(
        &[],
        vec![
            ("==", Operator),
            ("toString", Method),
            ("hashCode", Getter),
            ("runtimeType", Getter),
            ("noSuchMethod", Method),
        ],
    );
    t.insert("Object", object);

    t.insert(
        "Iterable",
        ty(
            &["Object"],
            vec![
                ("length", Getter),
                ("isEmpty", Getter),
                ("isNotEmpty", Getter),
                ("first", Getter),
                ("last", Getter),
                ("single", Getter),
                ("iterator", Getter),
                ("map", Method),
                ("where", Method),
                ("whereType", Method),
                ("contains", Method),
                ("forEach", Method),
                ("any", Method),
                ("every", Method),
                ("toList", Method),
                ("toSet", Method),
                ("expand", Method),
                ("fold", Method),
                ("reduce", Method),
                ("join", Method),
                ("elementAt", Method),
                ("skip", Method),
                ("skipWhile", Method),
                ("take", Method),
                ("takeWhile", Method),
                ("cast", Method),
                ("followedBy", Method),
                ("firstWhere", Method),
                ("lastWhere", Method),
                ("singleWhere", Method),
            ],
        ),
    );

    t.insert(
        "List",
        ty(
            &["Iterable"],
            vec![
                ("[]", Operator),
                ("[]=", Operator),
                ("add", Method),
                ("addAll", Method),
                ("insert", Method),
                ("insertAll", Method),
                ("remove", Method),
                ("removeAt", Method),
                ("removeLast", Method),
                ("removeWhere", Method),
                ("retainWhere", Method),
                ("removeRange", Method),
                ("clear", Method),
                ("sort", Method),
                ("shuffle", Method),
                ("indexOf", Method),
                ("indexWhere", Method),
                ("lastIndexOf", Method),
                ("sublist", Method),
                ("getRange", Method),
                ("setRange", Method),
                ("fillRange", Method),
                ("replaceRange", Method),
                ("setAll", Method),
                ("asMap", Method),
                ("reversed", Getter),
            ],
        ),
    );

    t.insert(
        "Set",
        ty(
            &["Iterable"],
            vec![
                ("add", Method),
                ("addAll", Method),
                ("remove", Method),
                ("removeAll", Method),
                ("retainAll", Method),
                ("contains", Method),
                ("containsAll", Method),
                ("union", Method),
                ("intersection", Method),
                ("difference", Method),
                ("lookup", Method),
                ("clear", Method),
                ("removeWhere", Method),
                ("retainWhere", Method),
            ],
        ),
    );

    t.insert(
        "Map",
        ty(
            &["Object"],
            vec![
                ("[]", Operator),
                ("[]=", Operator),
                ("keys", Getter),
                ("values", Getter),
                ("entries", Getter),
                ("length", Getter),
                ("isEmpty", Getter),
                ("isNotEmpty", Getter),
                ("containsKey", Method),
                ("containsValue", Method),
                ("forEach", Method),
                ("putIfAbsent", Method),
                ("remove", Method),
                ("addAll", Method),
                ("addEntries", Method),
                ("update", Method),
                ("updateAll", Method),
                ("removeWhere", Method),
                ("clear", Method),
                ("map", Method),
                ("cast", Method),
            ],
        ),
    );

    t.insert(
        "String",
        ty(
            &["Object", "Comparable", "Pattern"],
            vec![
                ("[]", Operator),
                ("length", Getter),
                ("isEmpty", Getter),
                ("isNotEmpty", Getter),
                ("codeUnits", Getter),
                ("runes", Getter),
                ("toUpperCase", Method),
                ("toLowerCase", Method),
                ("trim", Method),
                ("trimLeft", Method),
                ("trimRight", Method),
                ("substring", Method),
                ("contains", Method),
                ("startsWith", Method),
                ("endsWith", Method),
                ("indexOf", Method),
                ("lastIndexOf", Method),
                ("replaceAll", Method),
                ("replaceFirst", Method),
                ("replaceRange", Method),
                ("split", Method),
                ("splitMapJoin", Method),
                ("padLeft", Method),
                ("padRight", Method),
                ("codeUnitAt", Method),
                ("compareTo", Method),
                ("allMatches", Method),
                ("matchAsPrefix", Method),
            ],
        ),
    );

    t.insert(
        "num",
        ty(
            &["Object", "Comparable"],
            vec![
                ("isNaN", Getter),
                ("isFinite", Getter),
                ("isInfinite", Getter),
                ("isNegative", Getter),
                ("sign", Getter),
                ("abs", Method),
                ("ceil", Method),
                ("floor", Method),
                ("round", Method),
                ("truncate", Method),
                ("toInt", Method),
                ("toDouble", Method),
                ("clamp", Method),
                ("compareTo", Method),
                ("remainder", Method),
                ("toStringAsFixed", Method),
                ("toStringAsExponential", Method),
                ("toStringAsPrecision", Method),
            ],
        ),
    );

    t.insert(
        "int",
        ty(
            &["num"],
            vec![
                ("isEven", Getter),
                ("isOdd", Getter),
                ("bitLength", Getter),
                ("gcd", Method),
                ("toRadixString", Method),
                ("modPow", Method),
                ("modInverse", Method),
                ("toSigned", Method),
                ("toUnsigned", Method),
            ],
        ),
    );

    t.insert(
        "double",
        ty(
            &["num"],
            vec![
                ("roundToDouble", Method),
                ("floorToDouble", Method),
                ("ceilToDouble", Method),
                ("truncateToDouble", Method),
            ],
        ),
    );

    t.insert("bool", ty(&["Object"], vec![]));

    t.insert(
        "Enum",
        ty(&["Object"], vec![("index", Getter), ("name", Getter)]),
    );

    t.insert("Comparable", ty(&["Object"], vec![("compareTo", Method)]));
    t.insert("Pattern", ty(&["Object"], vec![("allMatches", Method)]));

    t.insert(
        "Queue",
        ty(
            &["Iterable"],
            vec![
                ("add", Method),
                ("addFirst", Method),
                ("addLast", Method),
                ("addAll", Method),
                ("removeFirst", Method),
                ("removeLast", Method),
                ("remove", Method),
                ("clear", Method),
            ],
        ),
    );

    t.insert("LinkedHashMap", ty(&["Map"], vec![]));
    t.insert("HashMap", ty(&["Map"], vec![]));
    t.insert("SplayTreeMap", ty(&["Map"], vec![]));
    t.insert("LinkedHashSet", ty(&["Set"], vec![]));
    t.insert("HashSet", ty(&["Set"], vec![]));
    t.insert("SplayTreeSet", ty(&["Set"], vec![]));

    t
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    fn program(src: &str) -> Program {
        let falcon_dart_parser::ParseResult {
            program, errors, ..
        } = parse(src);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        program
    }

    fn annotation_packages<'a>(
        meta: &'a Program,
        flutter: &'a Program,
    ) -> (
        Vec<super::super::IdentitySource<'a>>,
        [super::super::PackageIdentity; 2],
    ) {
        use super::super::{IdentitySource, PackageIdentity};
        let sources = [
            ("/packages/meta/lib/meta.dart", meta),
            ("/packages/flutter/lib/foundation.dart", flutter),
            ("/packages/flutter/lib/widgets.dart", flutter),
            ("/packages/flutter/lib/material.dart", flutter),
        ]
        .into_iter()
        .map(|(path, program)| IdentitySource {
            path: Path::new(path),
            program,
            has_parse_errors: false,
        })
        .collect();
        let packages = [
            PackageIdentity {
                name: "meta".into(),
                lib_root: "/packages/meta/lib".into(),
            },
            PackageIdentity {
                name: "flutter".into(),
                lib_root: "/packages/flutter/lib".into(),
            },
        ];
        (sources, packages)
    }

    fn index(src: &str) -> TypeIndex {
        TypeIndex::from_program(&program(src))
    }

    // ── Recording ───────────────────────────────────────────────────────────────

    #[test]
    fn records_type_kinds() {
        let idx = index("class Box<T, U> {} mixin M {} enum E { a } extension type X(int it) {}");
        assert_eq!(idx.type_kind("Box"), Some(TypeKind::Class));
        assert_eq!(idx.type_kind("M"), Some(TypeKind::Mixin));
        assert_eq!(idx.type_kind("E"), Some(TypeKind::Enum));
        assert_eq!(idx.type_kind("X"), Some(TypeKind::ExtensionType));
        assert!(idx.is_known_type("Box"));
        assert!(!idx.is_known_type("Nope"));
    }

    #[test]
    fn mixin_class_is_its_own_kind() {
        let idx = index("mixin class MC {}");
        assert_eq!(idx.type_kind("MC"), Some(TypeKind::MixinClass));
    }

    // ── Member lookup ────────────────────────────────────────────────────────────

    #[test]
    fn own_member_is_found() {
        let idx = index("class C { void foo() {} int get bar => 1; int baz = 0; }");
        assert_eq!(
            idx.member_lookup("C", "foo"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(
            idx.member_lookup("C", "bar"),
            MemberResult::Found(MemberKind::Getter)
        );
        assert_eq!(
            idx.member_lookup("C", "baz"),
            MemberResult::Found(MemberKind::Field {
                writable: true,
                covariant: false
            })
        );
    }

    #[test]
    fn inherited_member_through_resolved_chain() {
        let idx = index("class A { void foo() {} } class B extends A {}");
        assert_eq!(
            idx.member_lookup("B", "foo"),
            MemberResult::Found(MemberKind::Method)
        );
    }

    #[test]
    fn absent_member_on_fully_resolved_chain_is_proven_absent() {
        // C's only ancestor is the implicit Object, which lacks `foo`.
        let idx = index("class C {}");
        assert_eq!(idx.member_lookup("C", "foo"), MemberResult::Absent);
        assert_eq!(idx.is_immutable_type("C"), Truth::No);
        // Object's own members remain findable through the implicit edge.
        assert_eq!(
            idx.member_lookup("C", "toString"),
            MemberResult::Found(MemberKind::Method)
        );
    }

    #[test]
    fn unresolved_supertype_poisons_absence_to_unknown() {
        // `Widget` is external/unknown → cannot prove `foo` absent.
        let idx = index("class C extends Widget {}");
        assert_eq!(idx.member_lookup("C", "foo"), MemberResult::Unknown);
        assert_eq!(idx.is_immutable_type("C"), Truth::Unknown);
        // But an own member is still found despite the unresolved ancestor.
        let idx2 = index("class C extends Widget { void foo() {} }");
        assert_eq!(
            idx2.member_lookup("C", "foo"),
            MemberResult::Found(MemberKind::Method)
        );
    }

    #[test]
    fn ambiguous_type_name_poisons_all_lookups() {
        let idx = index("class C { void foo() {} } class C { void bar() {} }");
        assert!(!idx.is_known_type("C"));
        assert_eq!(idx.member_lookup("C", "foo"), MemberResult::Unknown);
        assert_eq!(idx.member_lookup("C", "nope"), MemberResult::Unknown);
    }

    #[test]
    fn unknown_type_lookup_is_unknown() {
        let idx = index("class C {}");
        assert_eq!(idx.member_lookup("Ghost", "foo"), MemberResult::Unknown);
    }

    #[test]
    fn core_members_and_subtype_edges() {
        let idx = TypeIndex::default();
        assert_eq!(
            idx.member_lookup("List", "whereType"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(
            idx.member_lookup("List", "isEmpty"),
            MemberResult::Found(MemberKind::Getter)
        );
        assert_eq!(
            idx.member_lookup("String", "isEmpty"),
            MemberResult::Found(MemberKind::Getter)
        );
        assert_eq!(
            idx.member_lookup("LinkedHashMap", "putIfAbsent"),
            MemberResult::Found(MemberKind::Method)
        );
        // Inherited from num via int, and from Object via List.
        assert_eq!(
            idx.member_lookup("int", "toStringAsFixed"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(
            idx.member_lookup("List", "hashCode"),
            MemberResult::Found(MemberKind::Getter)
        );
        // Truly absent on a fully-known core chain.
        assert_eq!(idx.member_lookup("int", "nope"), MemberResult::Absent);
    }

    #[test]
    fn user_type_extending_core_inherits_core_members() {
        let idx = index("class MyList extends ListBase {}");
        // ListBase is unknown → unknown, poisoned.
        assert_eq!(idx.member_lookup("MyList", "add"), MemberResult::Unknown);

        let idx2 = index("class MyList implements List {}");
        assert_eq!(
            idx2.member_lookup("MyList", "add"),
            MemberResult::Found(MemberKind::Method)
        );
    }

    #[test]
    fn immutable_and_const_constructor_facts_follow_inheritance() {
        let idx = index(
            "import 'package:meta/meta.dart';
             @immutable class A { const A.named(); }
             class B extends A { final int value; B(this.value) : super.named(); }",
        );
        assert_eq!(idx.is_immutable_type("B"), Truth::Yes);
        assert_eq!(idx.constructor_is_const("A", Some("named")), Truth::Yes);
        assert_eq!(idx.constructor_is_const("Object", None), Truth::Yes);
    }

    #[test]
    fn unnamed_constructor_keys_match_explicit_new() {
        let idx = index(
            "class A { const A(); const A.named(); }
             class B extends A {}
             class C { C(); }
             class D { int value = 0; }
             class E { const E.named(); }",
        );
        for name in [None, Some("new"), Some("")] {
            assert_eq!(idx.constructor_is_const("Object", name), Truth::Yes);
            assert_eq!(idx.constructor_is_const("A", name), Truth::Yes);
            assert_eq!(idx.constructor_is_const("B", name), Truth::Yes);
            assert_eq!(idx.constructor_is_const("C", name), Truth::No);
            assert_eq!(idx.constructor_is_const("D", name), Truth::No);
            assert_eq!(idx.constructor_is_const("E", name), Truth::No);
        }
        assert_eq!(idx.constructor_is_const("A", Some("named")), Truth::Yes);
        assert_eq!(idx.constructor_is_const("A", Some("missing")), Truth::No);
    }

    // ── Subtype ──────────────────────────────────────────────────────────────────

    #[test]
    fn subtype_reflexive_and_direct() {
        let idx = index("class A {} class B extends A {}");
        assert_eq!(idx.is_subtype("A", "A"), Truth::Yes);
        assert_eq!(idx.is_subtype("B", "A"), Truth::Yes);
        assert_eq!(idx.is_subtype("B", "Object"), Truth::Yes);
    }

    #[test]
    fn subtype_transitive_and_via_implements_with() {
        let idx = index(
            "class A {} class B extends A {} class C extends B {}
             mixin M {} class D with M implements A {}",
        );
        assert_eq!(idx.is_subtype("C", "A"), Truth::Yes);
        assert_eq!(idx.is_subtype("D", "M"), Truth::Yes);
        assert_eq!(idx.is_subtype("D", "A"), Truth::Yes);
    }

    #[test]
    fn subtype_proven_no_on_resolved_chains() {
        let idx = index("class A {} class B {}");
        assert_eq!(idx.is_subtype("A", "B"), Truth::No);
        // Core chains too.
        assert_eq!(idx.is_subtype("String", "int"), Truth::No);
        assert_eq!(idx.is_subtype("List", "Iterable"), Truth::Yes);
        assert_eq!(idx.is_subtype("int", "num"), Truth::Yes);
    }

    #[test]
    fn subtype_unknown_on_unresolved_edge() {
        let idx = index("class B extends Widget {}");
        // Could be a subtype of anything Widget extends → cannot prove no.
        assert_eq!(idx.is_subtype("B", "State"), Truth::Unknown);
        // But the direct, named ancestor is still a proven Yes.
        assert_eq!(idx.is_subtype("B", "Widget"), Truth::Yes);
    }

    #[test]
    fn subtype_ambiguous_is_unknown() {
        let idx = index("class C {} class C {} class D extends C {}");
        assert_eq!(idx.is_subtype("D", "Object"), Truth::Yes);
        // Passing through ambiguous C poisons the rest of the chain.
        assert_eq!(idx.is_subtype("D", "Foo"), Truth::Unknown);
    }

    #[test]
    fn subtype_handles_cycles_without_infinite_loop() {
        // Pathological mutual extends (illegal Dart, but recovery could yield it).
        let idx = index("class A extends B {} class B extends A {}");
        // Never resolves to Q, and the chain is a self-referential cycle that the
        // visited-set breaks; both A and B are known so it terminates.
        assert_eq!(idx.is_subtype("A", "B"), Truth::Yes);
        assert_eq!(idx.is_subtype("A", "Q"), Truth::No);
    }

    #[test]
    fn enum_is_subtype_of_enum_with_index_member() {
        let idx = index("enum Color { red, green }");
        assert_eq!(idx.is_subtype("Color", "Enum"), Truth::Yes);
        assert_eq!(
            idx.member_lookup("Color", "index"),
            MemberResult::Found(MemberKind::Getter)
        );
        // Enum constants are recorded as members.
        assert_eq!(
            idx.member_lookup("Color", "red"),
            MemberResult::Found(MemberKind::Field {
                writable: false,
                covariant: false
            })
        );
    }

    #[test]
    fn parse_error_declarations_have_no_resolved_shape() {
        use super::super::IdentitySource;
        let path = Path::new("/project/bad.dart");
        let falcon_dart_parser::ParseResult { program: bad, .. } = parse("class Bad extends {");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &bad,
                has_parse_errors: true,
            }],
            &[],
        );
        let index = TypeIndex::from_resolved_files(
            [(
                path,
                LibrarySource {
                    program: &bad,
                    has_parse_errors: true,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        );
        let identity = DeclarationIdentity::Project {
            library: identities.library(identities.file(path).unwrap()),
            name: "Bad".into(),
        };
        assert_eq!(
            index.member_lookup_identity(&identity, "m"),
            MemberResult::Unknown
        );
        assert_eq!(
            index.is_subtype_identity(&identity, &DeclarationIdentity::OBJECT),
            Truth::Unknown
        );
        assert_eq!(index.is_immutable_type_identity(&identity), Truth::Unknown);
        assert_eq!(index.type_kind_identity(&identity), None);
    }

    #[test]
    fn no_parse_errors_leaves_index_complete() {
        let good = program("class Good { void m() {} }");
        let idx = TypeIndex::from_program(&good);
        assert_eq!(
            idx.member_lookup("Good", "m"),
            MemberResult::Found(MemberKind::Method)
        );
    }

    #[test]
    fn user_declaration_colliding_with_core_name_is_ambiguous() {
        // A project that redeclares a core name (e.g. its own `List`) cannot be
        // disambiguated from `dart:core`'s `List`, so every lookup poisons.
        let idx = index("class List { void mine() {} }");
        assert!(!idx.is_known_type("List"));
        assert_eq!(idx.member_lookup("List", "mine"), MemberResult::Unknown);
        assert_eq!(idx.member_lookup("List", "add"), MemberResult::Unknown);
        assert_eq!(idx.is_subtype("List", "Iterable"), Truth::Unknown);
        // A non-colliding user type still resolves as a user type.
        let idx2 = index("class Widgetish { void mine() {} }");
        assert_eq!(
            idx2.member_lookup("Widgetish", "mine"),
            MemberResult::Found(MemberKind::Method)
        );
    }

    #[test]
    fn unresolved_parts_poison_absence_but_not_presence() {
        let idx = index("part 'other.dart'; class C { void own() {} }");
        // A member could live in the unseen part → cannot prove absence.
        assert_eq!(idx.member_lookup("C", "ghost"), MemberResult::Unknown);
        assert_eq!(idx.is_immutable_type("C"), Truth::Unknown);
        // A member we *do* see is still a definite Found.
        assert_eq!(
            idx.member_lookup("C", "own"),
            MemberResult::Found(MemberKind::Method)
        );
    }

    #[test]
    fn from_program_flags_files_with_parts_as_unresolved() {
        let idx = index("part 'other.dart'; class C {}");
        // The part could add members → absence is not provable.
        assert_eq!(idx.member_lookup("C", "ghost"), MemberResult::Unknown);
    }

    #[test]
    fn multi_file_project_index() {
        use super::super::IdentitySource;
        let a_path = Path::new("/project/a.dart");
        let b_path = Path::new("/project/b.dart");
        let a = program("class Animal { void breathe() {} }");
        let b = program("import 'a.dart'; class Dog extends Animal { void bark() {} }");
        let sources = [
            IdentitySource {
                path: a_path,
                program: &a,
                has_parse_errors: false,
            },
            IdentitySource {
                path: b_path,
                program: &b,
                has_parse_errors: false,
            },
        ];
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let idx = TypeIndex::from_resolved_files(
            sources.iter().map(|source| {
                (
                    source.path,
                    LibrarySource {
                        program: source.program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let library = identities.library(identities.file(b_path).unwrap());
        let dog = identities
            .resolve_declaration(library, "Dog".into())
            .unwrap();
        let animal = identities
            .resolve_declaration(library, "Animal".into())
            .unwrap();
        assert_eq!(
            idx.member_lookup_identity(&dog, "breathe"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(idx.is_subtype_identity(&dog, &animal), Truth::Yes);
        assert_eq!(idx.type_kind_identity(&animal), Some(TypeKind::Class));
        assert_eq!(
            idx.supertype_matches_identity(&dog, |identity| identity == &animal),
            Truth::Yes
        );
        assert_eq!(
            idx.supertype_matches_identity(&animal, |identity| identity == &dog),
            Truth::No
        );
    }

    #[test]
    fn same_names_and_prefixed_ancestors_keep_library_identity() {
        use super::super::{IdentitySource, SignatureIndex};
        let left_path = Path::new("/project/left.dart");
        let right_path = Path::new("/project/right.dart");
        let app_path = Path::new("/project/app.dart");
        let left =
            program("class Base { const Base(); void left() {} } class String { void local() {} }");
        let right = program("class Base { Base(); void right() {} }");
        let app = program(
            "import 'left.dart' as l; import 'right.dart' as r; class L extends l.Base {} class R extends r.Base {}",
        );
        let files = [(left_path, &left), (right_path, &right), (app_path, &app)];
        let sources: Vec<_> = files
            .iter()
            .map(|(path, program)| IdentitySource {
                path,
                program,
                has_parse_errors: false,
            })
            .collect();
        let identities = IdentityIndex::from_project_files(&sources, &[]);
        let index = TypeIndex::from_resolved_files(
            files.iter().map(|(path, program)| {
                (
                    *path,
                    LibrarySource {
                        program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let signature_files: Vec<_> = files
            .iter()
            .map(|(path, program)| (path.to_path_buf(), *program))
            .collect();
        let signatures = SignatureIndex::from_project_files(&signature_files, &identities, &index);
        let resolve = |path, name: QualifiedName<'_>| {
            let library = identities.library(identities.file(path).unwrap());
            identities.resolve_declaration(library, name).unwrap()
        };
        let left_base = resolve(app_path, QualifiedName::prefixed("l", "Base"));
        let right_base = resolve(app_path, QualifiedName::prefixed("r", "Base"));
        let l = resolve(app_path, "L".into());
        let r = resolve(app_path, "R".into());
        assert_ne!(left_base, right_base);
        assert_eq!(
            index.member_lookup_identity(&l, "left"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(
            index.member_lookup_identity(&l, "right"),
            MemberResult::Absent
        );
        assert_eq!(
            index.member_lookup_identity(&r, "right"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(index.is_subtype_identity(&l, &left_base), Truth::Yes);
        assert_eq!(index.is_subtype_identity(&l, &right_base), Truth::No);
        assert!(signatures.is_const_constructor(&left_base, "new"));
        assert!(!signatures.is_const_constructor(&right_base, "new"));
        let local_string = resolve(left_path, "String".into());
        assert_eq!(
            index.member_lookup_identity(&local_string, "local"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(
            index.is_subtype_identity(
                &local_string,
                &DeclarationIdentity::sdk("dart:core", "String")
            ),
            Truth::No
        );
    }

    #[test]
    fn unrelated_parse_errors_do_not_poison_resolved_library_shapes() {
        use super::super::IdentitySource;
        let good_path = Path::new("/project/good.dart");
        let bad_path = Path::new("/project/bad.dart");
        let good = program("class C { void known() {} }");
        let bad = program("class C {}");
        let identities = IdentityIndex::from_project_files(
            &[
                IdentitySource {
                    path: good_path,
                    program: &good,
                    has_parse_errors: false,
                },
                IdentitySource {
                    path: bad_path,
                    program: &bad,
                    has_parse_errors: true,
                },
            ],
            &[],
        );
        let index = TypeIndex::from_resolved_files(
            [
                (
                    good_path,
                    LibrarySource {
                        program: &good,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                ),
                (
                    bad_path,
                    LibrarySource {
                        program: &bad,
                        has_parse_errors: true,
                        has_unresolved_parts: false,
                    },
                ),
            ],
            &identities,
        );
        let library = identities.library(identities.file(good_path).unwrap());
        let identity = identities.resolve_declaration(library, "C".into()).unwrap();
        assert_eq!(
            index.member_lookup_identity(&identity, "known"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(
            index.member_lookup_identity(&identity, "missing"),
            MemberResult::Absent
        );
    }

    #[test]
    fn resolved_immutable_annotations_preserve_aliases_and_uncertainty() {
        use super::super::IdentitySource;
        let path = Path::new("/project/annotations.dart");
        let meta = program("class Immutable { const Immutable(); } const immutable = Immutable();");
        let flutter = program("export 'package:meta/meta.dart' show immutable;");
        for (source, expected) in [
            (
                "import 'package:meta/meta.dart'; @immutable class C {} class Child extends C {}",
                Truth::Yes,
            ),
            (
                "import 'package:meta/meta.dart'; @immutable class C {} @Deprecated('x') class Child extends C {}",
                Truth::Yes,
            ),
            (
                "import 'package:meta/meta.dart'; @immutable class C {} @pragma('x') class Child extends C {}",
                Truth::Yes,
            ),
            (
                "@Deprecated('x') class C {} @pragma('x') class Child extends C {}",
                Truth::No,
            ),
            (
                "import 'package:meta/meta.dart' as m; @m.immutable class C {} class Child extends C {}",
                Truth::Yes,
            ),
            (
                "import 'package:meta/meta.dart' as m; @m.Immutable() class C {} class Child extends C {}",
                Truth::Yes,
            ),
            (
                "import 'package:meta/meta.dart'; const immutable = 0; @immutable class C {} class Child extends C {}",
                Truth::No,
            ),
            (
                "import 'package:meta/meta.dart' hide immutable; @immutable class C {} class Child extends C {}",
                Truth::Unknown,
            ),
            (
                "import 'package:meta/meta.dart'; import 'missing.dart'; @immutable class C {} class Child extends C {}",
                Truth::Unknown,
            ),
            (
                "import 'package:flutter/foundation.dart'; @immutable class C {} class Child extends C {}",
                Truth::Yes,
            ),
            (
                "import 'package:flutter/widgets.dart' as f; @f.immutable class C {} class Child extends C {}",
                Truth::Yes,
            ),
            (
                "import 'package:flutter/material.dart'; @immutable class C {} class Child extends C {}",
                Truth::Yes,
            ),
            (
                "@immutable class C {} class Child extends C {}",
                Truth::Unknown,
            ),
            (
                "import 'missing.dart'; @immutable class C {} class Child extends C {}",
                Truth::Unknown,
            ),
            ("class C {} class Child extends C {}", Truth::No),
        ] {
            let program = program(source);
            let (mut sources, packages) = annotation_packages(&meta, &flutter);
            sources.push(IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            });
            let identities = IdentityIndex::from_project_files(&sources, &packages);
            let index = TypeIndex::from_resolved_files(
                [(
                    path,
                    LibrarySource {
                        program: &program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )],
                &identities,
            );
            let library = identities.library(identities.file(path).unwrap());
            for name in ["C", "Child"] {
                let identity = identities
                    .resolve_declaration(library, name.into())
                    .unwrap();
                assert_eq!(
                    index.is_immutable_type_identity(&identity),
                    expected,
                    "{source}: {name}"
                );
                assert_eq!(
                    index.superclass_chain_is_immutable_identity(&identity),
                    expected,
                    "{source}: {name}"
                );
            }
        }
    }

    #[test]
    fn incomplete_imports_and_reexports_do_not_bind_core_names() {
        use super::super::IdentitySource;
        let bad_path = Path::new("/project/bad.dart");
        let facade_path = Path::new("/project/facade.dart");
        let app_path = Path::new("/project/app.dart");
        let clean_path = Path::new("/project/clean.dart");
        for parse_error in [false, true] {
            let bad = program(if parse_error {
                "class Duration {} class Map {}"
            } else {
                "part 'missing.dart'; class Duration {} class Map {}"
            });
            let facade = program("export 'bad.dart';");
            let app = program("import 'facade.dart'; class C extends Duration {}");
            let clean = program("class Good { void own() {} }");
            let sources = [
                IdentitySource {
                    path: bad_path,
                    program: &bad,
                    has_parse_errors: parse_error,
                },
                IdentitySource {
                    path: facade_path,
                    program: &facade,
                    has_parse_errors: false,
                },
                IdentitySource {
                    path: app_path,
                    program: &app,
                    has_parse_errors: false,
                },
                IdentitySource {
                    path: clean_path,
                    program: &clean,
                    has_parse_errors: false,
                },
            ];
            let identities = IdentityIndex::from_project_files(&sources, &[]);
            let index = TypeIndex::from_resolved_files(
                sources.iter().map(|source| {
                    (
                        source.path,
                        LibrarySource {
                            program: source.program,
                            has_parse_errors: source.has_parse_errors,
                            has_unresolved_parts: source.path == bad_path && !parse_error,
                        },
                    )
                }),
                &identities,
            );
            let bad_library = identities.library(identities.file(bad_path).unwrap());
            let app_library = identities.library(identities.file(app_path).unwrap());
            for name in ["Map", "Duration"] {
                assert!(
                    identities
                        .resolve_declaration(bad_library, name.into())
                        .is_none()
                );
                assert!(
                    identities
                        .resolve_declaration(app_library, name.into())
                        .is_none()
                );
            }
            let c = identities
                .resolve_declaration(app_library, "C".into())
                .unwrap();
            assert_eq!(
                index.is_subtype_identity(&c, &DeclarationIdentity::sdk("dart:core", "Duration")),
                Truth::Unknown
            );
            assert_eq!(
                index.member_lookup_identity(&c, "missing"),
                MemberResult::Unknown
            );
            let clean_library = identities.library(identities.file(clean_path).unwrap());
            let good = identities
                .resolve_declaration(clean_library, "Good".into())
                .unwrap();
            assert_eq!(
                index.member_lookup_identity(&good, "missing"),
                MemberResult::Absent
            );
        }
    }

    #[test]
    fn resolved_immutable_markers_follow_project_reexports_with_source_packages() {
        use super::super::IdentitySource;
        let facade_path = Path::new("/project/facade.dart");
        let app_path = Path::new("/project/app.dart");
        let meta = program("class Immutable { const Immutable(); } const immutable = Immutable();");
        let flutter = program("export 'package:meta/meta.dart' show immutable;");
        for exported in ["package:meta/meta.dart", "package:flutter/foundation.dart"] {
            let facade = program(&format!("export '{exported}' show immutable;"));
            let app = program("import 'facade.dart' as f; @f.immutable class C {}");
            let (mut sources, packages) = annotation_packages(&meta, &flutter);
            sources.extend([
                IdentitySource {
                    path: facade_path,
                    program: &facade,
                    has_parse_errors: false,
                },
                IdentitySource {
                    path: app_path,
                    program: &app,
                    has_parse_errors: false,
                },
            ]);
            let identities = IdentityIndex::from_project_files(&sources, &packages);
            let index = TypeIndex::from_resolved_files(
                sources.iter().map(|source| {
                    (
                        source.path,
                        LibrarySource {
                            program: source.program,
                            has_parse_errors: false,
                            has_unresolved_parts: false,
                        },
                    )
                }),
                &identities,
            );
            let library = identities.library(identities.file(app_path).unwrap());
            let marker = identities
                .resolve_imported_member(library, QualifiedName::prefixed("f", "immutable"))
                .unwrap();
            assert!(marker.is_package("meta", "immutable"));
            let source_marker = identities
                .resolve_value_declaration(library, QualifiedName::prefixed("f", "immutable"))
                .unwrap();
            assert!(matches!(source_marker, DeclarationIdentity::Project { .. }));
            assert!(identities.is_package_declaration(&source_marker, "meta", "immutable"));
            let identity = identities.resolve_declaration(library, "C".into()).unwrap();
            assert_eq!(index.is_immutable_type_identity(&identity), Truth::Yes);
            assert_eq!(
                index.superclass_chain_is_immutable_identity(&identity),
                Truth::Yes
            );
        }
    }
}
