//! Cross-file declaration index.
//!
//! [`ProjectIndex`] maps a declaration *name* to its declared return type, built
//! from every analyzed file (or a single [`Program`] for degraded per-file use).
//! It is the minimum needed by `avoid-ignoring-return-values`: given a call like
//! `foo()` or `x.foo()`, ask [`ProjectIndex::return_type`] whether the callee
//! returns `void` (safe to discard) or a value (worth flagging).
//!
//! Two sources feed the index:
//!
//! 1. **User declarations** — top-level functions, methods, getters, and setters
//!    across the project, keyed by simple name. Because the index is name-keyed
//!    and receiver-agnostic (there is no type resolution to bind `x.foo` to a
//!    class), two declarations of the same name that *disagree* on their return
//!    type are recorded as ambiguous and resolve to [`StaticType::Unknown`].
//!    Setters are always `void`.
//! 2. **A curated builtin table** — Dart core-library members whose return
//!    matters to the rule. This is a heuristic *dominant-return* table, not
//!    ground truth: entries capture the conventional return of a name across its
//!    common receivers (mirroring the intent of the rule's old side-effect
//!    allowlist), which is sound enough for a rule that is off by default.
//!
//! User declarations take precedence over builtins for the same name.

use std::collections::HashMap;

use falcon_syntax::ast::*;

use super::StaticType;

/// What the index knows about a name's return type.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ReturnFact {
    /// A curated builtin fact; a user declaration may override it.
    Builtin(StaticType),
    /// Exactly one user declaration seen so far.
    Known(StaticType),
    /// Multiple user declarations disagree — not safe to state a type.
    Ambiguous,
}

/// A name → declared-return-type index over a set of files.
#[derive(Debug, Clone, Default)]
pub struct ProjectIndex {
    decls: HashMap<String, ReturnFact>,
}

impl ProjectIndex {
    /// An empty index (builtins only are added by [`ProjectIndex::with_builtins`]).
    pub fn new() -> Self {
        Self {
            decls: HashMap::new(),
        }
    }

    /// Build a cross-file index from every analyzed file, seeded with the
    /// builtin table. Files with parse errors are skipped for declaration
    /// harvesting (their recovered nodes can be spurious).
    pub fn from_project_files<'a, I>(files: I) -> Self
    where
        I: IntoIterator<Item = &'a ProgramSource<'a>>,
    {
        let mut index = Self::with_builtins();
        for file in files {
            if !file.has_parse_errors {
                index.add_program(file.program);
            }
        }
        index
    }

    /// Build a degraded, single-file index (this file's declarations plus the
    /// builtin table). Used where only one [`Program`] is available (e.g. the
    /// LSP's single open buffer, or a per-file CLI pass).
    pub fn from_program(program: &Program) -> Self {
        let mut index = Self::with_builtins();
        index.add_program(program);
        index
    }

    /// An index seeded with only the curated builtin table.
    pub fn with_builtins() -> Self {
        let mut decls = HashMap::new();
        for (name, ty) in builtin_returns() {
            decls.insert(name.to_string(), ReturnFact::Builtin(ty.clone()));
        }
        Self { decls }
    }

    /// The declared return type of `name`, or [`StaticType::Unknown`] when the
    /// name is absent, ambiguous, or genuinely unknown. Callers should treat a
    /// non-`Void` result as "returns a value"; only [`StaticType::Void`] is a
    /// positive statement that discarding the result is fine.
    pub fn return_type(&self, name: &str) -> StaticType {
        match self.decls.get(name) {
            Some(ReturnFact::Known(ty) | ReturnFact::Builtin(ty)) => ty.clone(),
            Some(ReturnFact::Ambiguous) | None => StaticType::Unknown,
        }
    }

