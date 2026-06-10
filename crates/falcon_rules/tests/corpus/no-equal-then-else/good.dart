// Good cases for no-equal-then-else rule
// No violations expected

void testTernaryWithDifferentValues() {
  final x = condition ? value1 : value2;
  final y = condition ? "yes" : "no";
  final z = condition ? 42 : 0;
}

void testIfElseWithDifferentReturns() {
  if (condition) {
    return success;
  } else {
    return failure;
  }
}

void testIfElseWithDifferentPrints() {
  if (a) {
    print("active");
  } else {
    print("inactive");
  }
}

String getStatus(bool active) {
  if (active) {
    return "ready";
  } else {
    return "waiting";
  }
}

class Widget {
  void onEvent(bool shouldExecute) {
    if (shouldExecute) {
      onSuccess();
    } else {
      onFailure();
    }
  }
}

int calculate(bool flag) {
  return flag ? 100 : 50;
}

void testComplexExpression() {
  final result = condition ? (a + b) : (c + d);
}

bool checkValue(bool test) {
  if (test) {
    return true;
  } else {
    return false;
  }
}

void testWithDifferentVariables() {
  final x = "first";
  final y = "second";
  final result = condition ? x : y;
}

void testUnconditionalReturn() {
  return value;
}

void testConditionalWithSideEffects() {
  if (condition) {
    log("path A");
    processA();
  } else {
    log("path B");
    processB();
  }
}

String handleResult(Result result) {
  if (result.isSuccess) {
    return result.value;
  } else if (result.error != null) {
    return result.error!.message;
  } else {
    return "Unknown error";
  }
}

void testNestedIfElse(bool a, bool b) {
  if (a) {
    if (b) {
      print("a and b");
    } else {
      print("a not b");
    }
  } else {
    print("not a");
  }
}

int priority(bool isUrgent) {
  return isUrgent ? 1 : 10;
}
