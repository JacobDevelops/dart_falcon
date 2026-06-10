// Test cases for newline-before-return rule
// violations: return statement not preceded by a blank line

int badFunction(int x) {
  final result = x * 2;
  return result; /* expect: newline-before-return */
}

String processName(String name) {
  final trimmed = name.trim();
  final upper = trimmed.toUpperCase();
  return upper; /* expect: newline-before-return */
}

class Calculator {
  int add(int a, int b) {
    final sum = a + b;
    return sum; /* expect: newline-before-return */
  }

  int multiply(int a, int b) {
    final product = a * b;
    return product; /* expect: newline-before-return */
  }
}