    /// Harvest the return-type facts of every function/method/getter/setter in a
    /// program. User declarations override builtins; conflicting user
    /// declarations of the same name become [`ReturnFact::Ambiguous`].
    fn add_program(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(f) => {
                    let ty = if f.is_setter {
                        StaticType::Void
                    } else {
                        return_of(f.return_type.as_ref())
                    };
                    self.record(&f.name.name, ty);
                }
                TopLevelDecl::Class(c) => self.add_members(&c.members),
                TopLevelDecl::Mixin(m) => self.add_members(&m.members),
                TopLevelDecl::MixinClass(m) => self.add_members(&m.members),
                TopLevelDecl::Enum(e) => self.add_members(&e.members),
                TopLevelDecl::Extension(e) => self.add_members(&e.members),
                TopLevelDecl::ExtensionType(e) => self.add_members(&e.members),
                _ => {}
            }
        }
    }

    fn add_members(&mut self, members: &[ClassMember]) {
        for member in members {
            match member {
                ClassMember::Method(m) => {
                    self.record(&m.name.name, return_of(m.return_type.as_ref()))
                }
                ClassMember::Getter(g) => {
                    self.record(&g.name.name, return_of(g.return_type.as_ref()))
                }
                ClassMember::Setter(s) => self.record(&s.name.name, StaticType::Void),
                _ => {}
            }
        }
    }

    /// Record a *user* declaration's return type. A user fact always overrides a
    /// builtin (user code wins). Two user facts for the same name that disagree
    /// become [`ReturnFact::Ambiguous`]; agreeing ones stay known.
    fn record(&mut self, name: &str, ty: StaticType) {
        match self.decls.get(name) {
            None | Some(ReturnFact::Builtin(_)) => {
                self.decls.insert(name.to_string(), ReturnFact::Known(ty));
            }
            Some(ReturnFact::Ambiguous) => {}
            Some(ReturnFact::Known(existing)) => {
                if *existing != ty {
                    self.decls.insert(name.to_string(), ReturnFact::Ambiguous);
                }
            }
        }
    }
}

/// A file's parsed program plus its parse-error flag — the minimal shape
/// [`ProjectIndex::from_project_files`] needs, decoupled from the analysis
/// crate's `ProjectFile` (which additionally owns the source text). The driver
/// can build these by borrowing its already-collected `ProjectFile`s.
pub struct ProgramSource<'a> {
    pub program: &'a Program,
    pub has_parse_errors: bool,
}

/// Map a declared return type to a [`StaticType`]. An *omitted* return type is
/// `Unknown` (Dart infers it; we do not), an explicit `void` is `Void`.
fn return_of(ty: Option<&DartType>) -> StaticType {
    match ty {
        None => StaticType::Unknown,
        Some(t) => StaticType::from_dart_type(t),
    }
}

