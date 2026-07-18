// Bad: unused callback parameter should be named _

void example1() {
  final items = [1, 2, 3];
  items.forEach((item) { /* expect: prefer-underscore-for-unused-callback-parameters */
    print('hello');
  });
}

void example2() {
  final map = {'a': 1, 'b': 2};
  map.forEach((key, value) { /* expect: prefer-underscore-for-unused-callback-parameters */
    print('hello');
  });
}

void example3() {
  final items = ['x', 'y'];
  items.map((element) => 'mapped').toList(); /* expect: prefer-underscore-for-unused-callback-parameters */
}

void example4() {
  final numbers = [1, 2, 3];
  numbers.where((n) => true).toList(); /* expect: prefer-underscore-for-unused-callback-parameters */
}

// Bad: unused parameter in reduce-like operation
void example5() {
  final values = [1, 2, 3, 4];
  values.fold(0, (previous, current) => previous + 1); /* expect: prefer-underscore-for-unused-callback-parameters */
}

// Bad: unused index parameter in indexed map
void example6() {
  final items = ['a', 'b', 'c'];
  items.asMap().forEach((index, value) { /* expect: prefer-underscore-for-unused-callback-parameters */
    print(value);
  });
}
