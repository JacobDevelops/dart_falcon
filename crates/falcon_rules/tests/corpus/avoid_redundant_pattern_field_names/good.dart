class Point {
  final int x;
  final int y;
  const Point(this.x, this.y);
}

void shorthand(Object o) {
  switch (o) {
    case Point(:final x, :final y):
      print('$x$y');
    default:
      break;
  }
}

void renamed(Object o) {
  switch (o) {
    case Point(x: final a, y: final b):
      print('$a$b');
    default:
      break;
  }
}

void differentNames(Object o) {
  if (o case Point(x: y)) {
    print(y);
  }
}

int switchExpr(Object o) => switch (o) {
      Point(:final x) => x,
      _ => 0,
    };

void records(Object o) {
  switch (o) {
    case (first: final a, second: final b):
      print('$a$b');
    default:
      break;
  }
}
