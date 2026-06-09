// Bad: multi-line argument list without trailing comma
void example() {
  functionCall(
    'first',
    'second',
    'third' /* expect: prefer-trailing-comma */
  );
}

class MyClass {
  void methodCall(
    String arg1,
    int arg2,
    bool arg3 /* expect: prefer-trailing-comma */
  ) {
    print('$arg1 $arg2 $arg3');
  }
}

final result = complex(
  value1,
  value2,
  nested(
    item1,
    item2 /* expect: prefer-trailing-comma */
  ) /* expect: prefer-trailing-comma */
);

List<String> items = [
  'a',
  'b',
  'c' /* expect: prefer-trailing-comma */
];
