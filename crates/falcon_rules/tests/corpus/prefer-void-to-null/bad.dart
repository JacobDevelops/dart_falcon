Null f() {} /* expect: prefer-void-to-null */

Future<Null> g() async {} /* expect: prefer-void-to-null */

void h(Null x) {} /* expect: prefer-void-to-null */

class A {
  Null field; /* expect: prefer-void-to-null */

  Null method() => null; /* expect: prefer-void-to-null */

  Stream<Null> get events => const Stream.empty(); /* expect: prefer-void-to-null */
}

void main() {
  final List<Null> items = <Null>[null]; /* expect: prefer-void-to-null */ /* expect: prefer-void-to-null */
  print(items);
}

class Base {
  void run() {}
}

class Sub extends Base {
  @override
  Null run() => null; /* expect: prefer-void-to-null */
}

Map<String, Null> lookup() => {}; /* expect: prefer-void-to-null */

class Remote extends SomeUnknownBase {
  Null fetch() => null; /* expect: prefer-void-to-null */
}
