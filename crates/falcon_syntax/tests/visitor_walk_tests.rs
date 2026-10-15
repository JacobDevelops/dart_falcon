//! Positive regression coverage for the visitor traversal gaps fixed in
//! `visitor.rs`. Each test plants a unique sentinel (an int literal, a named
//! type, an identifier, or a string) in a position that the default walk chain
//! only reaches through a newly-added walk edge, then asserts the recording
//! visitor observed it exactly once. Missing and duplicated walk edges both
//! fail the corresponding sentinel assertion.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::*;

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

impl Visitor for Recorder {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::IntLit { value, .. } = node {
            self.ints.push(value.clone());
        }
        walk_expr(self, node);
    }

    fn visit_string_lit(&mut self, node: &StringLitNode) {
        self.strings.push(node.value.clone());
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        match node {
            DartType::Named(n) => {
                if let Some(seg) = n.segments.last() {
                    self.named_types.push(seg.name.clone());
                }
            }
            DartType::Dynamic { .. } => self.dynamic_types += 1,
            _ => {}
        }
        walk_dart_type(self, node);
    }

    fn visit_identifier(&mut self, node: &Identifier) {
        self.idents.push(node.name.clone());
    }

    fn visit_annotation(&mut self, node: &Annotation) {
        self.annotations += 1;
        walk_annotation(self, node);
    }
}

