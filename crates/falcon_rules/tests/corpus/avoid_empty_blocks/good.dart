class MyClass {
  /// Catch block with handler
  void handleError() {
    try {
      doSomething();
    } catch (e) {
      print('Error: $e');
    }
  }

  /// If with body
  void checkCondition(bool condition) {
    if (condition) {
      print('condition is true');
    }
  }

  /// Else with body
  void checkElse(bool condition) {
    if (condition) {
      print('yes');
    } else {
      print('no');
    }
  }

  /// For loop with body
  void iterate() {
    for (int i = 0; i < 10; i++) {
      print(i);
    }
  }

  /// While loop with body
  void loop() {
    int count = 0;
    while (count < 10) {
      count++;
    }
  }

  /// Method with implementation
  void methodWithBody() {
    print('doing something');
  }

  /// Catch block with comment
  void handleWithComment() {
    try {
      doSomething();
    } catch (e) {
      // Silently ignore this exception
    }
  }
}
