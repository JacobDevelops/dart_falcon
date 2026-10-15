// @dart = 3.13

class Primary(final int value);

// INVALID-CODE: Dart 3.13 rejects final ordinary parameters syntactically.
// The analyzer emits no unnecessary_final lint for those invalid formals.
class Ordinary {
  Ordinary(final int value);
  void method(final int value) {}
}

void parameters(final int value, [final int other = 0]) {
  void local(final int input) {}
  var closure = (final int input) => input;
  local(closure(value + other));
}
