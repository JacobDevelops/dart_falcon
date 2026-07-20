class NestedIfExamples {
  // A single level of nesting is allowed (default max_nesting_level is 2).
  void singleNesting(bool a, bool b) {
    if (a) {
      if (b) {
        print('both true');
      }
    }
  }

  // Combined conditions avoid nesting entirely.
  void combined(bool a, bool b, bool c) {
    if (a && b && c) {
      print('all true');
    }
  }

  // Guard clauses with early returns keep the body flat.
  void guards(bool a, bool b) {
    if (!a) {
      return;
    }
    doSomething();
    if (!b) {
      return;
    }
    print('both conditions met');
  }

  // An if/else-if chain is a sibling chain, not nesting: each then-branch
  // holds no further `if`.
  void elseIfChain(int code) {
    if (code == 1) {
      print('one');
    } else if (code == 2) {
      print('two');
    } else {
      print('other');
    }
  }

  // A labeled `if` with only one nested `if` stays under the threshold.
  void labeledSingle(bool a, bool b) {
    outer:
    if (a) {
      if (b) {
        print('two');
      }
    }
  }

  void doSomething() {}
}
