// @dart=3.12
// Private named initializing formals are available starting in Dart 3.12.
class PrivateNamed {
  int _value;
  PrivateNamed({required int value}) : _value = value; /* expect: prefer-initializing-formals */
  int get value => _value;
}
