// @dart = 2.14
// Before Dart 2.15 constructors cannot be torn off, so a closure is needed.
class Point {
  Point(this.x);
  final int x;
}

class Box<T> {
  Box(this.inner);
  final List<T> inner;
}

void main(List<int> numbers) {
  final points = numbers.map((x) => Point(x));
  final boxes = [<int>[]].map((inner) => Box<int>(inner));
  final wrap = (List<int> inner) => Box(inner);
  print([points, boxes, wrap]);
}
