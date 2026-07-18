// Bad: explicit `.new` on a constructor invocation is redundant.
class Foo {
  Foo();
}

class Baz {
  Baz.new(); /* expect: unnecessary-constructor-name */
}

void examples() {
  final a = Foo.new(); /* expect: unnecessary-constructor-name */
  final b = Foo.new()..toString(); /* expect: unnecessary-constructor-name */
  final c = <Foo>[Foo.new()]; /* expect: unnecessary-constructor-name */
  final d = identical(a, Foo.new()); /* expect: unnecessary-constructor-name */
  final e = Foo.new(); /* expect: unnecessary-constructor-name */
  final g = Foo.new(); /* expect: unnecessary-constructor-name */
  print([a, b, c, d, e, g]);
}
