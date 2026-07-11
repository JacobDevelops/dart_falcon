typedef OnDone = Future<void> Function(); /* expect: prefer-async-callback */

class A {
  final Future<void> Function() onSave; /* expect: prefer-async-callback */

  A(this.onSave);

  void register(Future<void> Function() cb) {} /* expect: prefer-async-callback */
}

Future<void> Function() make() => () async {}; /* expect: prefer-async-callback */

void f() {
  Future<void> Function() local = () async {}; /* expect: prefer-async-callback */
  local();
}
