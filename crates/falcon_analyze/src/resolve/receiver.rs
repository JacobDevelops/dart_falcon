//! Receiver typing — a thin layer over [`LocalTypes`] that resolves a few more
//! expression shapes when a [`TypeIndex`] and enclosing-type context are on hand.
//!
//! A rule that needs the static type of a *receiver* (`recv.foo()`, `recv.bar`)
//! drives [`ReceiverTypes::of_expr`]. It answers everything [`LocalTypes`] does,
//! and additionally:
//!
//! * `this` → the enclosing type;
//! * a constructor call `X()` / `X<T>()` / `X.named()` / `prefix.X()` where the
//!   head names a known [`TypeIndex`] type → `Other { X }` (project gotcha:
//!   `X()` and `X.from()` parse as `Call` chains, not [`Expr::New`]).
//!
//! Everything else defers to [`LocalTypes::of_expr`], so `Field` / `Index` stay
//! [`StaticType::Unknown`] unless trivially provable, preserving the soundness
//! discipline: a lost fact is `Unknown`, never a guess.

use falcon_syntax::ast::Expr;

use super::{LocalTypes, MemberResult, StaticType, TypeIndex};

/// A receiver-type resolver: local scope + optional project type index + the
/// enclosing type name (for `this`).
pub struct ReceiverTypes<'a> {
    pub locals: &'a LocalTypes,
    pub types: Option<&'a TypeIndex>,
    /// Simple name of the class/mixin/enum/extension-type whose body is being
    /// analyzed, if any — the static type of `this`.
    pub enclosing_type: Option<&'a str>,
}

impl<'a> ReceiverTypes<'a> {
    /// Construct a resolver. `locals` is required; `types` and `enclosing_type`
    /// are optional and only widen what can be resolved.
    pub fn new(
        locals: &'a LocalTypes,
        types: Option<&'a TypeIndex>,
        enclosing_type: Option<&'a str>,
    ) -> Self {
        Self {
            locals,
            types,
            enclosing_type,
        }
    }

    /// Infer the coarse static type of an expression, extending
    /// [`LocalTypes::of_expr`] with `this` and constructor-call typing.
    pub fn of_expr(&self, expr: &Expr) -> StaticType {
        match expr {
            Expr::This { .. } => match self.enclosing_type {
                Some(name) => other(name),
                None => StaticType::Unknown,
            },
            Expr::Call { callee, .. } => self
                .constructor_type(callee)
                .unwrap_or_else(|| self.locals.of_expr(expr)),
            _ => self.locals.of_expr(expr),
        }
    }

    /// If `callee` names a known type (an unnamed, generic, named, or prefixed
    /// constructor), the instance type it produces; otherwise `None` so the
    /// caller falls back to the ordinary (Unknown) call typing.
    ///
    /// A callee `Ident` that is a tracked *local* is never a constructor — it is
    /// a value being invoked — so those short-circuit to `None`.
    fn constructor_type(&self, callee: &Expr) -> Option<StaticType> {
        let types = self.types?;
        match callee {
            // `X()` / `X<T>()` (type args ride on the `Call`, not the callee).
            Expr::Ident(id) => self.type_call(types, &id.name),
            // Bare generic tear-off as callee: `X<T>` in `X<T>()` variants.
            Expr::GenericInstantiation { target, .. } => match target.as_ref() {
                Expr::Ident(id) => self.type_call(types, &id.name),
                _ => None,
            },
            // `X.named()` (X a type) or `prefix.X()` (X a type).
            Expr::Field { object, field, .. } => {
                let Expr::Ident(obj) = object.as_ref() else {
                    return None;
                };
                // `local.method()` is an instance call, never a constructor.
                if self.is_local(&obj.name) {
                    return None;
                }
                if types.is_known_type(&obj.name) {
                    // `X.name()` is a named constructor of `X` *only* when `name`
                    // is not a declared member. A static method/getter/field named
                    // `name` returns some other type we cannot infer, so degrade to
                    // `None` (→ Unknown) rather than guess `X` — the soundness
                    // discipline forbids a guess (a lost fact is Unknown).
                    return match types.member_lookup(&obj.name, &field.name) {
                        MemberResult::ProvenAbsent => Some(other(&obj.name)),
                        MemberResult::Found(_) | MemberResult::Unknown => None,
                    };
                }
                if types.is_known_type(&field.name) {
                    return Some(other(&field.name)); // prefixed constructor
                }
                None
            }
            _ => None,
        }
    }

    /// `Some(Other { name })` when `name` is a known type and not a tracked
    /// local (so `X()` is a constructor, but `f()` on a local `f` is not).
    fn type_call(&self, types: &TypeIndex, name: &str) -> Option<StaticType> {
        if self.is_local(name) {
            return None;
        }
        types.is_known_type(name).then(|| other(name))
    }

    /// Whether `name` is a currently-tracked local/parameter (has a binding in
    /// scope), regardless of whether its type is known — an untyped local still
    /// shadows a same-named type, so `X()` is a value call, not a constructor.
    fn is_local(&self, name: &str) -> bool {
        self.locals.is_bound(name)
    }
}

