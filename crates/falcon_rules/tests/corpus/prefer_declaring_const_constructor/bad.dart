// Bad: constructor could be const but isn't
class Point {
  final int x;
  final int y;

  Point(this.x, this.y); /* expect: prefer_declaring_const_constructor */
}

// Bad: immutable class with non-const constructor
class Color {
  final int red;
  final int green;
  final int blue;

  Color(this.red, this.green, this.blue); /* expect: prefer_declaring_const_constructor */
}

// Bad: nested final field class without const
class Coordinate {
  final double latitude;
  final double longitude;
  final String label;

  Coordinate(this.latitude, this.longitude, this.label); /* expect: prefer_declaring_const_constructor */
}

// Bad: immutable model with final fields
class Size {
  final double width;
  final double height;

  Size(this.width, this.height); /* expect: prefer_declaring_const_constructor */
}

// Bad: all-final constructor
class Duration {
  final int seconds;
  final int milliseconds;

  Duration(this.seconds, this.milliseconds); /* expect: prefer_declaring_const_constructor */
}

// Bad: final fields with const-evaluable literal initializers are eligible.
class Flags {
  final bool enabled = true;
  final int count = 0;

  Flags(); /* expect: prefer_declaring_const_constructor */
}
