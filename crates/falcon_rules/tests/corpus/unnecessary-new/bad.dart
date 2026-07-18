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

// ── Reached only via the constructor-initializer walk ─────────────────────────
// Each `new` below lives inside a ConstructorInitializer (field init, super-call,
// this-call, or assert). The default walk descends into those nodes only after
// the visitor traversal-gap fix, so these diagnostics regress if it is reverted.
class Wrapper {
  Wrapper([Object? o]);
}

class InitFieldInit {
  final Object a;
  InitFieldInit() : a = new Foo(); /* expect: unnecessary-new */
}

class InitSuperCall extends Wrapper {
  InitSuperCall() : super(new Foo()); /* expect: unnecessary-new */
}

class InitThisCall {
  final Object a;
  InitThisCall() : this._(new Foo()); /* expect: unnecessary-new */
  InitThisCall._(this.a);
}

class InitAssert {
  InitAssert() : assert(new Foo() != null); /* expect: unnecessary-new */
}
