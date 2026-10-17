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

// The analyzer does not search inside assignments that write something else.
class Callbacks {
  void Function()? onCancel;
}

void nestedInAssignment(List<int> target, int targetOffset, int size, Callbacks c) {
  target[targetOffset++] = 1;
  var copy = 0;
  copy = size++;
  c.onCancel = () {
    size = 0;
  };
  print(copy);
}

class InitializerOnly {
  final int value;
  InitializerOnly(int input) : value = input++;
}
