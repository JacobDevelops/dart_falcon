class Point {
  int x;
  Point(this.x) {
    x = x + 1;
  }

  void move(int dx) {
    x += dx;
  }
}

int compute(int value, [int? fallback]) {
  fallback ??= 1;
  final result = value + fallback;
  return result;
}

void named({int? count}) {
  count ??= 2;
  print(count);
}

void shadowed(int value) {
  void inner(int value) {
    print(value);
  }

  inner(value);
  [1, 2].forEach((value) {
    print(value);
  });
}

void loop(int item, List<int> items) {
  for (item in items) {
    print(item);
  }
}
