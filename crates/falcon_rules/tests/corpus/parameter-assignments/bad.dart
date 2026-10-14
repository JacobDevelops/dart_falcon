class Point {
  int x;
  Point(this.x);

  void move(int dx) {
    dx = dx * 2; /* expect: parameter-assignments */
    x += dx;
  }

  Point.from(int start) : x = 0 {
    start++; /* expect: parameter-assignments */
    x = start;
  }
}

int compute(int value, [int? fallback]) {
  value += 1; /* expect: parameter-assignments */
  fallback ??= 1;
  fallback ??= 2; /* expect: parameter-assignments */
  fallback = 3; /* expect: parameter-assignments */
  return value + fallback;
}

void named({required int count, int step = 1}) {
  count ??= 2; /* expect: parameter-assignments */
  step = 2; /* expect: parameter-assignments */
  print(count + step);
}

void swap(int a, int b) {
  (a, b) = (b, a); /* expect: parameter-assignments */ /* expect: parameter-assignments */
  print(a + b);
}

void closures(int total) {
  void bump() {
    total--; /* expect: parameter-assignments */
  }

  bump();
  final f = (int n) {
    n = 0; /* expect: parameter-assignments */
    return n;
  };
  print(f(total));
}
