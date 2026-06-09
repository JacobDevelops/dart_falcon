// Test cases for binary-expression-operand-order rule
// All lines with violations should have /* expect: binary-expression-operand-order */

void compareWithLiterals() {
  if (5 == x) {} /* expect: binary-expression-operand-order */
  if (null == value) {} /* expect: binary-expression-operand-order */
  if (true == isActive) {} /* expect: binary-expression-operand-order */
  if ("test" == name) {} /* expect: binary-expression-operand-order */
}

void assertLiteralsFirst() {
  assert(42 < count); /* expect: binary-expression-operand-order */
  assert(0 <= index); /* expect: binary-expression-operand-order */
  assert(100 > percentage); /* expect: binary-expression-operand-order */
}

bool checkValues(int x, String s, bool flag) {
  return 10 == x || "hello" == s || false == flag; /* expect: binary-expression-operand-order */ /* expect: binary-expression-operand-order */ /* expect: binary-expression-operand-order */
}

void testInequality() {
  if (5 != x) {} /* expect: binary-expression-operand-order */
  if (null != value) {} /* expect: binary-expression-operand-order */
}

class Validator {
  bool isValid(int age, String email) {
    return 18 <= age && "example" == email; /* expect: binary-expression-operand-order */ /* expect: binary-expression-operand-order */
  }
}

void whileCondition(int count) {
  while (0 < count) { /* expect: binary-expression-operand-order */
    count--;
  }
}

int getValue() {
  return 5 == null ? 0 : 1; /* expect: binary-expression-operand-order */
}

void multipleComparisons(int a, int b, int c) {
  if (1 < a && 2 < b && 3 < c) {} /* expect: binary-expression-operand-order */ /* expect: binary-expression-operand-order */ /* expect: binary-expression-operand-order */
}
