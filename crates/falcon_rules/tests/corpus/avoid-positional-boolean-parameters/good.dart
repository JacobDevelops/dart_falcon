// Named bools are fine.
void configure({bool verbose = false, bool dryRun = false}) {}

void mixed(int first, {required bool second}) {}

// Private declarations are skipped.
void _private(bool a) {}

class Base {
  void _hidden(bool a) {}

  Base._internal(bool a);
  Base();

  // Getters, method setters, and operators are skipped.
  set flag(bool value) {}
  bool get flag => true;
  bool operator ==(Object other) => true;
  int get hashCode => 0;
}

class Sub extends Base {
  Sub() : super();

  // A private constructor is skipped.
  Sub._(bool a);
}

// Legacy typedefs and function-typed parameters are not function types.
typedef void Legacy(bool a);

void takes(void callback(bool a)) {}

void nonBool(int a, String b, Object c) {}

void main() {
  _private(true);
  Base._internal(true);
  Sub._(true);
}

class NamedPrimaryBoolean({bool enabled = false});

class PrivatePrimaryBoolean._hidden(bool enabled);
