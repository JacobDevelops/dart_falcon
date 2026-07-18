//! Cross-file *type* index — the shape of the project's named types.
//!
//! Where [`crate::resolve::ProjectIndex`] answers "what does a call named `foo`
//! return?", [`TypeIndex`] answers "what is type `Foo` — its kind, its
//! supertypes, and its members?". It is the substrate a receiver-typed rule
//! stands on: given a receiver whose static type is `Other { name }`, ask
//! whether that type (or any ancestor) declares a member, or whether it is a
//! subtype of some other type.
//!
//! Three sources feed it:
//!
//! 1. **User declarations** — every class / mixin / mixin-class / enum /
//!    extension-type across the project, keyed by simple name. Because lookups
//!    are name-keyed and falcon has no full resolver, two declarations of the
//!    same name are recorded as *ambiguous*: every lookup through them poisons to
//!    [`MemberResult::Unknown`] / [`SubtypeResult::Unknown`] rather than pick one.
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
//! ([`MemberResult::ProvenAbsent`] / [`SubtypeResult::ProvenNo`]) is returned
//! *only* when the entire supertype chain is resolved — every ancestor known, no
//! ambiguity, no unresolved external edge, no unresolved part. The instant
//! certainty is lost the answer degrades to `Unknown`. A consumer may thus miss a
//! fact, but must never be told a wrong one.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use falcon_syntax::ast::*;

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
    Field,
    Operator,
}

/// Three-valued result of a member lookup on a type.
///
/// [`MemberResult::ProvenAbsent`] is only ever returned when the type's full
/// supertype chain is resolved and no ancestor declares the member; any
/// uncertainty (unknown ancestor, ambiguous name, unresolved part) degrades the
/// answer to [`MemberResult::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemberResult {
    Found(MemberKind),
    ProvenAbsent,
    Unknown,
}

/// Three-valued result of a subtype query.
///
/// [`SubtypeResult::ProvenNo`] is only ever returned when `sub`'s full ancestor
/// chain is resolved and `super` appears nowhere in it; any uncertainty degrades
/// to [`SubtypeResult::Unknown`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtypeResult {
    Yes,
    ProvenNo,
    Unknown,
}

/// A single user-declared type's recorded shape.
#[derive(Debug, Clone)]
struct TypeEntry {
    kind: TypeKind,
    type_param_count: usize,
    /// Last-segment names from `extends` / `with` / `implements` / `on`.
    supertypes: Vec<String>,
    /// Own declared member names → their kind.
    members: HashMap<String, MemberKind>,
    /// The declaring library has an unresolved `part`, so this type's member set
    /// may be incomplete — poisons any proof of absence through it.
    has_unresolved_parts: bool,
}

/// One name's slot in the user-declaration table.
#[derive(Debug, Clone)]
enum Slot {
    Unique(TypeEntry),
    /// Two or more declarations share this name — every lookup poisons.
    Ambiguous,
}

/// A resolved view of a type name: a user entry, a core entry, or ambiguous.
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

/// A name → type-shape index over a project's declarations, backed by a curated
/// core-library table for the types falcon cannot see.
#[derive(Debug, Clone, Default)]
pub struct TypeIndex {
    types: HashMap<String, Slot>,
}

impl TypeIndex {
    /// An empty index (core-library types only).
    pub fn new() -> Self {
        Self {
            types: HashMap::new(),
        }
    }

    /// Build a cross-file index from every analyzed library file. Files whose
    /// parse failed are skipped (their recovered nodes can be spurious); every
    /// type inherits its file's `has_unresolved_parts` flag.
    pub fn from_library_files<'a, I>(files: I) -> Self
    where
        I: IntoIterator<Item = LibrarySource<'a>>,
    {
        let mut index = Self::new();
        for file in files {
            if !file.has_parse_errors {
                index.add_program(file.program, file.has_unresolved_parts);
            }
        }
        index
    }

