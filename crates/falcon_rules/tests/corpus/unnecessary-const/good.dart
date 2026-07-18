// Good: `const` only where it is actually required to establish a constant.
class Point {
  final int x;
  final int y;
  const Point(this.x, this.y);
}

const outerList = [1, 2, 3]; // implicitly const, no keyword needed
const point = Point(1, 2); // implicitly const
const nested = [
  [1],
  [2],
]; // all implicit
final runtime = const [1, 2, 3]; // final var: `const` needed to make it constant
var mutable = const <int>[1]; // `const` needed (context is not constant)

void f() {
  const local = [1, 2, 3]; // implicitly const initializer
  print(const [1, 2, 3]); // `const` needed: argument context is not constant
  print(local);
}
