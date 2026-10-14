class Field {
  final int value;
  Field(this.value);

  @override
  bool operator ==(Object other) {
    if (other is! Field) return false;
    return value == other.value;
  }

  @override
  int get hashCode => value.hashCode;
}

class Pair {
  final int a;
  Pair(this.a);

  @override
  bool operator ==(Object other) => other is Pair && other.a == a;

  @override
  int get hashCode => a;
}

// Pattern-based type test.
class Point {
  final int x;
  Point(this.x);

  @override
  bool operator ==(Object other) => switch (other) {
        Point(:final x) => x == this.x,
        _ => false,
      };

  @override
  int get hashCode => x;
}

// Casting something other than the parameter.
class Holder {
  final Object inner;
  Holder(this.inner);

  @override
  bool operator ==(Object other) =>
      other is Holder && (inner as num) == (other.inner as num);

  @override
  int get hashCode => inner.hashCode;
}

// Casts in ordinary methods are not reported.
class Util {
  bool equals(Object other) => (other as Util) == this;
}
