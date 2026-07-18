class Point {
  final int x;
  final int y;
  const Point(this.x, this.y);
}

void objectPatterns(Object o) {
  switch (o) {
    case Point(
          x: x, /* expect: avoid-redundant-pattern-field-names */
          y: y, /* expect: avoid-redundant-pattern-field-names */
        ):
      print('$x$y');
    default:
      break;
  }
}

void ifCase(Object o) {
  if (o case Point(x: x)) { /* expect: avoid-redundant-pattern-field-names */
    print(x);
  }
}

int switchExpr(Object o) => switch (o) {
      Point(y: y) => y, /* expect: avoid-redundant-pattern-field-names */
      _ => 0,
    };

void recordPattern(Object o) {
  switch (o) {
    case (
          first: first, /* expect: avoid-redundant-pattern-field-names */
          second: second, /* expect: avoid-redundant-pattern-field-names */
        ):
      print('$first$second');
    default:
      break;
  }
}
