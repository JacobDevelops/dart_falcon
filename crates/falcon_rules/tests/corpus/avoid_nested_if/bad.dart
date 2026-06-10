class NestedIfExamples {
  void example1(bool a, bool b, bool c) {
    /// Two levels of nesting
    if (a) {
      if (b) { /* expect: avoid_nested_if */
        print('both true');
      }
    }
  }

  void example2(bool a, bool b, bool c) {
    /// Three levels of nesting
    if (a) {
      if (b) {
        if (c) { /* expect: avoid_nested_if */
          print('all true');
        }
      }
    }
  }

  void example3(bool a, bool b) {
    if (a) {
      doSomething();
      if (b) { /* expect: avoid_nested_if */
        print('nested after statement');
      }
    }
  }

  void example4(bool condition) {
    if (condition) {
      if (true) { /* expect: avoid_nested_if */
        print('nested');
      }
    }
  }

  void doSomething() {}
}
