class Size {
  final int inBytes;
  const Size(this.inBytes);

  @override
  bool operator ==(Object other) => other is Size && other.inBytes == inBytes;

  @override
  int get hashCode => inBytes.hashCode;
}

// Extending a value type is fine.
class Kilobytes extends Size {
  const Kilobytes(int kb) : super(kb * 1024);
}

// Implementing a class that keeps identity equality is fine.
abstract class Shape {
  double get area;
}

class Square implements Shape {
  @override
  double get area => 1;
}

// An abstract `==` declaration is not an override with value semantics.
abstract class Comparable2 {
  @override
  bool operator ==(Object other);
}

class Impl implements Comparable2 {}

// A `==` provided by a mixin is not a class override.
mixin Equality {
  @override
  bool operator ==(Object other) => true;
}

class Point implements Equality {}

// SDK types keep the platform's `==`.
class MyException implements Exception {}

// Unresolved interfaces stay silent.
class Remote implements SomeUnknownType {}
