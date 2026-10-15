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


// SDK types keep the platform's `==`.
class MyException implements Exception {}

// Unresolved interfaces stay silent.
class Remote implements SomeUnknownType {}

// Only class declarations participate in the official lint.
mixin SizeInterface implements Size {}
enum SizeEnum implements Size {
  byte;
  @override
  int get inBytes => 1;
}
