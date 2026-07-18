// Bad: explicit `new` keyword, which is always optional.
class Foo {
  Foo();
  Foo.named();
}

void examples() {
  final a = new Foo(); /* expect: unnecessary-new */
  final b = new Foo.named(); /* expect: unnecessary-new */
  final c = new List<int>.empty(); /* expect: unnecessary-new */
  final d = new StringBuffer(); /* expect: unnecessary-new */
  final e = new Foo()..toString(); /* expect: unnecessary-new */
  final f = [new Foo()]; /* expect: unnecessary-new */
  print([a, b, c, d, e, f]);
}
