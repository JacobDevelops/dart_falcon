class Point {
  final int x;
  final int y;
  const Point(this.x, this.y);
}

void objectPatterns(Object o) {
  switch (o) {
    case Point(
          x: x, /* expect: avoid_redundant_pattern_field_names */
          y: y, /* expect: avoid_redundant_pattern_field_names */
        ):
      print('$x$y');
    default:
      break;
  }
}

void ifCase(Object o) {
  if (o case Point(x: x)) { /* expect: avoid_redundant_pattern_field_names */
    print(x);
  }
}

int switchExpr(Object o) => switch (o) {
      Point(y: y) => y, /* expect: avoid_redundant_pattern_field_names */
      _ => 0,
    };

void recordPattern(Object o) {
  switch (o) {
    case (
          first: first, /* expect: avoid_redundant_pattern_field_names */
          second: second, /* expect: avoid_redundant_pattern_field_names */
        ):
      print('$first$second');
    default:
      break;
  }
}
