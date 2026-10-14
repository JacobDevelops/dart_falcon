// One positional bool is allowed.
void toggle(bool on) {}

// Named bools are fine.
void configure({bool verbose = false, bool dryRun = false}) {}

void mixed(bool first, {required bool second}) {}

// Private declarations are skipped.
void _private(bool a, bool b) {}

class Base {
  void _hidden(bool a, bool b) {}

  Base._internal(bool a, bool b);
  Base();

  // Getters, setters, and operators are skipped.
  set flag(bool value) {}
  bool get flag => true;
  bool operator ==(Object other) => true;
  int get hashCode => 0;
}

class Sub extends Base {
  Sub() : super();

  // A private constructor is skipped.
  Sub._(bool a, bool b);
}

void nonBool(int a, String b, bool c) {}

void main() {
  _private(true, false);
}
