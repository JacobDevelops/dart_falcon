// Good cases for binary-expression-operand-order rule
// No violations expected

void compareWithVariables() {
  if (x == 5) {}
  if (value == null) {}
  if (isActive == true) {}
  if (name == "test") {}
}

void assertVariablesFirst() {
  assert(count > 42);
  assert(index >= 0);
  assert(percentage < 100);
}

bool checkValues(int x, String s, bool flag) {
  return x == 10 || s == "hello" || flag == false;
}

void testInequality() {
  if (x != 5) {}
  if (value != null) {}
}

class Validator {
  bool isValid(int age, String email) {
    return age >= 18 && email == "example";
  }
}

void whileCondition(int count) {
  while (count > 0) {
    count--;
  }
}

int getValue() {
  return null == 5 ? 0 : 1;
}

void multipleComparisons(int a, int b, int c) {
  if (a > 1 && b > 2 && c > 3) {}
}

void stringComparisons(String name, String expected) {
  if (name == expected) {}
  if (name.isEmpty) {}
  if (name.isNotEmpty) {}
}

void numericOperations(int x, double y, num z) {
  assert(x > 0);
  assert(y < 100.5);
  assert(z >= 10);
}

bool logicalCombinations(bool a, bool b, int count) {
  return a && b || count == 5;
}
