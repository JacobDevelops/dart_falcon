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

class D {
  final int value = 0;
  @override bool operator ==(Object other) => other is D; /* expect: hash-and-equals */
}
