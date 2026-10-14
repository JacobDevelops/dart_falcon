class Point {
  const Point(this.x, this.y);
  final int x;
  final int y;

  static const origin = Point(0, 0);
  static const alsoOrigin = Point(0, 0);
  static const _secret = Point(9, 9);
}

class Flag {
  const Flag(this.on);
  final bool on;

  static const yes = Flag(true);
}

void main() {
  const notNamed = Point(2, 3);
  final notConst = Point(0, 0);
  const otherFlag = Flag(false);
  final runtimeFlag = Flag(true);
  const viaName = Point.origin;
  print([notNamed, notConst, otherFlag, runtimeFlag, viaName, Point._secret]);
}
