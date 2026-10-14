int twiceOf(int x) => x * 2;

int clamp(int x) {
  if (x < 0) return 0;
  return x;
}

class Box {
  int _value = 0;
  int get value => _value;

  void reset() {
    return;
  }

  void bump() {
    _value++;
  }

  int compute() {
    final y = _value * 2;
    return y;
  }

  Iterable<int> values() sync* {
    yield _value;
  }
}

void main() {
  final f = (int x) {
    print(x);
  };
  f(1);
}
