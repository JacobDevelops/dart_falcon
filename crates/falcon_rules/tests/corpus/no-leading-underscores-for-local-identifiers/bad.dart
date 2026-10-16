// Local variables and parameters must not start with an underscore.

void f(int _param) {} /* expect: no-leading-underscores-for-local-identifiers */

void g() {
  var _local = 1; /* expect: no-leading-underscores-for-local-identifiers */
  print(_local);
}

void h() {
  final _temp = 2; /* expect: no-leading-underscores-for-local-identifiers */
  print(_temp);
}

void i() {
  for (var _x in <int>[]) { /* expect: no-leading-underscores-for-local-identifiers */
    print(_x);
  }
}

void j() {
  try {
  } catch (_e) { /* expect: no-leading-underscores-for-local-identifiers */
    print(_e);
  }
}

void k() {
  final fn = (int _p) => 0; /* expect: no-leading-underscores-for-local-identifiers */
  print(fn(1));
}

class Mixed {
  final String _token;
  Mixed(this._token, int _other);
}

void localFunction() {
  void _helper() {} /* expect: no-leading-underscores-for-local-identifiers */
  _helper();
}

class Primary(int _argument, final int field); /* expect: no-leading-underscores-for-local-identifiers */

class PatternValue {
  final int _value;
  PatternValue(this._value);
}

void renamedField(PatternValue input) {
  if (input case PatternValue(_value: var _local)) { /* expect: no-leading-underscores-for-local-identifiers */
    print(_local);
  }
}

typedef LocalCallback = void Function(int _value); /* expect: no-leading-underscores-for-local-identifiers */

void functionTypedParameter(
  void Function(int _value) callback, /* expect: no-leading-underscores-for-local-identifiers */
) {}
