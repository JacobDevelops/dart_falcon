class Point {
  final num x;
  final num y;
  Point(this.x, this.y);

  Point.polar(num theta, num radius)
      : x = radius * theta,
        y = radius * theta;

  // Nullable return type: a constructor cannot return null.
  static Point? tryParse(String text) => text.isEmpty ? null : Point(0, 0);

  // Generic static method.
  static Point first<T>(List<T> xs) => Point(xs.length, 0);

  // Returns a cached instance without constructing one.
  static final Point _zero = Point(0, 0);
  static Point zero() => _zero;

  // Instance method creating a new instance.
  Point translate(num dx) => Point(x + dx, y);

  // Returns another type.
  static String describe() => Point(0, 0).toString();
}

// Generic classes are skipped.
class Box<T> {
  final T value;
  Box(this.value);
  static Box<int> ofInt() => Box<int>(0);
}

// Factory constructor already.
class Logger {
  Logger._();
  factory Logger.create() => Logger._();
}

// Returning a subtype instance.
class Shape {
  static Shape square() => Square();
}

class Square extends Shape {}

// The analyzer's visitor overwrites earlier matching constructor calls.
class LastCreation {
  static LastCreation create() {
    final value = LastCreation();
    Object();
    return value;
  }
}
