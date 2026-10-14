// @dart = 2.19

class Point {
  const Point(this.x);
  final int x;
}

const origin = Point(0);

void check(Object? value) {
  switch (value) {
    case const [1, 2]:
      break;
    case const {'a': 1}:
      break;
    case const Point(1):
      break;
    case origin:
      break;
    case -1:
      break;
    case 'text':
      break;
    case null:
      break;
  }
}
