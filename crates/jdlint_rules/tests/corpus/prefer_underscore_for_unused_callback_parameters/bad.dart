// Bad: unused callback parameter should be named _

void example1() {
  final items = [1, 2, 3];
  items.forEach((item) { /* expect: prefer_underscore_for_unused_callback_parameters */
    print('hello');
  });
}

void example2() {
  final map = {'a': 1, 'b': 2};
  map.forEach((key, value) { /* expect: prefer_underscore_for_unused_callback_parameters */
    print('hello');
  });
}

void example3() {
  final items = ['x', 'y'];
  items.map((element) => 'mapped').toList(); /* expect: prefer_underscore_for_unused_callback_parameters */
}

void example4() {
  final numbers = [1, 2, 3];
  numbers.where((n) => true).toList(); /* expect: prefer_underscore_for_unused_callback_parameters */
}
