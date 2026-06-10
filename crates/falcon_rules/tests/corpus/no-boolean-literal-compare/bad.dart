// Test cases for no-boolean-literal-compare rule
// All violations are marked inline below.

void testBooleanComparisons() {
  if (isValid == true) {} /* expect: no-boolean-literal-compare */
  if (isActive == false) {} /* expect: no-boolean-literal-compare */
  if (hasError == true) {} /* expect: no-boolean-literal-compare */
  if (isEmpty == false) {} /* expect: no-boolean-literal-compare */
}

void testBooleanInequality() {
  if (isReady != true) {} /* expect: no-boolean-literal-compare */
  if (isDone != false) {} /* expect: no-boolean-literal-compare */
}

void testInWhile() {
  while (isRunning == true) {} /* expect: no-boolean-literal-compare */
  while (shouldContinue == false) {} /* expect: no-boolean-literal-compare */
}

bool checkStatus() {
  return isInitialized == true; /* expect: no-boolean-literal-compare */
}

void assertConditions() {
  assert(isValid == true); /* expect: no-boolean-literal-compare */
  assert(isDisabled == false); /* expect: no-boolean-literal-compare */
}

class Widget {
  bool isEnabled = true;

  void setState() {
    if (isEnabled == true) { /* expect: no-boolean-literal-compare */
      activate();
    }
  }

  bool canRender() => isEnabled == true; /* expect: no-boolean-literal-compare */
}

void ternaryWithBoolean() {
  final status = isActive == true ? "on" : "off"; /* expect: no-boolean-literal-compare */
  final result = isValid == false ? "error" : "ok"; /* expect: no-boolean-literal-compare */
}

void multipleComparisons() {
  if (flag1 == true && flag2 == false) {} /* expect: no-boolean-literal-compare */ /* expect: no-boolean-literal-compare */
}

void methodCall(bool condition) {
  process(condition == true); /* expect: no-boolean-literal-compare */
}
