//! Regression coverage for traversal boundaries and source-child ordering.

use falcon_dart_parser::parser::parse;
use falcon_syntax::cst::visit::*;
use falcon_syntax::cst::*;

/// Records every leaf the default walk delivers, keyed by kind, so a sentinel's
/// exact-once presence proves traversal reached its position without duplication.
#[derive(Default)]
struct Recorder {
    ints: Vec<String>,
    strings: Vec<String>,
    named_types: Vec<String>,
    idents: Vec<String>,
    dynamic_types: usize,
    annotations: usize,
}

impl Recorder {
    fn has_int(&self, v: &str) -> bool {
        self.ints.iter().filter(|s| s.as_str() == v).count() == 1
    }
    fn has_string(&self, v: &str) -> bool {
        self.strings.iter().filter(|s| s.as_str() == v).count() == 1
    }
    fn has_named(&self, v: &str) -> bool {
        self.named_types.iter().filter(|s| s.as_str() == v).count() == 1
    }
    fn has_ident(&self, v: &str) -> bool {
        self.idents.iter().filter(|s| s.as_str() == v).count() == 1
    }
}

impl<'a> Visitor<'a> for Recorder {
    fn visit_expr(&mut self, node: Expr<'a>) {
        if let Expr::IntLit(value) = node {
            self.ints.push(value.value().to_owned());
        }
        walk_expr(self, node);
    }

    fn visit_string_lit(&mut self, node: StringLitNode<'a>) {
        self.strings.push(node.value().into_owned());
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) {
        match node {
            DartType::Named(n) => {
                if let Some(seg) = n.segments().last() {
                    self.named_types.push(seg.name().to_owned());
                }
            }
            DartType::Dynamic(_) => self.dynamic_types += 1,
            _ => {}
        }
        walk_dart_type(self, node);
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) {
        self.idents.push(node.name().to_owned());
    }

    fn visit_annotation(&mut self, node: Annotation<'a>) {
        self.annotations += 1;
        walk_annotation(self, node);
    }
}

fn record(src: &str) -> Recorder {
    let parsed = parse(src);
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parsed;
    assert!(
        errors.is_empty(),
        "unexpected parse errors in {src:?}: {errors:?}"
    );
    let mut r = Recorder::default();
    r.visit_program(tree.program(src));
    r
}

#[test]
fn walks_function_type_parameter_annotations() {
    let r = record("typedef F = void Function(@sentinel(713) int value);");
    assert_eq!(r.annotations, 1);
    assert!(r.has_ident("sentinel"));
    assert!(r.has_int("713"));
}

#[test]
fn walks_catch_types_and_bindings() {
    let r = record("void f() { try {} on CatchType catch (caught, trace) {} }");
    assert!(r.has_named("CatchType"));
    assert!(r.has_ident("caught"));
    assert!(r.has_ident("trace"));
}

#[test]
fn walks_collection_literal_type_arguments() {
    let r = record(
        "void f() { var a = <ListArg>[]; var b = <SetArg>{value}; \
         var c = <MapKey, MapValue>{key: value}; }",
    );
    for name in ["ListArg", "SetArg", "MapKey", "MapValue"] {
        assert!(
            r.has_named(name),
            "collection type argument {name} not walked"
        );
    }
}

#[test]
fn walks_pattern_type_arguments_and_constant_name_segments() {
    let r = record(
        "void f(Object value) { switch (value) { \
         case <PatternList>[a]: break; \
         case <PatternKey, PatternValue>{'key': b}: break; \
         case PatternOwner.member: break; } }",
    );
    for name in ["PatternList", "PatternKey", "PatternValue"] {
        assert!(r.has_named(name), "pattern type argument {name} not walked");
    }
    assert!(r.has_ident("PatternOwner"));
    assert!(r.has_ident("member"));
}

#[test]
fn walks_collection_for_in_header_types_and_bindings() {
    let r = record(
        "void f() { var a = [for (ListItem listItem in source) listItem]; \
         var b = {for (SetItem setItem in source) setItem}; \
         var c = {for (MapItem mapItem in source) mapItem: value}; }",
    );
    for name in ["ListItem", "SetItem", "MapItem"] {
        assert!(r.has_named(name), "for-in header type {name} not walked");
    }
    for name in ["listItem", "setItem", "mapItem"] {
        assert_eq!(
            r.idents
                .iter()
                .filter(|ident| ident.as_str() == name)
                .count(),
            2,
            "for-in binding and body use of {name} must both be walked",
        );
    }
}

