// Tall-style formatting (language 3.7+) disables the lint; pin an older version.
// @dart = 3.6
void call(Object a, [Object? b, Object? c]) {}
void named({Object? a, Object? b}) {}

class Point {
  final int x;
  final int y;

  Point(
    this.x,
    this.y) /* expect: require-trailing-commas */
      : assert(x >= 0,
            'x must be non-negative'); /* expect: require-trailing-commas */

  Point.origin({
    this.x = 0,
    this.y = 0}); /* expect: require-trailing-commas */
}

void declaration(
  int a,
  int b) {} /* expect: require-trailing-commas */

void optional(
  int a, [
  int b = 0]) {} /* expect: require-trailing-commas */

void f() {
  call('a',
      'b'); /* expect: require-trailing-commas */

  named(a: 1,
      b: () {
        print('named function literals get no exemption');
      }); /* expect: require-trailing-commas */

  final list = [
    1,
    2]; /* expect: require-trailing-commas */

  final map = {
    'a': 1,
    'b': 2}; /* expect: require-trailing-commas */

  assert(list.isNotEmpty,
      map.toString()); /* expect: require-trailing-commas */

  call(1, 2,
      3); /* expect: require-trailing-commas */
}

typedef Callback = void Function(
  int a,
  int b); /* expect: require-trailing-commas */

List<Object> nestedLast() => [
      [0, 'a'],
      [1, 'b']
    ]; /* expect: require-trailing-commas */
