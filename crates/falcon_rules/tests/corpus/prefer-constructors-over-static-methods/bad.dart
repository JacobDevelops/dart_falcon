class Point {
  final num x;
  final num y;
  Point(this.x, this.y);

  static Point polar(num theta, num radius) => /* expect: prefer-constructors-over-static-methods */
      Point(radius * theta, radius * theta);

  static Point origin() { /* expect: prefer-constructors-over-static-methods */
    return Point(0, 0);
  }

  static Point fromList(List<num> xs) { /* expect: prefer-constructors-over-static-methods */
    final p = Point(xs[0], xs[1]);
    return p;
  }
}

class Named {
  final String name;
  Named._(this.name);

  static Named create(String name) => Named._(name.trim()); /* expect: prefer-constructors-over-static-methods */
}

class WithNew {
  WithNew();

  static WithNew make() => new WithNew(); /* expect: prefer-constructors-over-static-methods */
}

class Implicit {
  static Implicit build() => Implicit(); /* expect: prefer-constructors-over-static-methods */
}
