// Good cases for no-boolean-literal-compare rule
// No violations expected

void testBooleanDirect() {
  if (isValid) {}
  if (!isActive) {}
  if (hasError) {}
  if (!isEmpty) {}
}

void testBooleanNegation() {
  if (!isReady) {}
  if (!isDone) {}
}

void testInWhile() {
  while (isRunning) {}
  while (!shouldContinue) {}
}

bool checkStatus() {
  return isInitialized;
}

void assertConditions() {
  assert(isValid);
  assert(!isDisabled);
}

class Widget {
  bool isEnabled = true;

  void setState() {
    if (isEnabled) {
      activate();
    }
  }

  bool canRender() => isEnabled;
}

void ternaryWithBoolean() {
  final status = isActive ? "on" : "off";
  final result = !isValid ? "error" : "ok";
}

void multipleComparisons() {
  if (flag1 && !flag2) {}
}

void methodCall(bool condition) {
  process(condition);
}

void complexLogic(bool a, bool b, bool c) {
  if (a && (b || !c)) {
    print('condition met');
  }
}

bool canAccess(bool isAuthenticated, bool isAuthorized) {
  return isAuthenticated && isAuthorized;
}

void toggleState(bool current) {
  final newState = !current;
  update(newState);
}

class FeatureFlag {
  bool isEnabled = false;

  void check() {
    if (!isEnabled) {
      enable();
    }
  }
}
