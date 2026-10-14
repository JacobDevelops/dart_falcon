class Rectangle {
  final int width;
  final int height;
  Rectangle(this.width, this.height);
  Rectangle.square(int size)
      : width = size,
        height = size;
}

class ColoredRectangle extends Rectangle {
  final String color;
  ColoredRectangle(this.color, super.width, super.height);
}

class Square extends Rectangle {
  Square(super.size) : super.square();
}

// Named super parameters are matched by name already.
class Base {
  Base({required int a});
}

class Child extends Base {
  Child({required super.a});
}

// Ordinary parameters forwarded explicitly are not super parameters.
class Explicit extends Rectangle {
  Explicit(int w, int h) : super(w, h);
}

// Unresolvable super constructors stay silent.
class Remote extends SomeUnknownBase {
  Remote(super.anything);
}

// More super parameters than the super constructor has: not reported.
class Short {
  Short(int a);
}
