class Example {
  void first() {}
  void second() {} /* expect: newline-before-method */

  void spaced() {
    void local() {}
    local();
  }
  void nested() { /* expect: newline-before-method */
    final callback = () { print('not a declaration'); };
    callback();
  }
}

mixin Actions {
  int value = 0;
  void run() {} /* expect: newline-before-method */
}

extension ActionsOnInt on int {
  int get doubled => this * 2;
  void run() {} /* expect: newline-before-method */
}

enum Action {
  run;
  void execute() {} /* expect: newline-before-method */
}

class Documented {
  void first() {}
  /// No blank line above this documentation.
  void second() {} /* expect: newline-before-method */
}