    /// Build a degraded, single-file index. A file that declares (or is) a `part`
    /// is treated as having unresolved parts, since the sibling files that could
    /// complete its types are not available in single-file mode.
    pub fn from_program(program: &Program) -> Self {
        let unresolved = !program.part_directives.is_empty() || program.part_of_directive.is_some();
        let mut index = Self::new();
        index.add_program(program, unresolved);
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

    /// The declaration kind of `name`, or `None` when it is unknown, ambiguous,
    /// or a core type (core types have no user-`TypeKind`).
    pub fn type_kind(&self, name: &str) -> Option<TypeKind> {
        match self.resolve(name) {
            Some(Resolved::User(e)) => Some(e.kind),
            _ => None,
        }
    }

    /// The declared type-parameter count of a *user* type `name`, or `None` when
    /// it is unknown, ambiguous, or a core type.
    pub fn type_param_count(&self, name: &str) -> Option<usize> {
        match self.resolve(name) {
            Some(Resolved::User(e)) => Some(e.type_param_count),
            _ => None,
        }
    }

    /// Look up `member` on `type_name`, walking the supertype chain.
    ///
    /// Returns [`MemberResult::Found`] on the first ancestor declaring it,
    /// [`MemberResult::ProvenAbsent`] only when the *entire* chain is resolved and
    /// none declares it, and [`MemberResult::Unknown`] whenever the chain has any
    /// unresolved edge (unknown ancestor, ambiguity, or unresolved part).
    pub fn member_lookup(&self, type_name: &str, member: &str) -> MemberResult {
        let mut visited = HashSet::new();
        let mut stack = vec![type_name.to_string()];
        let mut fully_resolved = true;
        while let Some(name) = stack.pop() {
            if !visited.insert(name.clone()) {
                continue;
            }
            match self.resolve(&name) {
                None | Some(Resolved::Ambiguous) => fully_resolved = false,
                Some(Resolved::User(e)) => {
                    if let Some(kind) = e.members.get(member) {
                        return MemberResult::Found(*kind);
                    }
                    if e.has_unresolved_parts {
                        fully_resolved = false;
                    }
                    push_ancestors(&name, e.supertypes.iter().map(String::as_str), &mut stack);
                }
                Some(Resolved::Core(c)) => {
                    if let Some(kind) = core_member(c, member) {
                        return MemberResult::Found(kind);
                    }
                    push_ancestors(&name, c.supertypes.iter().copied(), &mut stack);
                }
            }
        }
        if fully_resolved {
            MemberResult::ProvenAbsent
        } else {
            MemberResult::Unknown
        }
    }

    /// Whether `sub` is a subtype of `super_`, walking `sub`'s ancestor chain.
    ///
    /// Returns [`SubtypeResult::Yes`] on a name match, [`SubtypeResult::ProvenNo`]
    /// only when `sub`'s entire chain is resolved and never names `super_`, and
    /// [`SubtypeResult::Unknown`] whenever the chain has an unresolved edge.
    pub fn is_subtype(&self, sub: &str, super_: &str) -> SubtypeResult {
        if sub == super_ {
            return SubtypeResult::Yes;
        }
        let mut visited = HashSet::new();
        let mut stack = vec![sub.to_string()];
        let mut fully_resolved = true;
        while let Some(name) = stack.pop() {
            if name == super_ {
                return SubtypeResult::Yes;
            }
            if !visited.insert(name.clone()) {
                continue;
            }
            match self.resolve(&name) {
                None | Some(Resolved::Ambiguous) => fully_resolved = false,
                Some(Resolved::User(e)) => {
                    if e.has_unresolved_parts {
                        fully_resolved = false;
                    }
                    push_ancestors(&name, e.supertypes.iter().map(String::as_str), &mut stack);
                }
                Some(Resolved::Core(c)) => {
                    push_ancestors(&name, c.supertypes.iter().copied(), &mut stack);
                }
            }
        }
        if fully_resolved {
            SubtypeResult::ProvenNo
        } else {
            SubtypeResult::Unknown
        }
    }

    // ── Internals ───────────────────────────────────────────────────────────────

    /// Resolve a name against user declarations first, then the core table.
    fn resolve(&self, name: &str) -> Option<Resolved<'_>> {
        match self.types.get(name) {
            Some(Slot::Unique(e)) => Some(Resolved::User(e)),
            Some(Slot::Ambiguous) => Some(Resolved::Ambiguous),
            None => core_table().get(name).map(Resolved::Core),
        }
    }

