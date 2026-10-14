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

void nestedReturns() {
  if (true) {
    final value = 42;
    return value; /* expect: newline-before-return */
  }
  print('done');
  return; /* expect: newline-before-return */
}

final callback = () {
  print('callback');
  return 1; /* expect: newline-before-return */
};

int switchReturns(int value) {
  switch (value) {
    case 0:
      print(value);
      return value; /* expect: newline-before-return */
    default:
      return 1;
  }
}

void labeledReturn() {
  label: {
    print('label');
    return; /* expect: newline-before-return */
  }
}

class Operand {
  Operand operator +(Operand other) {
    print(other);
    return other; /* expect: newline-before-return */
  }
}
