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
  unknownName;
  unknownPrefix.member;
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
