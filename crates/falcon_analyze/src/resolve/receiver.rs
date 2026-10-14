//! Constructor calls parse as [`Expr::Call`], so receiver typing supplements [`LocalTypes`].

use falcon_syntax::ast::Expr;

use super::{LocalTypes, MemberResult, StaticType, TypeIndex};

pub struct ReceiverTypes<'a> {
    pub locals: &'a LocalTypes,
    pub types: Option<&'a TypeIndex>,
    pub enclosing_type: Option<&'a str>,
}

impl<'a> ReceiverTypes<'a> {
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

    fn constructor_type(&self, callee: &Expr) -> Option<StaticType> {
        let types = self.types?;
        match callee {
            Expr::Ident(id) => self.type_call(types, &id.name),
            Expr::GenericInstantiation { target, .. } => match target.as_ref() {
                Expr::Ident(id) => self.type_call(types, &id.name),
                _ => None,
            },
            Expr::Field { object, field, .. } => {
                let Expr::Ident(obj) = object.as_ref() else {
                    return None;
                };
                if self.is_local(&obj.name) {
                    return None;
                }
                if types.is_known_type(&obj.name) {
                    // Static members may return another type; only proven absence permits constructor typing.
                    return match types.member_lookup(&obj.name, &field.name) {
                        MemberResult::ProvenAbsent => Some(other(&obj.name)),
                        MemberResult::Found(_) | MemberResult::Unknown => None,
                    };
                }
                if types.is_known_type(&field.name) {
                    return Some(other(&field.name));
                }
                None
            }
            _ => None,
        }
    }

    fn type_call(&self, types: &TypeIndex, name: &str) -> Option<StaticType> {
        if self.is_local(name) {
            return None;
        }
        types.is_known_type(name).then(|| other(name))
    }

    /// Even an untyped local shadows a same-named type.
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
        // Guessing the constructor type here would suppress diagnostics on the actual return type.
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
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = prefix.Widget(); }")),
            other("Widget")
        );
    }

    #[test]
    fn call_on_local_is_not_a_constructor() {
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
        assert_eq!(
            rt.of_expr(&first_init("void f() { var x = unknownFn(); }")),
            StaticType::Unknown
        );
    }

    #[test]
    fn falls_back_to_local_types_for_non_call_non_this() {
        let locals = LocalTypes::new();
        let rt = ReceiverTypes::new(&locals, None, Some("C"));
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
