class NestedIfExamples {
  // A vertical chain: the outer then-branch contains two further `if`
  // statements (b and c), so the outermost `if` is flagged.
  void chain(bool a, bool b, bool c) {
    if (a) { /* expect: avoid-nested-if */
      if (b) {
        if (c) {
          print('all true');
        }
      }
    }
  }

  // Two sibling `if` statements inside the then-branch also reach the
  // nesting threshold of two.
  void siblings(bool a, bool b, bool c) {
    if (a) { /* expect: avoid-nested-if */
      if (b) {
        print('b');
      }
      if (c) {
        print('c');
      }
    }
  }

  // Nested `if` statements reached through an intervening loop still count.
  void throughLoop(bool a, bool b, List<int> xs) {
    if (a) { /* expect: avoid-nested-if */
      for (final x in xs) {
        if (b) {
          if (x > 0) {
            print(x);
          }
        }
      }
    }
  }

  void doSomething() {}
}
