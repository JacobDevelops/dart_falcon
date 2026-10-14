enum Color { red, green, blue }

class Point {
  final int x;
  Point(this.x);
}

abstract class Shape {
  const Shape();
  static const unit = Circle();
  static const none = Circle();
}

class Circle extends Shape {
  const Circle();
}

extension type const Level._(int value) {
  static const low = Level._(0);
  static const high = Level._(1);
}

void levels(Level level) {
  switch (level) {
    case Level.low:
      print('low');
    default:
      print('other');
  }
}

void f(Color color, int n, String s, Point p, Shape shape, dynamic d) {
  switch (color) {
    case Color.red:
      print('red');
    case Color.green:
    case Color.blue:
      print('other');
  }
  switch (n) {
    case 1:
      print(1);
    default:
      print('n');
  }
  switch (s) {
    case 'a':
      print(s);
    default:
      print('default');
  }
  switch (p) {
    case Point(x: 0):
      print('origin');
    default:
      print('elsewhere');
  }
  switch (shape) {
    case Circle():
      print('circle');
    default:
      print('shape');
  }
  switch (d) {
    case 1:
      print(d);
    default:
      print('d');
  }
  final label = switch (color) {
    Color.red => 'red',
    _ => 'other',
  };
  print(label);
}