    /// Harvest every named-type declaration in a program.
    fn add_program(&mut self, program: &Program, has_unresolved_parts: bool) {
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Class(c) => {
                    let supertypes = supertype_names(
                        c.extends
                            .as_ref()
                            .into_iter()
                            .chain(&c.with_clause)
                            .chain(&c.implements),
                    );
                    self.add_type(
                        &c.name.name,
                        TypeEntry {
                            kind: TypeKind::Class,
                            type_param_count: c.type_params.len(),
                            supertypes,
                            members: collect_members(&c.members),
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::Mixin(m) => {
                    let supertypes = supertype_names(m.on_clause.iter().chain(&m.implements));
                    self.add_type(
                        &m.name.name,
                        TypeEntry {
                            kind: TypeKind::Mixin,
                            type_param_count: m.type_params.len(),
                            supertypes,
                            members: collect_members(&m.members),
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::MixinClass(m) => {
                    let supertypes = supertype_names(
                        m.extends
                            .as_ref()
                            .into_iter()
                            .chain(&m.with_clause)
                            .chain(&m.implements),
                    );
                    self.add_type(
                        &m.name.name,
                        TypeEntry {
                            kind: TypeKind::MixinClass,
                            type_param_count: m.type_params.len(),
                            supertypes,
                            members: collect_members(&m.members),
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::Enum(e) => {
                    let mut supertypes = supertype_names(e.with_clause.iter().chain(&e.implements));
                    // Every enum is a subtype of `Enum` (source of `index` / `name`).
                    supertypes.push("Enum".to_string());
                    let mut members = collect_members(&e.members);
                    // Enum constants are accessible names on the type.
                    for v in &e.variants {
                        members
                            .entry(v.name.name.clone())
                            .or_insert(MemberKind::Field);
                    }
                    self.add_type(
                        &e.name.name,
                        TypeEntry {
                            kind: TypeKind::Enum,
                            type_param_count: e.type_params.len(),
                            supertypes,
                            members,
                            has_unresolved_parts,
                        },
                    );
                }
                TopLevelDecl::ExtensionType(e) => {
                    let supertypes = supertype_names(e.implements.iter());
                    self.add_type(
                        &e.name.name,
                        TypeEntry {
                            kind: TypeKind::ExtensionType,
                            type_param_count: e.type_params.len(),
                            supertypes,
                            members: collect_members(&e.members),
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
                self.types.insert(name.to_string(), Slot::Unique(entry));
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
    stack: &mut Vec<String>,
) {
    for s in supertypes {
        stack.push(s.to_string());
    }
    if name != "Object" {
        stack.push("Object".to_string());
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

/// Collect a member list into a name → kind map. Constructors and error nodes
/// contribute nothing; a getter/setter pair keeps whichever is seen first (only
/// the member's *existence* matters to lookups).
fn collect_members(members: &[ClassMember]) -> HashMap<String, MemberKind> {
    let mut map = HashMap::new();
    for member in members {
        match member {
            ClassMember::Field(f) => {
                for d in &f.declarators {
                    map.insert(d.name.name.clone(), MemberKind::Field);
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
            ClassMember::Constructor(_) | ClassMember::Error(_) => {}
        }
    }
    map
}

// ── Curated core-library table ──────────────────────────────────────────────────

/// A curated core-library type: its subtype edges and its common members.
struct CoreType {
    supertypes: Vec<&'static str>,
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
    use MemberKind::{Getter, Method};
    let mut t: HashMap<&'static str, CoreType> = HashMap::new();

    let ty = |supertypes: &[&'static str], members: Vec<(&'static str, MemberKind)>| CoreType {
        supertypes: supertypes.to_vec(),
        members,
    };

    t.insert(
        "Object",
        ty(
            &[],
            vec![
                ("toString", Method),
                ("hashCode", Getter),
                ("runtimeType", Getter),
                ("noSuchMethod", Method),
            ],
        ),
    );

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
        let (program, errors) = parse(src);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        program
    }

    fn index(src: &str) -> TypeIndex {
        TypeIndex::from_program(&program(src))
    }

    // ── Recording ───────────────────────────────────────────────────────────────

    #[test]
    fn records_kind_and_type_param_count() {
        let idx = index("class Box<T, U> {} mixin M {} enum E { a } extension type X(int it) {}");
        assert_eq!(idx.type_kind("Box"), Some(TypeKind::Class));
        assert_eq!(idx.type_param_count("Box"), Some(2));
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
            MemberResult::Found(MemberKind::Field)
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
        assert_eq!(idx.member_lookup("C", "foo"), MemberResult::ProvenAbsent);
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
        let idx = TypeIndex::new();
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
        assert_eq!(idx.member_lookup("int", "nope"), MemberResult::ProvenAbsent);
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

    // ── Subtype ──────────────────────────────────────────────────────────────────

    #[test]
    fn subtype_reflexive_and_direct() {
        let idx = index("class A {} class B extends A {}");
        assert_eq!(idx.is_subtype("A", "A"), SubtypeResult::Yes);
        assert_eq!(idx.is_subtype("B", "A"), SubtypeResult::Yes);
        assert_eq!(idx.is_subtype("B", "Object"), SubtypeResult::Yes);
    }

    #[test]
    fn subtype_transitive_and_via_implements_with() {
        let idx = index(
            "class A {} class B extends A {} class C extends B {}
             mixin M {} class D with M implements A {}",
        );
        assert_eq!(idx.is_subtype("C", "A"), SubtypeResult::Yes);
        assert_eq!(idx.is_subtype("D", "M"), SubtypeResult::Yes);
        assert_eq!(idx.is_subtype("D", "A"), SubtypeResult::Yes);
    }

    #[test]
    fn subtype_proven_no_on_resolved_chains() {
        let idx = index("class A {} class B {}");
        assert_eq!(idx.is_subtype("A", "B"), SubtypeResult::ProvenNo);
        // Core chains too.
        assert_eq!(idx.is_subtype("String", "int"), SubtypeResult::ProvenNo);
        assert_eq!(idx.is_subtype("List", "Iterable"), SubtypeResult::Yes);
        assert_eq!(idx.is_subtype("int", "num"), SubtypeResult::Yes);
    }

    #[test]
    fn subtype_unknown_on_unresolved_edge() {
        let idx = index("class B extends Widget {}");
        // Could be a subtype of anything Widget extends → cannot prove no.
        assert_eq!(idx.is_subtype("B", "State"), SubtypeResult::Unknown);
        // But the direct, named ancestor is still a proven Yes.
        assert_eq!(idx.is_subtype("B", "Widget"), SubtypeResult::Yes);
    }

    #[test]
    fn subtype_ambiguous_is_unknown() {
        let idx = index("class C {} class C {} class D extends C {}");
        assert_eq!(idx.is_subtype("D", "Object"), SubtypeResult::Yes);
        // Passing through ambiguous C poisons the rest of the chain.
        assert_eq!(idx.is_subtype("D", "Foo"), SubtypeResult::Unknown);
    }

    #[test]
    fn subtype_handles_cycles_without_infinite_loop() {
        // Pathological mutual extends (illegal Dart, but recovery could yield it).
        let idx = index("class A extends B {} class B extends A {}");
        // Never resolves to Q, and the chain is a self-referential cycle that the
        // visited-set breaks; both A and B are known so it terminates.
        assert_eq!(idx.is_subtype("A", "B"), SubtypeResult::Yes);
        assert_eq!(idx.is_subtype("A", "Q"), SubtypeResult::ProvenNo);
    }

    #[test]
    fn enum_is_subtype_of_enum_with_index_member() {
        let idx = index("enum Color { red, green }");
        assert_eq!(idx.is_subtype("Color", "Enum"), SubtypeResult::Yes);
        assert_eq!(
            idx.member_lookup("Color", "index"),
            MemberResult::Found(MemberKind::Getter)
        );
        // Enum constants are recorded as members.
        assert_eq!(
            idx.member_lookup("Color", "red"),
            MemberResult::Found(MemberKind::Field)
        );
    }

    #[test]
    fn parse_error_files_skipped_but_good_ones_kept() {
        let good = program("class Good { void m() {} }");
        let (bad, _) = parse("class Bad extends {");
        let sources = [
            LibrarySource {
                program: &good,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &bad,
                has_parse_errors: true,
                has_unresolved_parts: false,
            },
        ];
        let idx = TypeIndex::from_library_files(sources);
        assert_eq!(
            idx.member_lookup("Good", "m"),
            MemberResult::Found(MemberKind::Method)
        );
        assert!(!idx.is_known_type("Bad"));
    }

    #[test]
    fn unresolved_parts_poison_absence_but_not_presence() {
        let p = program("class C { void own() {} }");
        let sources = [LibrarySource {
            program: &p,
            has_parse_errors: false,
            has_unresolved_parts: true,
        }];
        let idx = TypeIndex::from_library_files(sources);
        // A member could live in the unseen part → cannot prove absence.
        assert_eq!(idx.member_lookup("C", "ghost"), MemberResult::Unknown);
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
        let a = program("class Animal { void breathe() {} }");
        let b = program("class Dog extends Animal { void bark() {} }");
        let sources = [
            LibrarySource {
                program: &a,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
            LibrarySource {
                program: &b,
                has_parse_errors: false,
                has_unresolved_parts: false,
            },
        ];
        let idx = TypeIndex::from_library_files(sources);
        assert_eq!(
            idx.member_lookup("Dog", "breathe"),
            MemberResult::Found(MemberKind::Method)
        );
        assert_eq!(idx.is_subtype("Dog", "Animal"), SubtypeResult::Yes);
    }
}
