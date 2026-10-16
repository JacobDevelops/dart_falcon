class Base {
  void foo() {}
  void bar(int x) {}
  int get val => 0;
  set name(String s) {}
  int baz(int a, int b) => 0;
  void qux() {}
}

class Derived extends Base {
  @override
  void foo() => super.foo(); /* expect: unnecessary-overrides */

  @override
  /* expect: unnecessary-overrides */ void bar(int x) {
    super.bar(x);
  }

  @override
  int get val => super.val; /* expect: unnecessary-overrides */

  @override
  set name(String s) => super.name = s; /* expect: unnecessary-overrides */

  @override
  /* expect: unnecessary-overrides */ int baz(int a, int b) {
    return super.baz(a, b);
  }

  @override
  /* expect: unnecessary-overrides */ void qux() {
    super.qux();
  }
}
