// Good: unused callback parameters renamed to _

void example1() {
  final items = [1, 2, 3];
  items.forEach((_) {
    print('hello');
  });
}

void example2() {
  final map = {'a': 1, 'b': 2};
  map.forEach((_, __) {
    print('hello');
  });
}

void example3() {
  final items = ['x', 'y'];
  items.map((_) => 'mapped').toList();
}

void example4() {
  final numbers = [1, 2, 3];
  numbers.where((_) => true).toList();
}

void example5() {
  final items = [1, 2, 3];
  items.forEach((item) {
    print(item);
  });
}

void example6() {
  final map = {'a': 1, 'b': 2};
  map.forEach((key, value) {
    print('$key: $value');
  });
}
