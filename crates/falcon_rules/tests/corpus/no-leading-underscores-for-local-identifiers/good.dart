// Locals without leading underscores; wildcards and members are exempt.

int _private = 0;

class C {
  int _field = 0;
}

void f(int param) {
  print(param);
}

void g() {
  var local = 1;
  print(local);
}

void h(int _) {}

void i() {
  var __ = 1;
  print(__);
}

// Initializing formals and super formals are not local identifiers: the
// underscore denotes the field's privacy and the spelling is forced.
class Provider {
  final String _token;
  Provider(this._token);
}

class NamedProvider {
  final String _token;
  NamedProvider({required this._token});
}

class SubProvider extends Provider {
  SubProvider(super._token);
}

// Declaring primary parameters and extension representations name private fields.
class Primary(final int _field);
class MutablePrimary(var int _field);
extension type Representation(int _value) {}

class ObjectValue {
  final int _value;
  ObjectValue(this._value);
}

void fieldShortcut(ObjectValue input) {
  if (input case ObjectValue(:var _value)) {
    print(_value);
  }
}

void localWildcardFunction() {
  void __() {}
  __();
}

typedef LocalCallback = void Function(int value);

void functionTypedParameter(void Function(int value) callback) {}
