// Good cases for no-empty-block rule
// No violations expected

void testCatchBlockWithHandler() {
  try {
    riskyOperation();
  } catch (e) {
    logger.error('Operation failed: $e');
  }
}

void testCatchBlockWithRethrow() {
  try {
    riskyOperation();
  } catch (e) {
    rethrow;
  }
}

void testIfBlockWithBody() {
  if (condition) {
    print("yes");
  }
}

void testElseBlockWithBody() {
  if (condition) {
    print("yes");
  } else {
    print("no");
  }
}

void testMethodWithBody() {
  onPressed() {
    print("pressed");
  }
}

class Widget {
  void onTap() {
    handleTap();
  }

  void onLongPress() {
    // Allow long press without action for now
    handleLongPress();
  }
}

void testForLoopWithBody() {
  for (int i = 0; i < 10; i++) {
    print(i);
  }
}

void testWhileLoopWithBody() {
  while (condition) {
    process();
    condition = evaluate();
  }
}

void testFinallyBlockWithBody() {
  try {
    doSomething();
  } finally {
    cleanup();
  }
}

void testIfBlockWithComment() {
  if (impossible) {
    // This condition should never be true in practice
    throw AssertionError('Impossible state reached');
  }
}

void testCatchWithComment() {
  try {
    parse();
  } catch (e) {
    // Intentionally swallow parse errors in legacy format
  }
}

void testAsyncCallback(Future<String> future) {
  future.then((value) {
    print('Got: $value');
  });
}

void testEmptyCallbackWithComment(VoidCallback callback) {
  callback = () {
    // No-op callback for testing
  };
}

class ErrorHandler {
  void handleOptional(Error? error) {
    if (error == null) {
      // No error to handle, silent success
      return;
    }
    log(error);
  }
}
