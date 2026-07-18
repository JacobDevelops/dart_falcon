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
  Mixed(this._token, int _other); /* expect: no-leading-underscores-for-local-identifiers */
}
