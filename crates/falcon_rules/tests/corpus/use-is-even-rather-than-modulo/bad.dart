bool even(int x) => x % 2 == 0; /* expect: use-is-even-rather-than-modulo */
bool odd(int x) => x % 2 == 1; /* expect: use-is-even-rather-than-modulo */

void loop(List<int> values) {
  for (final v in values) {
    if (v % 2 == 0) print(v); /* expect: use-is-even-rather-than-modulo */
  }
  final int count = values.length;
  print(count % 2 == 1); /* expect: use-is-even-rather-than-modulo */
}

class Grid {
  Grid(this.size) : assert(size % 2 == 0); /* expect: use-is-even-rather-than-modulo */
  final int size;
  bool get centered => size % 2 == 1; /* expect: use-is-even-rather-than-modulo */
}

bool notTwo(int x) => x % 2 == 2; /* expect: use-is-even-rather-than-modulo */
void defaults([bool flag = 3 % 2 == 1]) {} /* expect: use-is-even-rather-than-modulo */

class Holder<T> {
  Holder(this.test);
  final bool Function(T value) test;
}

extension Filter<T> on List<T> {
  List<T> keep(bool Function(T value) test) => this;
}

void take(bool Function(int value) test) {}

void closures(List<int> values) {
  Holder<int>((i) => i % 2 == 0); /* expect: use-is-even-rather-than-modulo */
  values.keep((i) => i % 2 == 0); /* expect: use-is-even-rather-than-modulo */
  take((i) => i % 2 == 1); /* expect: use-is-even-rather-than-modulo */
}
