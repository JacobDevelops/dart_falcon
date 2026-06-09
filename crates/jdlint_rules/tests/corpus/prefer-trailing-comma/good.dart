// Good: multi-line lists/calls with trailing commas
void example() {
  functionCall(
    'first',
    'second',
    'third',
  );
}

class MyClass {
  void methodCall(
    String arg1,
    int arg2,
    bool arg3,
  ) {
    print('$arg1 $arg2 $arg3');
  }
}

final result = complex(
  value1,
  value2,
  nested(
    item1,
    item2,
  ),
);

List<String> items = [
  'a',
  'b',
  'c',
];

// OK: single-line calls don't need trailing commas
void single() {
  print('hello');
  functionCall('a', 'b', 'c');
}

// OK: single argument on multiple lines
void singleArg(
  String veryLongArgumentName,
) {
  print(veryLongArgumentName);
}
