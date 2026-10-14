class Point {
  int x = 0;
  int y = 0;
  void reset() {}
  void move(int dx) {}
}

class Holder {
  final List<int> items = [];

  void fill() {
    items.add(1);
    items.add(2); /* expect: cascade-invocations */
  }
}

void declared() {
  final list = <int>[];
  list.add(1); /* expect: cascade-invocations */
  list.add(2);
}

void parameter(Point p) {
  p.reset();
  p.move(1); /* expect: cascade-invocations */
}

void setters(Point p) {
  p.x = 1;
  p.y = 2; /* expect: cascade-invocations */
}

void assigned(Point p) {
  p = Point();
  p.reset(); /* expect: cascade-invocations */
}

void cascades(StringBuffer buffer) {
  buffer..write('a')..write('b');
  buffer.write('c'); /* expect: cascade-invocations */
}

void inSwitch(int value, List<int> list) {
  switch (value) {
    case 1:
      list.clear();
      list.add(value); /* expect: cascade-invocations */
  }
}

void broken(List<int> list, List<int> other) {
  list.add(1);
  list.add(2); /* expect: cascade-invocations */
  other.add(3);
  list.add(4);
}
