class Unrelated {
  int unrelated = 0;
}

class Example {
  int value;
  static int count = 0;
  Example(this.value);

  void namesDiffer(int input) {
    final localValue = input;
    final unrelated = 2;
    final callback = (int other) => other;
  }

  static void noInstanceScope(int value) {
    final callback = (int value) => value;
  }

  void signatureOnly(void Function(int value) callback) {}

  void assignmentIsNotDeclaration() {
    (value, count) = (value, count);
  }
}

void topLevel(int value) {
  final count = value;
}

class Another {
  void namesAreLocalToTheOwner() {
    final value = 1;
    final count = 2;
  }
}

extension Separate on Example {
  void unrelated() {}
  static void staticScope() {
    final unrelated = 1;
  }
}

abstract class SignatureOnly {
  int value = 0;
  void abstractMethod(int value);
  external void externalMethod(int value);
  void typedCallback(void callback(int value)) {}
}

class EagerInitializer {
  int value = 0;
  final callback = (int value) => value;
}

class ContextHolder {
  Object get context => Object();
}

class Screen extends ContextHolder {
  Object build(Object context) => context;
}
