// Good: no redundant `.new`. Bare `X.new` tear-offs keep `.new` (it is required there).
class Foo {
  Foo();
  Foo.named();
}

void examples() {
  final tearOff = Foo.new; // tear-off of the default constructor — `.new` required
  final a = Foo();
  final b = Foo.named();
  final list = [1, 2, 3].map((_) => Foo.new).toList();
  final c = Foo.named()..toString();
  final d = StringBuffer();
  print([tearOff, a, b, list, c, d]);
}
