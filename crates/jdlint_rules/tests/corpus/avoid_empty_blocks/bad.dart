class MyClass {
  /// Empty catch block
  void handleError() {
    try {
      doSomething();
    } catch (e) { /* expect: avoid_empty_blocks */
    }
  }

  /// Empty if body
  void checkCondition(bool condition) {
    if (condition) { /* expect: avoid_empty_blocks */
    }
  }

  /// Empty else body
  void checkElse(bool condition) {
    if (condition) {
      print('yes');
    } else { /* expect: avoid_empty_blocks */
    }
  }

  /// Empty for loop
  void iterate() {
    for (int i = 0; i < 10; i++) { /* expect: avoid_empty_blocks */
    }
  }

  /// Empty while loop
  void loop() {
    while (true) { /* expect: avoid_empty_blocks */
    }
  }

  /// Empty function body
  void emptyMethod() { /* expect: avoid_empty_blocks */
  }
}
