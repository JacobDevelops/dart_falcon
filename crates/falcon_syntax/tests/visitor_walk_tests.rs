//! Positive regression coverage for the visitor traversal gaps fixed in
//! `visitor.rs`. Each test plants a unique sentinel (an int literal, a named
//! type, an identifier, or a string) in a position that the default walk chain
//! only reaches through a newly-added walk edge, then asserts the recording
//! visitor observed it. If any of these walk edges is removed, the corresponding
//! sentinel stops being visited and the test fails.

use falcon_dart_parser::parser::parse;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::*;

/// Records every leaf the default walk delivers, keyed by kind, so a sentinel's
/// presence proves the traversal reached the position that carries it.
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
        self.ints.iter().any(|s| s == v)
    }
    fn has_string(&self, v: &str) -> bool {
        self.strings.iter().any(|s| s == v)
    }
    fn has_named(&self, v: &str) -> bool {
        self.named_types.iter().any(|s| s == v)
    }
    fn has_ident(&self, v: &str) -> bool {
        self.idents.iter().any(|s| s == v)
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
    let (prog, errors) = parse(src);
    assert!(
        errors.is_empty(),
        "unexpected parse errors in {src:?}: {errors:?}"
    );
    let mut r = Recorder::default();
    r.visit_program(&prog);
    r
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
    let (prog, errors) = parse(src);
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
