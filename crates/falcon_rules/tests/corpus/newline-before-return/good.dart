// Test cases for newline-before-return rule
// No violations: return preceded by blank line or is the only statement

int goodFunction(int x) {
  final result = x * 2;

  return result;
}

String processName(String name) {
  final trimmed = name.trim();
  final upper = trimmed.toUpperCase();

  return upper;
}

// Single-statement body: no preceding statement so no blank line needed
int identity(int x) {
  return x;
}

class Calculator {
  int add(int a, int b) {
    final sum = a + b;

    return sum;
  }

  int multiply(int a, int b) {
    final product = a * b;

    return product;
  }
}

// With blank line in nested blocks
void nestedCorrect() {
  if (true) {
    final value = 42;

    return value;
  }

  print('done');

  return;
}

// Arrow function: no block, no check needed
int arrow(int x) => x * 2;

// Conditional with proper spacing
bool check(int x) {
  if (x > 0) {

    return true;
  }

  return false;
}
