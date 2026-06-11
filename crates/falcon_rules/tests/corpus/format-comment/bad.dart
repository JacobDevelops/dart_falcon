// Test cases for format-comment rule
// Violations: line comments starting with lowercase letter

class Example {
  int value = 0;

  void method() {
    // this should be uppercase /* expect: format-comment */
    value = 1;

    // another lowercase comment /* expect: format-comment */
    value = 2;

    // Correct comment — no violation
    value = 3;

    // do something bad /* expect: format-comment */
    print(value);
  }
}

void topLevel() {
  // calculate the result /* expect: format-comment */
  final x = 1 + 2;

  // Result is ready
  print(x);
}

void moreComments() {
  // incorrect start /* expect: format-comment */
  final a = 1;

  // also bad /* expect: format-comment */
  final b = 2;
}
