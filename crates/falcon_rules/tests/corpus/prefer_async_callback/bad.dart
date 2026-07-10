typedef OnDone = Future<void> Function(); /* expect: prefer_async_callback */

class A {
  final Future<void> Function() onSave; /* expect: prefer_async_callback */

  A(this.onSave);

  void register(Future<void> Function() cb) {} /* expect: prefer_async_callback */
}

Future<void> Function() make() => () async {}; /* expect: prefer_async_callback */

void f() {
  Future<void> Function() local = () async {}; /* expect: prefer_async_callback */
  local();
}