#[test]
fn local_var_hook_observes_statement_and_loop_declarations() {
    #[derive(Default)]
    struct Locals {
        declarations: Vec<NodeId>,
    }

    impl<'a> Visitor<'a> for Locals {
        fn visit_local_var(&mut self, declaration: LocalVarDecl<'a>) {
            self.declarations.push(declaration.syntax().id());
        }
    }

    let src = "void f() { var statement = 1; \
     for (var counter = 0; counter < 1; counter++) {} \
     var collection = [for (var item = 0; item < 1; item++) item]; }";
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parse(src);
    let program = tree.program(src);
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
        panic!("expected function");
    };
    let Some(FunctionBody::Block(body)) = function.body() else {
        panic!("expected block");
    };
    let Stmt::LocalVar(statement) = body.stmts().next().unwrap() else {
        panic!("expected local declaration");
    };
    let Stmt::For(loop_statement) = body.stmts().nth(1).unwrap() else {
        panic!("expected statement loop");
    };
    let Some(ForInit::VarDecl(counter)) = loop_statement.header().init() else {
        panic!("expected loop declaration");
    };
    let Stmt::LocalVar(collection) = body.stmts().nth(2).unwrap() else {
        panic!("expected collection declaration");
    };
    let Some(Expr::List(list)) = collection.declarators().next().unwrap().initializer() else {
        panic!("expected list");
    };
    let CollectionElement::For(element) = list.elements().next().unwrap() else {
        panic!("expected collection loop");
    };
    let Some(ForInit::VarDecl(item)) = element.header().init() else {
        panic!("expected collection loop declaration");
    };

    let mut visitor = Locals::default();
    visitor.visit_stmt(body.stmts().next().unwrap());
    visitor.visit_stmt(body.stmts().nth(1).unwrap());
    visitor.visit_expr(
        collection
            .declarators()
            .next()
            .unwrap()
            .initializer()
            .unwrap(),
    );
    assert_eq!(
        visitor.declarations,
        [
            statement.syntax().id(),
            counter.syntax().id(),
            item.syntax().id(),
        ],
    );
}

#[test]
fn walks_constructor_field_initializer() {
    let r = record("class C { final int a; C() : a = 90001; }");
    assert!(r.has_int("90001"), "field-init value not walked");
}

#[test]
fn walks_constructor_super_call_args() {
    let r = record("class B { B([Object? o]); } class C extends B { C() : super(90002); }");
    assert!(r.has_int("90002"), "super-call arg not walked");
}

#[test]
fn walks_constructor_this_call_args() {
    let r = record("class C { final int a; C() : this._(90003); C._(this.a); }");
    assert!(r.has_int("90003"), "this-call arg not walked");
}

#[test]
fn walks_constructor_assert_condition_and_message() {
    let r = record("class C { C() : assert(90004 > 0, 'msg90005'); }");
    assert!(r.has_int("90004"), "assert condition not walked");
    assert!(r.has_string("msg90005"), "assert message not walked");
}

#[test]
fn walks_annotations_on_every_carrier() {
    let r = record(
        "@A(90010) int tlv = 0;\n\
         @A(90011) extension Ext on int {}\n\
         @A(90012) extension type ET(int v) {}\n\
         class C {\n\
           @A(90013) int f = 0;\n\
           @A(90014) int get g => 0;\n\
           @A(90015) set s(int v) {}\n\
           void m(@A(90016) int p) {}\n\
         }\n\
         enum E { @A(90017) a; }\n",
    );
    for sentinel in [
        "90010", "90011", "90012", "90013", "90014", "90015", "90016", "90017",
    ] {
        assert!(
            r.has_int(sentinel),
            "annotation arg {sentinel} not delivered/walked"
        );
    }
}

#[test]
fn walks_enum_variant_args_and_type_args() {
    let r = record("enum E { a<VarTArg>(90020); const E(int x); }");
    assert!(r.has_int("90020"), "enum variant arg not walked");
    assert!(r.has_named("VarTArg"), "enum variant type-arg not walked");
}

#[test]
fn walks_local_function_param_defaults_and_types() {
    let r = record("void outer() { void g([LocalPT x = 90030]) {} }");
    assert!(r.has_int("90030"), "local-func param default not walked");
    assert!(r.has_named("LocalPT"), "local-func param type not walked");
}

