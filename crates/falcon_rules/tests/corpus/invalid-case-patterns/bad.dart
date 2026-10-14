// @dart = 2.19

class Point {
  const Point(this.x);
  final int x;
}

void check(Object? value, bool a, bool b) {
  switch (value) {
    case [1, 2]: /* expect: invalid-case-patterns */
      break;
    case {'a': 1}: /* expect: invalid-case-patterns */
      break;
    case Point(1): /* expect: invalid-case-patterns */
      break;
    case identical(1, 2): /* expect: invalid-case-patterns */
      break;
    case _: /* expect: invalid-case-patterns */
      break;
    case (<int>[]): /* expect: invalid-case-patterns */
      break;
  }
}
