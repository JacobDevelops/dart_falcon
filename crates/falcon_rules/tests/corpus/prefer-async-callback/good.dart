typedef OnValue = void Function();
typedef OnData = Future<void> Function(int value);
typedef Getter = Future<int> Function();

class A {
  final void Function() onTap;
  final Future<void> Function(String) onData;

  A(this.onTap, this.onData);
}

Future<int> Function() make() => () async => 1;

void Function() other() => () {};

void f() {
  void Function() local = () {};
  local();
}
