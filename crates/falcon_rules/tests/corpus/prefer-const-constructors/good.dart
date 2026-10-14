class Point {
  const Point(this.x, this.y);
  const Point.origin()
      : x = 0,
        y = 0;

  final int x;
  final int y;
}

class Mutable {
  Mutable(this.value);

  int value;
}

class Box<T> {
  const Box(this.value);

  final T value;
}

void use(Object value) {}

Box<T> wrap<T>(T value) => Box<T>(value);

Box<List<T>> empty<T>() => Box([]);

void constructors(int x) {
  const a = Point(1, 2);
  final b = const Point.origin();
  final c = Point(x, 2);
  final d = Mutable(1);
  final e = Object();
  final f = const [Point(1, 2), Point.origin()];
  final g = Unknown(1);
  use(Point(x, x));
  final Point h = const .origin();
  print([a, b, c, d, e, f, g, h]);
}

class Holder {
  const Holder([this.point = const Point(0, 0)]);

  final Point point;
}

@Deprecated('use Point.origin')
const origin = Point(0, 0);
