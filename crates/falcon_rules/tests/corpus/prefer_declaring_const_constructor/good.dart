// Good: const constructor on immutable class
class Point {
  final int x;
  final int y;

  const Point(this.x, this.y);
}

// Good: const constructor for color
class Color {
  final int red;
  final int green;
  final int blue;

  const Color(this.red, this.green, this.blue);
}

// Good: const constructor for immutable coordinate
class Coordinate {
  final double latitude;
  final double longitude;
  final String label;

  const Coordinate(this.latitude, this.longitude, this.label);
}

// Good: mutable class doesn't need const (has non-final fields)
class Widget {
  final String name;
  late String _value;

  Widget(this.name);
}

// Good: const constructor for immutable size
class Size {
  final double width;
  final double height;

  const Size(this.width, this.height);
}

// Good: const constructor for immutable duration
class Duration {
  final int seconds;
  final int milliseconds;

  const Duration(this.seconds, this.milliseconds);
}

// Good: factory constructor (not required to be const)
class Pair {
  final int first;
  final int second;

  const Pair(this.first, this.second);

  factory Pair.symmetric(int value) {
    return Pair(value, value);
  }
}
