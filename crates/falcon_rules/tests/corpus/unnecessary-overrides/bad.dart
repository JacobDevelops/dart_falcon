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
  void bar(int x) { /* expect: unnecessary-overrides */
    super.bar(x);
  }

  @override
  int get val => super.val; /* expect: unnecessary-overrides */

  @override
  set name(String s) => super.name = s; /* expect: unnecessary-overrides */

  @override
  int baz(int a, int b) { /* expect: unnecessary-overrides */
    return super.baz(a, b);
  }

  @override
  void qux() { /* expect: unnecessary-overrides */
    super.qux();
  }
}
