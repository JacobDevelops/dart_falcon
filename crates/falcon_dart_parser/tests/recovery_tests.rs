use falcon_dart_parser::parse;
use falcon_syntax::cst::visit::Visitor;

struct Walk;
impl<'a> Visitor<'a> for Walk {}

#[test]
fn debug_and_visitor_walk_recovery_shapes_without_panicking() {
    for source in [
        "\u{feff}",
        "\u{feff}class C {}",
        "part of package.library;",
        "import 'a.dart' if (dart.library.io == 'true') 'b.dart' as p hide A; export 'c.dart' hide C;",
        "typedef Legacy<T>(T value); typedef F = (int first, {String second}) Function<T extends Object>(int value, {String? name});",
        "class A = Object with M implements I; mixin M on Object implements I {}",
        "class C(var int x) { this : assert(x > 0) {} C.named() : this(1); C.factory() = D.named; int get value => x; set value(int v) {} C operator +(C other) => this; }",
        "void nativeFunction() native 'nativeName';",
        "void map(void Function({int? e})) {}",
        "void g(int Function() Function() h) {}",
        "class User { final String name; const User({required this.name}); }",
        "class MA = S with M; class C { new(); factory named() = D; int operator *(int x)=>0; } typedef Comparator<T>(T a,T b); extension type E {}",
        "class C { set x(int f(String s)) {} } typedef F(x);",
        "void f() { var value = '${'a' '$b'}'; var missing = () }",
        "async int get x => 1; class C { async int get x => 1; async int operator +(v) => 1; }",
        "base interface mixin class M {} final base int x = 1;",
        "class C { factory {} operator [=(v) {} }",
        r#"void f(dynamic x) async* {
          ; label: while (true) { if (x) continue label; else break label; }
          do { assert(x, 'message'); } while (false);
          for (var i = 0; i < 2; i++) { yield i; }
          for (final (a, b) in x) { yield* x; }
          try { throw x; } on Object catch (e, s) { rethrow; } finally {}
          final (first, second) = x; (first, second) = x;
          var a = -x + x++ * 2; a ??= 1;
          a = x ? this.field : super.field;
          a = x is int; a = x as int; a = await x;
          a = new C.named(); a = .value; a = #symbol;
          a = x<int>; a = x[0]!;
          a = x..field..[0]..method<int>(1, named: 2)..field = 3..[0] = 4;
          a = [1, ?x, ...x, ...?x, if (x case int n when n > 0) n else 0, for (x in x) x];
          a = <int>{1}; a = <int, int>{1: 2, ...x, if (x) 3: 4 else 5: 6, for (x in x) x: x};
          a = (1, name: 2); a = <T>(T v) => v;
          a = 'before ${x + 1} after $x' 'adjacent';
          a = switch (x) { int n when n > 0 => n, _ => throw x };
          switch (x) { case [var first, ...var rest]: break; default: return; }
          if (x case (:var first, named: final second)) {}
          if (x case <int>[_, ...]) {}
          if (x case <String, int>{'key': var value}) {}
          if (x case C(field: var value, :var inferred)) {}
          if (x case (>= 1 && <= 2) || 3) {}
          if (x case var value as int) {}
          if (x case var value?) {}
          if (x case var value!) {}
          if (x case const C.named()) {}
          if (x case C.value) {}
          if (x case null) {}
          if (x case true) {}
          if (x case -1.5) {}
        }"#,
        "void f( { ",
    ] {
        let parsed = parse(source);
        let program = parsed.tree.program(source);
        assert!(!format!("{program:?}").is_empty(), "{source:?}");
        Walk.visit_program(program);
    }
}

#[test]
fn deeply_nested_interpolations_hit_depth_limit_without_overflow() {
    let nested = format!("{}0{}", "'${".repeat(20_000), "}'".repeat(20_000));
    let source = format!("var value = '${{{nested}}}${{{nested}}}';");
    let parsed = parse(&source);
    assert_eq!(
        parsed
            .errors
            .iter()
            .filter(|error| error.message == "nesting too deep")
            .count(),
        1,
        "{:?}",
        parsed.errors,
    );
    assert!(parsed.tree.is_lossless(&source));
}

#[test]
fn unparsed_interpolation_tokens_have_bounded_expansion() {
    let nested = format!("{}0{}", "'${".repeat(20_000), "}'".repeat(20_000));
    let source = format!("var value = '${{a {nested}}}';");
    let parsed = parse(&source);
    assert!(parsed.tree.is_lossless(&source));
    assert!(parsed.tree.tokens(&source).count() < 1_000);
}
