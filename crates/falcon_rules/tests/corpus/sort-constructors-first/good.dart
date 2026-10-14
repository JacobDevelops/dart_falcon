class A {
  A();
  A.named();
  factory A.make() => A();
  int x = 0;
  void m() {}
}

class OnlyMembers {
  int x = 0;
  void m() {}
}

enum E {
  one, two;

  const E();
  int get value => index;
}

mixin M {
  int field = 0;
}

extension type Id(int value) {
  Id.zero() : value = 0;
  int get doubled => value * 2;
}
