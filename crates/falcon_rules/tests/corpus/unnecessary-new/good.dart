// Good: instance creation without the `new` keyword.
class Foo {
  Foo();
  Foo.named();
}

void examples() {
  final a = Foo();
  final b = Foo.named();
  final c = StringBuffer();
  final d = <int>[1, 2, 3];
  final e = <int>{};
  final f = 'a literal';
  print([a, b, c, d, e, f]);
}