/// Curated, receiver-agnostic *dominant-return* table for Dart core-library
/// members whose return matters to `avoid-ignoring-return-values`.
///
/// `void` entries are the members whose result is conventionally discarded
/// (collection mutation, listeners, sinks, lifecycle, navigation, logging) —
/// this reproduces the intent of the rule's former `SIDE_EFFECT_NAMES` list.
/// Non-`void` entries are the well-known pure members whose result *should*
/// normally be used. Ambiguous names (e.g. `remove` — `bool` on `List`, `V?` on
/// `Map`, and side-effecting elsewhere) are deliberately omitted so they resolve
/// to `Unknown` rather than a possibly-wrong fact.
fn builtin_returns() -> &'static [(&'static str, StaticType)] {
    // Built lazily once; `StaticType` is not const-constructible (owns a String
    // in `Other`), so use a `OnceLock`.
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<(&'static str, StaticType)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let void = || StaticType::Void;
        let string = || StaticType::String { nullable: false };
        let int = || StaticType::Int { nullable: false };
        let boolean = || StaticType::Bool { nullable: false };
        let other = |n: &str| StaticType::Other {
            name: n.to_string(),
            nullable: false,
        };
        vec![
            // ── void: collection mutation ──
            ("addAll", void()),
            ("insert", void()),
            ("insertAll", void()),
            ("removeWhere", void()),
            ("retainWhere", void()),
            ("removeRange", void()),
            ("clear", void()),
            ("sort", void()),
            ("shuffle", void()),
            ("fillRange", void()),
            ("setAll", void()),
            ("setRange", void()),
            ("forEach", void()),
            ("addEntries", void()),
            ("updateAll", void()),
            // ── void: listeners / notifiers / streams / sinks ──
            ("addListener", void()),
            ("removeListener", void()),
            ("notifyListeners", void()),
            ("write", void()),
            ("writeln", void()),
            ("writeAll", void()),
            ("writeCharCode", void()),
            ("complete", void()),
            ("completeError", void()),
            ("addError", void()),
            // ── void: lifecycle / framework ──
            ("setState", void()),
            ("initState", void()),
            ("dispose", void()),
            ("didChangeDependencies", void()),
            ("didUpdateWidget", void()),
            ("deactivate", void()),
            ("markNeedsBuild", void()),
            ("markNeedsLayout", void()),
            ("markNeedsPaint", void()),
            // ── void: logging ──
            ("print", void()),
            // ── non-void: String ──
            ("toUpperCase", string()),
            ("toLowerCase", string()),
            ("trim", string()),
            ("trimLeft", string()),
            ("trimRight", string()),
            ("substring", string()),
            ("replaceAll", string()),
            ("toString", string()),
            ("padLeft", string()),
            ("padRight", string()),
            // ── non-void: int ──
            ("compareTo", int()),
            ("indexOf", int()),
            // ── non-void: bool ──
            ("contains", boolean()),
            ("startsWith", boolean()),
            ("endsWith", boolean()),
            ("containsKey", boolean()),
            ("containsValue", boolean()),
            // ── non-void: collections / futures ──
            ("map", other("Iterable")),
            ("where", other("Iterable")),
            ("expand", other("Iterable")),
            ("toList", other("List")),
            ("toSet", other("Set")),
            ("then", other("Future")),
            ("whenComplete", other("Future")),
            ("catchError", other("Future")),
        ]
    })
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

    #[test]
    fn function_return_types() {
        let p = program("void doThing() {} int compute() => 1; g() {}");
        let index = ProjectIndex::from_program(&p);
        assert_eq!(index.return_type("doThing"), StaticType::Void);
        assert_eq!(
            index.return_type("compute"),
            StaticType::Int { nullable: false }
        );
        // Omitted return type → Unknown (Dart would infer it; we do not).
        assert_eq!(index.return_type("g"), StaticType::Unknown);
    }

    #[test]
    fn methods_getters_setters() {
        let p = program(
            "class C { void m() {} String get name => 'x'; set value(int v) {} bool ok() => true; }",
        );
        let index = ProjectIndex::from_program(&p);
        assert_eq!(index.return_type("m"), StaticType::Void);
        assert_eq!(
            index.return_type("name"),
            StaticType::String { nullable: false }
        );
        // Setters are always void.
        assert_eq!(index.return_type("value"), StaticType::Void);
        assert_eq!(
            index.return_type("ok"),
            StaticType::Bool { nullable: false }
        );
    }

    #[test]
    fn absent_name_is_unknown() {
        let index = ProjectIndex::new();
        assert_eq!(index.return_type("nope"), StaticType::Unknown);
    }

    #[test]
    fn conflicting_user_decls_are_ambiguous() {
        let p = program("int foo() => 1; String foo() => 'x';");
        let index = ProjectIndex::from_program(&p);
        assert_eq!(index.return_type("foo"), StaticType::Unknown);
    }

    #[test]
    fn agreeing_user_decls_stay_known() {
        let p = program("class A { int size() => 1; } class B { int size() => 2; }");
        let index = ProjectIndex::from_program(&p);
        assert_eq!(
            index.return_type("size"),
            StaticType::Int { nullable: false }
        );
    }

    #[test]
    fn multi_file_index() {
        let a = program("void log(String s) {}");
        let b = program("int add(int x, int y) => x + y;");
        let sources = [
            ProgramSource {
                program: &a,
                has_parse_errors: false,
            },
            ProgramSource {
                program: &b,
                has_parse_errors: false,
            },
        ];
        let index = ProjectIndex::from_project_files(&sources);
        assert_eq!(index.return_type("log"), StaticType::Void);
        assert_eq!(
            index.return_type("add"),
            StaticType::Int { nullable: false }
        );
    }

    #[test]
    fn parse_error_files_are_skipped() {
        let good = program("int good() => 1;");
        // A deliberately broken source; we only assert its decls are not indexed.
        let (bad, _errs) = parse("int bad( { ");
        let sources = [
            ProgramSource {
                program: &good,
                has_parse_errors: false,
            },
            ProgramSource {
                program: &bad,
                has_parse_errors: true,
            },
        ];
        let index = ProjectIndex::from_project_files(&sources);
        assert_eq!(
            index.return_type("good"),
            StaticType::Int { nullable: false }
        );
        assert_eq!(index.return_type("bad"), StaticType::Unknown);
    }

    #[test]
    fn builtins_present_and_dominant_returns() {
        let index = ProjectIndex::with_builtins();
        assert_eq!(index.return_type("addAll"), StaticType::Void);
        assert_eq!(index.return_type("notifyListeners"), StaticType::Void);
        assert_eq!(
            index.return_type("toUpperCase"),
            StaticType::String { nullable: false }
        );
        assert_eq!(
            index.return_type("contains"),
            StaticType::Bool { nullable: false }
        );
        assert_eq!(
            index.return_type("map"),
            StaticType::Other {
                name: "Iterable".into(),
                nullable: false
            }
        );
        // Deliberately-omitted ambiguous name.
        assert_eq!(index.return_type("remove"), StaticType::Unknown);
    }

    #[test]
    fn user_decl_overrides_builtin() {
        // A project-defined `contains` returning int shadows the builtin bool.
        let p = program("int contains(x) => 0;");
        let index = ProjectIndex::from_program(&p);
        assert_eq!(
            index.return_type("contains"),
            StaticType::Int { nullable: false }
        );
    }
}