#[test]
fn walks_library_import_export_part_directives() {
    let r = record(
        "library dirlib90040;\n\
         import 'i90041.dart' show Imp90042;\n\
         export 'e90043.dart' show Exp90044;\n\
         part 'p90045.dart';\n",
    );
    assert!(r.has_ident("dirlib90040"), "library name not walked");
    assert!(r.has_string("i90041.dart"), "import uri not walked");
    assert!(r.has_ident("Imp90042"), "import show combinator not walked");
    assert!(r.has_string("e90043.dart"), "export uri not walked");
    assert!(r.has_ident("Exp90044"), "export show combinator not walked");
    assert!(r.has_string("p90045.dart"), "part uri not walked");
}

#[test]
fn walks_part_of_directive() {
    let r = record("part of 'parent90046.dart';\n");
    assert!(r.has_string("parent90046.dart"), "part-of uri not walked");
}

#[test]
fn walks_type_param_bounds_across_declarations() {
    let r = record(
        "class C<T extends CBound> { void m<U extends MBound>() {} }\n\
         void f<T extends FBound>() {}\n\
         typedef Cb<T extends TBound> = void Function();\n\
         mixin Mx<T extends MxBound> {}\n",
    );
    for bound in ["CBound", "MBound", "FBound", "TBound", "MxBound"] {
        assert!(r.has_named(bound), "type-param bound {bound} not walked");
    }
}

#[test]
fn walks_function_expression_type_param_bound() {
    let r = record("var fe = <T extends FeBound>(T x) => x;");
    assert!(
        r.has_named("FeBound"),
        "func-expr type-param bound not walked"
    );
}

#[test]
fn walks_function_typed_formal_type_param_bound() {
    let r = record("void f(int cb<T extends FormalBound>(T x)) {}");
    assert!(
        r.has_named("FormalBound"),
        "function-typed formal type-param bound not walked"
    );
}

#[test]
fn walks_mixin_class_supertypes() {
    let r = record("mixin class MC extends McBase with McMix implements McIface {}\n");
    assert!(r.has_named("McBase"), "mixin-class extends not walked");
    assert!(r.has_named("McMix"), "mixin-class with-clause not walked");
    assert!(r.has_named("McIface"), "mixin-class implements not walked");
}

#[test]
fn walks_extension_type_representation_and_implements() {
    let r = record("extension type ET(EtRepr value) implements EtIface {}");
    assert!(
        r.has_named("EtRepr"),
        "extension-type representation type not walked"
    );
    assert!(
        r.has_named("EtIface"),
        "extension-type implements not walked"
    );
}

#[test]
fn walks_call_type_args() {
    let r = record("void f() { gen<CallTArg>(); } void gen<T>() {}");
    assert!(r.has_named("CallTArg"), "call type-arg not walked");
}

#[test]
fn walks_cascade_call_type_args() {
    let r = record("void f(dynamic x) { x..m<CascTArg>(); }");
    assert!(r.has_named("CascTArg"), "cascade-call type-arg not walked");
}

#[test]
fn walks_literal_pattern_string() {
    let r = record("void f(Object x) { switch (x) { case 'patstr90050': break; } }");
    assert!(
        r.has_string("patstr90050"),
        "literal-pattern string not walked"
    );
}

#[test]
fn walks_operator_params() {
    let r = record("class C { C operator +(OpParam other) => this; }");
    assert!(r.has_named("OpParam"), "operator param type not walked");
}

#[test]
fn walks_function_typed_param_inner_params() {
    let r = record("void f(void cb(FnParam x)) {}");
    assert!(
        r.has_named("FnParam"),
        "function-typed param inner param not walked"
    );
}

#[test]
fn adjacent_string_span_covers_full_run() {
    let src = r"var x = 'a\q' 'b\w';";
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parse(src);
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let prog = tree.program(src);

    let TopLevelDecl::Variable(v) = prog.declarations().next().unwrap() else {
        panic!("expected top-level var");
    };
    let Some(Expr::StringLit(lit)) = v.declarators().next().unwrap().initializer() else {
        panic!("expected string literal initializer");
    };

    let full_run = r"'a\q' 'b\w'";
    let expected_start = src.find(full_run).unwrap();
    let expected_end = expected_start + full_run.len();
    assert_eq!(
        lit.span().start,
        expected_start,
        "span.start must be the first fragment"
    );
    assert_eq!(
        lit.span().end,
        expected_end,
        "span.end must be the last fragment"
    );
    assert_eq!(&src[lit.span().start..lit.span().end], full_run);
}

