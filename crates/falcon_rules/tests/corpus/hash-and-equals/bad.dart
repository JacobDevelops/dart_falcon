class A {
  @override bool operator ==(Object other) => identical(this, other); /* expect: hash-and-equals */
}

class B {
  @override int get hashCode => 42; /* expect: hash-and-equals */
}

class C {
  @override final int hashCode = 7; /* expect: hash-and-equals */
}

mixin M {
  @override bool operator ==(Object other) => true;
}

class D {
  final int value = 0;
  @override bool operator ==(Object other) => other is D; /* expect: hash-and-equals */
}

class SetterHash {
  set hashCode(int value) {} /* expect: hash-and-equals */
}

class EqualsBase {
  bool operator ==(Object other) => true; /* expect: hash-and-equals */
}

class InheritsEquals extends EqualsBase {
  int get hashCode => 0; /* expect: hash-and-equals */
}

class HashBase {
  int get hashCode => 0; /* expect: hash-and-equals */
}

class InheritsHash extends HashBase {
  bool operator ==(Object other) => true; /* expect: hash-and-equals */
}
