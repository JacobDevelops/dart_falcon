// Dart 3 libraries use patterns on purpose.
class Point {
  const Point(this.x);
  final int x;
}

void check(Object? value) {
  switch (value) {
    case [1, 2]:
      break;
    case {'a': 1}:
      break;
    case Point(x: 1):
      break;
    case _:
      break;
    case int() || String():
      break;
  }
}
