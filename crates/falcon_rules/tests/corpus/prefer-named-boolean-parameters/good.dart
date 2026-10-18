// Public signatures and Function types belong to the shipped rule.
void configure(bool verbose) {}
typedef Callback = void Function(bool enabled);
typedef void Legacy(bool enabled);
void _takesCallback(void callback(bool enabled)) {}
void _takesFunction(void Function(bool enabled) callback) {}

void _named({required bool verbose}) {}
void _optionalNamed({bool? enabled}) {}
void _nonBoolean(int count, String label) {}
void _inferred(enabled) {}

class Settings {
  final bool enabled;
  Settings(bool enabled) : enabled = enabled;
  Settings._inferred(this.enabled);
  Settings._named({required this.enabled});
  void _method({required bool enabled}) {}
  void method(bool enabled) {}
  set flag(bool value) {}
}

class Child extends Settings {
  Child._inferred(super.enabled);
}

class _PrivateClass {
  // Constructor privacy is determined by its own name, not its class name.
  _PrivateClass(bool enabled);
}

class NamedPrimary._hidden({required bool enabled});
class PublicPrimary(bool enabled);

set _flag(bool value) {}
final namedCallback = ({required bool enabled}) {};
final nonBooleanCallback = (int value) => value;
final inferredCallback = (value) => value;

void outer() {
  void local(bool enabled) {}
  void _localNamed({required bool enabled}) {}
}

// An alias is deliberately not an explicit bool spelling.
typedef Flag = bool;
void _alias(Flag enabled) {}

// Lexically shadowing type parameters are not dart:core bool.
void _generic<bool>(bool enabled) {}
final genericCallback = <bool>(bool enabled) {};
class Generic<bool> {
  Generic._hidden(bool enabled);
  void method() {
    void _local(bool enabled) {}
    final callback = (bool enabled) {};
  }
}
void enclosing<bool>() {
  void _local(bool enabled) {}
  final callback = (bool enabled) {};
}

// These positional signatures are fixed by their contexts.
void consume(void Function(bool) callback) {}
void contextualCallbacks() {
  consume((bool enabled) {});
  void Function(bool) callback = (bool enabled) {};
  callback = (bool enabled) {};
}
void Function(bool) returnedCallback() => (bool enabled) {};
final void Function(bool) typedCallback = (bool enabled) {};
class ContextualFields {
  final void Function(bool) callback = (bool enabled) {};
}

// A representation parameter must stay positional.
extension type Representation._hidden(bool enabled) {}
