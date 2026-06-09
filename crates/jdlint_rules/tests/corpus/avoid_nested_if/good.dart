class NestedIfExamples {
  void example1(bool a, bool b, bool c) {
    /// Combined condition instead of nesting
    if (a && b) {
      print('both true');
    }
  }

  void example2(bool a, bool b, bool c) {
    /// Using combined conditions
    if (a && b && c) {
      print('all true');
    }
  }

  void example3(bool a, bool b) {
    /// Guard clause with early return
    if (!a) {
      return;
    }

    doSomething();

    if (!b) {
      return;
    }

    print('both conditions met');
  }

  void example4(bool condition) {
    if (condition && true) {
      print('combined condition');
    }
  }

  void example5(bool a, bool b, bool c) {
    if (!a) {
      return;
    }

    if (!b) {
      return;
    }

    if (!c) {
      return;
    }

    print('all checks passed');
  }

  void doSomething() {}
}
