int get topGetter => 0;

int methodOne() => 1;

int methodTwo() => 2;

class Some {
  const Some();

  int get computed => 1;

  void method() {}

  void run() {
    computed;
    this.computed;
  }
}

Future<void> f(List<int> list, Some some, bool foo, int? a, int b, Object o) async {
  some.method();
  const Some();
  Some();
  methodOne();
  methodTwo();
  foo ? methodOne() : methodTwo();
  foo && list.remove(1);
  a ?? methodOne();
  a = 3;
  b++;
  ++b;
  a!;
  o as int;
  await Future.value(1);
  list..add(1)..clear();
  some.computed;
  topGetter;
  for (var i = 0; i < b; i++) {}
}

int g(int x) {
  return x;
}

void h(Object e) {
  try {
    throw e;
  } catch (_) {
    rethrow;
  }
}

int arrow(int x) => x;

void annotatedLocalFunctions() {
  @pragma('vm:prefer-inline')
  void local() {}
  local();
}

abstract class GetterInterface {
  int get value;
}

class ForwardingGetter implements GetterInterface {
  @override
  dynamic noSuchMethod(Invocation invocation) => 0;
}

void inheritedGetter(ForwardingGetter value) {
  value.value;
}

void functionReturnGetter(GetterInterface Function() create) {
  final value = create();
  value.value;
}

void declaredForUpdates() {
  for (var i = 0; i < 1; i + 1) {}
}

extension<T> on Future<T> {
  void get unawaited {}
}

void extensionGetter(Future<void> Function() work) {
  // An extension getter may have side effects.
  work().then((_) {}).unawaited;
}

class Box {}

extension BoxEffects on Box {
  void get effect {}
}

void localExtensionGetter(Box box) {
  box.effect;
}
