// Test cases for avoid-ignoring-return-values rule
// Lines with violations have an expectation annotation

String transform(String input) => input.toUpperCase();

int compute(int x, int y) => x + y;

List<int> buildList() => [1, 2, 3];

void badUsage() {
  transform('hello'); /* expect: avoid-ignoring-return-values */
  compute(2, 3); /* expect: avoid-ignoring-return-values */
  buildList(); /* expect: avoid-ignoring-return-values */
  'hello'.toUpperCase(); /* expect: avoid-ignoring-return-values */
  [1, 2, 3].map((x) => x * 2); /* expect: avoid-ignoring-return-values */
}

class Counter {
  int value = 0;

  int increment() => ++value;

  void badMethodCall() {
    increment(); /* expect: avoid-ignoring-return-values */
  }
}

// `save` is on the receiver-less side-effect allowlist, but the project index
// sees this declaration returns a value, so discarding it is now flagged — a
// precision win the allowlist alone could not make.
int save() => 42;

void resolvedNonVoidDiscard() {
  save(); /* expect: avoid-ignoring-return-values */
}
