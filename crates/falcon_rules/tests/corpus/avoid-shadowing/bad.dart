class Parent {
  int inherited = 0;
}

class Example extends Parent {
  int value = 0;
  int get total => 0;
  set status(int next) {}
  static int counter = 0;
  void refresh() {}

  void locals() {
    final value = 1; /* expect: avoid-shadowing */
    var total = 2; /* expect: avoid-shadowing */
    final inherited = 3; /* expect: avoid-shadowing */
    void refresh() {} /* expect: avoid-shadowing */
    final (status, other) = (1, 2); /* expect: avoid-shadowing */
    for (final value in <int>[1]) {} /* expect: avoid-shadowing */
    try {} catch (value) {} /* expect: avoid-shadowing */
  }

  void parameters(int value) {} /* expect: avoid-shadowing */

  void closure() {
    final callback = (int total) => total; /* expect: avoid-shadowing */
  }

  static void staticBody() {
    final counter = 1; /* expect: avoid-shadowing */
  }
}

extension Helpers on Example {
  void helper() {}
  void run() {
    final helper = 1; /* expect: avoid-shadowing */
  }
}

extension TargetMembers on Example {
  void inspect() {
    final value = 1; /* expect: avoid-shadowing */
    final inherited = 2; /* expect: avoid-shadowing */
  }
}

enum Choice {
  first;

  static void inspect() {
    final first = 1; /* expect: avoid-shadowing */
  }
}

class LazyInitializer {
  int value = 0;
  late final callback = (int value) => value; /* expect: avoid-shadowing */
}