fn record(src: &str) -> Recorder {
    let falcon_dart_parser::parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(src);
    assert!(
        errors.is_empty(),
        "unexpected parse errors in {src:?}: {errors:?}"
    );
    let mut r = Recorder::default();
    r.visit_program(&prog);
    r
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
fn local_var_hook_borrows_statement_and_loop_declarations() {
    #[derive(Default)]
    struct Locals {
        declarations: Vec<*const LocalVarDecl>,
    }

    impl Visitor for Locals {
        fn visit_local_var(&mut self, declaration: &LocalVarDecl) {
            self.declarations.push(std::ptr::from_ref(declaration));
        }
    }

    let falcon_dart_parser::parser::ParseResult {
        program, errors, ..
    } = parse(
        "void f() { var statement = 1; \
     for (var counter = 0; counter < 1; counter++) {} \
     var collection = [for (var item = 0; item < 1; item++) item]; }",
    );
    assert!(errors.is_empty(), "{errors:?}");
    let TopLevelDecl::Function(function) = &program.declarations[0] else {
        panic!("expected function");
    };
    let Some(FunctionBody::Block(body)) = &function.body else {
        panic!("expected block");
    };
    let Stmt::LocalVar(statement) = &body.stmts[0] else {
        panic!("expected local declaration");
    };
    let Stmt::For(loop_statement) = &body.stmts[1] else {
        panic!("expected statement loop");
    };
    let Some(ForInit::VarDecl(counter)) = &loop_statement.header.init else {
        panic!("expected loop declaration");
    };
    let Stmt::LocalVar(collection) = &body.stmts[2] else {
        panic!("expected collection declaration");
    };
    let Some(Expr::List { elements, .. }) = &collection.declarators[0].initializer else {
        panic!("expected list");
    };
    let CollectionElement::For { header, .. } = &elements[0] else {
        panic!("expected collection loop");
    };
    let Some(ForInit::VarDecl(item)) = &header.init else {
        panic!("expected collection loop declaration");
    };

    let mut visitor = Locals::default();
    visitor.visit_stmt(&body.stmts[0]);
    visitor.visit_stmt(&body.stmts[1]);
    visitor.visit_expr(collection.declarators[0].initializer.as_ref().unwrap());
    assert_eq!(
        visitor.declarations,
        [
            std::ptr::from_ref(statement),
            std::ptr::from_ref(counter),
            std::ptr::from_ref(item),
        ],
    );
}

// ── Item 1: constructor initializers ──────────────────────────────────────────

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

// ── Item 2: annotations delivered uniformly ───────────────────────────────────

#[test]
fn walks_annotations_on_every_carrier() {
    // Field, getter, setter, top-level var, extension, extension type, formal
    // param, and enum variant annotations were previously never delivered.
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

// ── Item 3: enum variant args + type args ─────────────────────────────────────

#[test]
fn walks_enum_variant_args_and_type_args() {
    let r = record("enum E { a<VarTArg>(90020); const E(int x); }");
    assert!(r.has_int("90020"), "enum variant arg not walked");
    assert!(r.has_named("VarTArg"), "enum variant type-arg not walked");
}

// ── Item 4: local function params (incl. default-value exprs) ──────────────────

#[test]
fn walks_local_function_param_defaults_and_types() {
    let r = record("void outer() { void g([LocalPT x = 90030]) {} }");
    assert!(r.has_int("90030"), "local-func param default not walked");
    assert!(r.has_named("LocalPT"), "local-func param type not walked");
}

// ── Item 5: directives ────────────────────────────────────────────────────────

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

// ── Item 6: type-parameter bounds ─────────────────────────────────────────────

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

// ── Item 7: mixin-class supertype clauses ─────────────────────────────────────

#[test]
fn walks_mixin_class_supertypes() {
    let r = record("mixin class MC extends McBase with McMix implements McIface {}\n");
    assert!(r.has_named("McBase"), "mixin-class extends not walked");
    assert!(r.has_named("McMix"), "mixin-class with-clause not walked");
    assert!(r.has_named("McIface"), "mixin-class implements not walked");
}

// ── Item 8: extension-type representation + implements ─────────────────────────

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

// ── Item 9: call / cascade-call type arguments ────────────────────────────────

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

// ── Item 10: string literal in a literal pattern ──────────────────────────────

#[test]
fn walks_literal_pattern_string() {
    let r = record("void f(Object x) { switch (x) { case 'patstr90050': break; } }");
    assert!(
        r.has_string("patstr90050"),
        "literal-pattern string not walked"
    );
}

// ── Extra gaps closed for consistency: operator params, function-typed params ──

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

// ── Item 11 (parser span fix): adjacent-string merged span covers the full run ─

#[test]
fn adjacent_string_span_covers_full_run() {
    let src = r"var x = 'a\q' 'b\w';";
    let falcon_dart_parser::parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(src);
    assert!(errors.is_empty(), "parse errors: {errors:?}");

    let TopLevelDecl::Variable(v) = &prog.declarations[0] else {
        panic!("expected top-level var");
    };
    let Some(Expr::StringLit(lit)) = &v.declarators[0].initializer else {
        panic!("expected string literal initializer");
    };

    // The merged span must run from the first fragment's opening quote to the
    // last fragment's closing quote — i.e. cover the entire adjacent run.
    let full_run = r"'a\q' 'b\w'";
    let expected_start = src.find(full_run).unwrap();
    let expected_end = expected_start + full_run.len();
    assert_eq!(
        lit.span.start, expected_start,
        "span.start must be the first fragment"
    );
    assert_eq!(
        lit.span.end, expected_end,
        "span.end must be the last fragment"
    );
    assert_eq!(&src[lit.span.start..lit.span.end], full_run);
}

#[derive(Default)]
struct CountingVisitor {
    class_count: usize,
    function_count: usize,
    field_count: usize,
}

impl Visitor for CountingVisitor {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        self.class_count += 1;
        walk_class_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function_count += 1;
        walk_function_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.field_count += 1;
        walk_field_decl(self, node);
    }
}

#[test]
fn test_visitor_empty_program() {
    let src = "";
    let falcon_dart_parser::parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor::default();
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 0);
    assert_eq!(visitor.function_count, 0);
}

#[test]
fn test_visitor_single_class() {
    let src = "class Foo {}";
    let falcon_dart_parser::parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor::default();
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 1);
}