#[test]
fn program_callbacks_preserve_traversal_order_and_coverage() {
    let src = "@tag(1 + 2) int top = 3; \
     class C { \
       int field = 4; \
       C([int value = 5]) : field = 6; \
       Object f([int value = 7]) { \
         for (var i = 8; i < 9; i += 10) { (() { return 11; })(); } \
         return [for (var j = 12; j < 13; j += 14) 15, \
                 for (final x in [16]) 17]; \
       } \
     }";
    let falcon_dart_parser::parser::ParseResult { tree, errors } = parse(src);
    let program = tree.program(src);
    assert!(errors.is_empty(), "{errors:?}");

    struct Expressions(Vec<String>);
    impl<'a> Visitor<'a> for Expressions {
        fn visit_expr(&mut self, expr: Expr<'a>) {
            match expr {
                Expr::Binary(binary) if binary.op() == BinaryOp::Add => self.0.push("+".to_owned()),
                Expr::IntLit(value) => self.0.push(value.value().to_owned()),
                _ => {}
            }
            walk_expr(self, expr);
        }
    }
    let mut expressions = Expressions(Vec::new());
    expressions.visit_program(program);
    assert_eq!(
        expressions.0,
        [
            "+", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
            "16", "17",
        ]
    );

    let mut statements = Vec::new();
    for_each_stmt_in_program(program, |stmt| {
        statements.push(match stmt {
            Stmt::For(_) => "for",
            Stmt::Block(_) => "block",
            Stmt::Expr(_) => "expr",
            Stmt::Return(_) => "return",
            _ => "other",
        });
    });
    assert_eq!(statements, ["for", "block", "expr", "return", "return"]);
}

#[test]
fn string_observation_override_does_not_suppress_interpolation_children() {
    // Recorder deliberately overrides visit_string_lit without recursing.
    let recorder = record("void f() { var text = '${90060}'; }");
    assert!(recorder.has_int("90060"));
    assert_eq!(recorder.strings.len(), 1);
}

#[test]
fn source_name_observation_preserves_semantic_identifier_boundary() {
    #[derive(Default)]
    struct Names {
        semantic: Vec<String>,
        source: Vec<String>,
    }

    impl<'a> Visitor<'a> for Names {
        fn visit_identifier(&mut self, node: Identifier<'a>) {
            self.semantic.push(node.name().to_owned());
        }

        fn visit_name(&mut self, node: Identifier<'a>) {
            self.source.push(node.name().to_owned());
        }
    }

    let src = "void f() { target.field; }";
    let result = parse(src);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let mut names = Names::default();
    names.visit_program(result.tree.program(src));
    assert_eq!(names.semantic, ["f", "target"]);
    assert_eq!(names.source, ["field"]);
}

#[test]
fn non_expression_strings_walk_interpolations_once() {
    let src = "var text = '${90061}';";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::Variable(variable) = parsed.tree.program(src).declarations().next().unwrap()
    else {
        panic!("expected variable");
    };
    let Some(Expr::StringLit(literal)) = variable.declarators().next().unwrap().initializer()
    else {
        panic!("expected interpolated string");
    };

    let mut pattern = Recorder::default();
    pattern.visit_literal_pattern_value(LiteralPatternValue::String(literal));
    assert!(pattern.has_int("90061"));

    let src = "void f() native '${90061}';";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::Function(function) = parsed.tree.program(src).declarations().next().unwrap()
    else {
        panic!("expected function");
    };
    let body = function.body().expect("expected native body");
    assert!(matches!(body, FunctionBody::Native(_)));
    let mut native = Recorder::default();
    native.visit_function_body(body);
    assert!(native.has_int("90061"));
}

#[test]
fn shorthand_pattern_labels_are_not_duplicate_source_names() {
    #[derive(Default)]
    struct Names {
        bindings: usize,
        labels: usize,
    }

    impl<'a> Visitor<'a> for Names {
        fn visit_identifier(&mut self, node: Identifier<'a>) {
            if matches!(node.name(), "first" | "second") {
                self.bindings += 1;
            }
        }

        fn visit_name(&mut self, node: Identifier<'a>) {
            if matches!(node.name(), "first" | "second") {
                self.labels += 1;
            }
        }
    }

    let src = "void f(Object value) { \
         if (value case (:var first)) {} \
         if (value case C(:var second)) {} \
         }";
    let parsed = parse(src);
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut names = Names::default();
    names.visit_program(parsed.tree.program(src));
    assert_eq!(names.bindings, 2);
    assert_eq!(names.labels, 0);
}
