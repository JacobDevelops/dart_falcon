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

class Keyed {
  const Keyed();
  @override
  bool operator ==(Object other) => other is Keyed;
  @override
  int get hashCode => 0;
}

class Wrapper {
  const Wrapper(this.value, {required this.flag}) : assert(value != null);
  final Object? value;
  final bool flag;
}

class Pair<T> {
  const Pair(this.first);
  final T first;
}

class Notify<T> {
  const Notify();
  void call(Pair<T> pair) {}
}

void notConstant(Notify<int> notify, Object? unknown) {
  print(Wrapper({1.5: 1}, flag: true));
  // Only the key itself can be const: `Keyed` overrides `==`.
  print(Wrapper({const Keyed(): 1}, flag: true));
  print(Wrapper(null, flag: true));
}

class Typed<T> {
  Pair<T> make() => Pair(1 as T);
  void send(void Function(Pair<T>) sink) => sink(Pair(null as T));
}