#[test]
fn test_visitor_class_with_fields() {
    let src = "class Foo { int x; String y; }";
    let falcon_dart_parser::parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor::default();
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 1);
    assert_eq!(visitor.field_count, 2);
}

#[test]
fn test_visitor_multiple_declarations() {
    let src = "class Foo {} void bar() {} class Baz {}";
    let falcon_dart_parser::parser::ParseResult {
        program: prog,
        errors,
        ..
    } = parse(src);
    assert!(errors.is_empty());

    let mut visitor = CountingVisitor::default();
    visitor.visit_program(&prog);

    assert_eq!(visitor.class_count, 2);
    assert_eq!(visitor.function_count, 1);
}

#[test]
fn program_callbacks_preserve_traversal_order_and_coverage() {
    let falcon_dart_parser::parser::ParseResult {
        program, errors, ..
    } = parse(
        "@tag(1 + 2) int top = 3; \
     class C { \
       int field = 4; \
       C([int value = 5]) : field = 6; \
       Object f([int value = 7]) { \
         for (var i = 8; i < 9; i += 10) { (() { return 11; })(); } \
         return [for (var j = 12; j < 13; j += 14) 15, \
                 for (final x in [16]) 17]; \
       } \
     }",
    );
    assert!(errors.is_empty(), "{errors:?}");

    let mut expressions = Vec::new();
    for_each_expr_in_program(&program, |expr| match expr {
        Expr::Binary {
            op: BinaryOp::Add, ..
        } => expressions.push("+".to_owned()),
        Expr::IntLit { value, .. } => expressions.push(value.clone()),
        _ => {}
    });
    assert_eq!(
        expressions,
        [
            "+", "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
            "16", "17",
        ]
    );

    let mut statements = Vec::new();
    for_each_stmt_in_program(&program, |stmt| {
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

    impl Visitor for Names {
        fn visit_identifier(&mut self, node: &Identifier) {
            self.semantic.push(node.name.clone());
        }

        fn visit_name(&mut self, node: &Identifier) {
            self.source.push(node.name.clone());
        }
    }

    let result = parse("void f() { target.field; }");
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    let mut names = Names::default();
    names.visit_program(&result.program);
    assert_eq!(names.semantic, ["f", "target"]);
    assert_eq!(names.source, ["field"]);
}

#[test]
fn non_expression_strings_walk_interpolations_once() {
    let parsed = parse("var text = '${90061}';");
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let TopLevelDecl::Variable(variable) = &parsed.program.declarations[0] else {
        panic!("expected variable");
    };
    let Some(Expr::StringLit(literal)) = &variable.declarators[0].initializer else {
        panic!("expected interpolated string");
    };

    let mut pattern = Recorder::default();
    pattern.visit_literal_pattern_value(&LiteralPatternValue::String(literal.clone()));
    assert!(pattern.has_int("90061"));

    let mut native = Recorder::default();
    native.visit_function_body(&FunctionBody::Native(Some(literal.clone()), literal.span));
    assert!(native.has_int("90061"));
}

#[test]
fn shorthand_pattern_labels_are_not_duplicate_source_names() {
    #[derive(Default)]
    struct Names {
        bindings: usize,
        labels: usize,
    }

    impl Visitor for Names {
        fn visit_identifier(&mut self, node: &Identifier) {
            if matches!(node.name.as_str(), "first" | "second") {
                self.bindings += 1;
            }
        }

        fn visit_name(&mut self, node: &Identifier) {
            if matches!(node.name.as_str(), "first" | "second") {
                self.labels += 1;
            }
        }
    }

    let parsed = parse(
        "void f(Object value) { \
         if (value case (:var first)) {} \
         if (value case C(:var second)) {} \
         }",
    );
    assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
    let mut names = Names::default();
    names.visit_program(&parsed.program);
    assert_eq!(names.bindings, 2);
    assert_eq!(names.labels, 0);
}
