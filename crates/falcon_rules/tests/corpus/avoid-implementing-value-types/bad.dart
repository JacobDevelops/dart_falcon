class Size {
  final int inBytes;
  const Size(this.inBytes);

  @override
  bool operator ==(Object other) => other is Size && other.inBytes == inBytes;

  @override
  int get hashCode => inBytes.hashCode;
}

class CustomSize implements Size { /* expect: avoid-implementing-value-types */
  @override
  int get inBytes => 0;
}

// `==` inherited from a superclass still makes it a value type.
class Kilobytes extends Size {
  const Kilobytes(int kb) : super(kb * 1024);
}

class FakeKilobytes implements Kilobytes { /* expect: avoid-implementing-value-types */
  @override
  int get inBytes => 1024;
}

class Other {}

class Both implements Other, Size { /* expect: avoid-implementing-value-types */
  @override
  int get inBytes => 1;
}

mixin SizeLike implements Size {}

enum Unit implements Size {
  byte;

  @override
  int get inBytes => 1;
}

abstract class Measured implements Kilobytes {} /* expect: avoid-implementing-value-types */

abstract class Comparable2 {
  @override
  bool operator ==(Object other);
}
class Impl implements Comparable2 {} /* expect: avoid-implementing-value-types */

mixin Equality {
  @override
  bool operator ==(Object other) => true;
}
class Point implements Equality {} /* expect: avoid-implementing-value-types */
