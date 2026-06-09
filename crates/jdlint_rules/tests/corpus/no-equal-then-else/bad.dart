// Test cases for no-equal-then-else rule
// All lines with violations should have /* expect: no-equal-then-else */

void testTernaryWithSameValue() {
  final x = condition ? value : value; /* expect: no-equal-then-else */
  final y = condition ? "text" : "text"; /* expect: no-equal-then-else */
  final z = condition ? 42 : 42; /* expect: no-equal-then-else */
}

void testIfElseWithSameReturn() {
  if (condition) {
    return x; /* expect: no-equal-then-else */
  } else {
    return x;
  }
}

void testIfElseWithSamePrint() {
  if (a) {
    print("value"); /* expect: no-equal-then-else */
  } else {
    print("value");
  }
}

String getStatus(bool active) {
  if (active) {
    return "ready"; /* expect: no-equal-then-else */
  } else {
    return "ready";
  }
}

class Widget {
  void onEvent(bool shouldExecute) {
    if (shouldExecute) {
      callback(); /* expect: no-equal-then-else */
    } else {
      callback();
    }
  }
}

int calculate(bool flag) {
  return flag ? 100 : 100; /* expect: no-equal-then-else */
}

void testComplexExpression() {
  final result = condition ? (a + b) : (a + b); /* expect: no-equal-then-else */
}

bool checkValue(bool test) {
  if (test) {
    return true; /* expect: no-equal-then-else */
  } else {
    return true;
  }
}

void testWithVariables() {
  final x = "same";
  final result = condition ? x : x; /* expect: no-equal-then-else */
}

void testMultilineIfElse() {
  if (condition) {
    final temp = calculate();
    process(temp); /* expect: no-equal-then-else */
  } else {
    final temp = calculate();
    process(temp);
  }
}
