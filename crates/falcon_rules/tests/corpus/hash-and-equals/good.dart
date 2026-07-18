class A {
  @override
  bool operator ==(Object other) => identical(this, other);

  @override
  int get hashCode => 0;
}

class B {
  void method() {}
}

mixin M {
  @override
  bool operator ==(Object other) => true;

  @override
  int get hashCode => 1;
}

class C {
  final int value = 0;
  int get other => 0;
}

enum E { a, b }
