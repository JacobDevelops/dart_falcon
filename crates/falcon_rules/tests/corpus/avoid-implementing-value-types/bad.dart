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

mixin SizeLike implements Size {} /* expect: avoid-implementing-value-types */

enum Unit implements Size { /* expect: avoid-implementing-value-types */
  byte;

  @override
  int get inBytes => 1;
}

abstract class Measured implements Kilobytes {} /* expect: avoid-implementing-value-types */
