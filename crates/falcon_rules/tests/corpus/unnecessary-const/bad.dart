// Bad: a `const` keyword used where the context is already constant.
class Point {
  final int x;
  final int y;
  const Point(this.x, this.y);
}

const outerList = const [1, 2, 3]; /* expect: unnecessary-const */
const outerMap = const {'a': 1}; /* expect: unnecessary-const */
const outerSet = const {1, 2, 3}; /* expect: unnecessary-const */
const point = const Point(1, 2); /* expect: unnecessary-const */
const nestedList = [const [1, 2]]; /* expect: unnecessary-const */
const wrapped = [const Point(1, 2)]; /* expect: unnecessary-const */
