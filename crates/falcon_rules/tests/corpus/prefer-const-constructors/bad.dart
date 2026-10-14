class Point {
  const Point(this.x, this.y);
  const Point.origin()
      : x = 0,
        y = 0;

  final int x;
  final int y;
}

class Label {
  const Label(this.text, {this.point = const Point.origin()});

  final String text;
  final Point point;
}

class Box<T> {
  const Box(this.value);

  final T value;
}

const int limit = 10;

void use(Object value) {}

void constructors() {
  final a = Point(1, 2); /* expect: prefer-const-constructors */
  final b = Point.origin(); /* expect: prefer-const-constructors */
  final c = new Point(3, 4); /* expect: prefer-const-constructors */
  final d = new Point.origin(); /* expect: prefer-const-constructors */
  final e = Box<int>(limit); /* expect: prefer-const-constructors */
  final f = Label('title', point: const Point(0, 1)); /* expect: prefer-const-constructors */
  use(Point(limit, -1)); /* expect: prefer-const-constructors */
  final List<Point> points = [Point(5, 6)]; /* expect: prefer-const-constructors */
  final Point g = .origin(); /* expect: prefer-const-constructors */
  print([a, b, c, d, e, f, points, g]);
}
