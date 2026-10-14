class Point {
  const Point(this.x, this.y);
  final int x;
  final int y;

  static const origin = Point(0, 0);
  static const unitX = Point(1, 0);
}

class Flag {
  const Flag();
  const Flag.named();

  static const instance = Flag();
}

const a = Point(0, 0); /* expect: use-named-constants */

void main() {
  const b = Point(1, 0); /* expect: use-named-constants */
  final c = const Point(0, 0); /* expect: use-named-constants */
  print(const Flag()); /* expect: use-named-constants */
  const list = [Point(0, 0)]; /* expect: use-named-constants */
  print([a, b, c, list, const Flag.named()]);
}

class Holder {
  const Holder([this.p = const Point(1, 0)]); /* expect: use-named-constants */
  final Point p;
}
