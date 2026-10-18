class Example {
  void first() {}

  void second() {}

  @override
  String toString() => 'Example';
}

mixin Actions {
  void first() {}

  void second() {}
}

extension ActionsOnInt on int {
  void first() {}

  void second() {}
}

void topLevel() {}
void anotherTopLevel() {}

class Documented {
  void first() {}

  /// Runs the second action.
  /// The blank line belongs above these comments.
  void second() {}

  /* A block comment can also introduce a method. */
  void third() {}
}

enum Action {
  run;

  /// Executes the action.
  void execute() {}
}
