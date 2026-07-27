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
  @override bool operator ==(Object other) => true; /* expect: hash-and-equals */
}

mixin class MC {
  @override int get hashCode => 1; /* expect: hash-and-equals */
}

class D {
  final int value = 0;
  @override bool operator ==(Object other) => other is D; /* expect: hash-and-equals */
}

// A `hashCode` setter does not override `Object.hashCode`, so `==` is still unpaired.
class SetterHash {
  @override bool operator ==(Object other) => true; /* expect: hash-and-equals */
  set hashCode(int value) {}
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