fn other(name: &str) -> StaticType {
    StaticType::Other {
        name: name.to_string(),
        nullable: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;
    use falcon_syntax::ast::{Stmt, TopLevelDecl};

    /// The initializer expression of the first local-var declaration in `f`.
    fn first_init(src: &str) -> Expr {
        let (program, errors) = parse(src);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        for decl in program.declarations {
            if let TopLevelDecl::Function(f) = decl {
                let body = f.body.expect("body");
                if let falcon_syntax::ast::FunctionBody::Block(b) = body {
                    for s in b.stmts {
                        if let Stmt::LocalVar(lv) = s {
                            return lv.declarators[0].initializer.clone().expect("init");
                        }
                    }
                }
            }
        }
        panic!("no local var initializer in: {src}");
    }

    fn type_index(src: &str) -> TypeIndex {
        let (program, errors) = parse(src);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        TypeIndex::from_program(&program)
    }

    #[test]
    fn this_resolves_to_enclosing_type() {
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, None, Some("Widget"));
        let (program, _) = parse("void f() { var x = this; }");
        // Extract `this` init directly.
        let TopLevelDecl::Function(func) = &program.declarations[0] else {
            panic!()
        };
        let falcon_syntax::ast::FunctionBody::Block(b) = func.body.as_ref().unwrap() else {
            panic!()
        };
        let Stmt::LocalVar(lv) = &b.stmts[0] else {
            panic!()
        };
        let init = lv.declarators[0].initializer.as_ref().unwrap();
        assert_eq!(rt.of_expr(init), other("Widget"));

        // Without enclosing type, `this` is Unknown.
        let rt2 = ReceiverTypes::new(&locals, None, None);
        assert_eq!(rt2.of_expr(init), StaticType::Unknown);
    }

    #[test]
    fn unnamed_and_generic_constructor_calls() {
        let types = type_index("class Foo<T> {} class Bar {}");
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, Some(&types), None);

        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = Bar(); }")),
            other("Bar")
        );
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = Foo<int>(); }")),
            other("Foo")
        );
    }

    #[test]
    fn named_constructor_call() {
        let types = type_index("class Foo { Foo.named(); }");
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = Foo.named(); }")),
            other("Foo")
        );
    }

    #[test]
    fn static_method_call_is_not_a_constructor() {
        // `Config.load()` where `load` is a *static method* returning some other
        // type must not be typed as `Config` — the return type is unknown, and a
        // guess would wrongly suppress diagnostics that fire on the real type.
        let types = type_index("class Config { static Map<String, String> load() => {}; }");
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = Config.load(); }")),
            StaticType::Unknown
        );
    }

    #[test]
    fn prefixed_constructor_call() {
        let types = type_index("class Widget {}");
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        // `prefix.Widget()` — prefix is unknown, field names a known type.
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = prefix.Widget(); }")),
            other("Widget")
        );
    }

    #[test]
    fn call_on_local_is_not_a_constructor() {
        // A tracked local of a callable type invoked as `cb()` is a value call,
        // not a constructor — even though the index has types.
        let types = type_index("class Foo {}");
        let mut locals = LocalTypes::new();
        locals.declare(
            "cb",
            StaticType::Other {
                name: "Function".into(),
                nullable: false,
            },
        );
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = cb(); }")),
            StaticType::Unknown
        );
    }

    #[test]
    fn untyped_local_shadowing_type_is_not_a_constructor() {
        // A local of *unknown* type that shadows a same-named type: `Foo()` is a
        // value call on the local, not a constructor. Distinguishing an absent
        // binding from one whose type is Unknown is the whole point.
        let types = type_index("class Foo {}");
        let mut locals = LocalTypes::new();
        locals.declare("Foo", StaticType::Unknown);
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = Foo(); }")),
            StaticType::Unknown
        );
    }

    #[test]
    fn named_constructor_on_untyped_local_is_not_a_constructor() {
        // `Foo.named()` where `Foo` is an untyped local: an instance member call,
        // not the named constructor `Foo.named`.
        let types = type_index("class Foo { Foo.named(); }");
        let mut locals = LocalTypes::new();
        locals.declare("Foo", StaticType::Unknown);
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = Foo.named(); }")),
            StaticType::Unknown
        );
    }

    #[test]
    fn instance_method_on_local_is_not_constructor() {
        let types = type_index("class Foo { int bar() => 1; }");
        let mut locals = LocalTypes::new();
        locals.declare("foo", other("Foo"));
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        // `foo.bar()` — foo is a local, so this is an instance call, not `Foo.bar` ctor.
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = foo.bar(); }")),
            StaticType::Unknown
        );
    }

    #[test]
    fn unknown_callee_defers_to_locals() {
        let types = type_index("class Foo {}");
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, Some(&types), None);
        // `unknownFn()` — not a type, not a local → Unknown.
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = unknownFn(); }")),
            StaticType::Unknown
        );
    }

    #[test]
    fn falls_back_to_local_types_for_non_call_non_this() {
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, None, Some("C"));
        // Literals and casts route through LocalTypes unchanged.
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = 42; }")),
            StaticType::Int { nullable: false }
        );
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = y as bool; }")),
            StaticType::Bool { nullable: false }
        );
    }

    #[test]
    fn no_type_index_means_no_constructor_typing() {
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, None, None);
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = Foo(); }")),
            StaticType::Unknown
        );
    }
}
