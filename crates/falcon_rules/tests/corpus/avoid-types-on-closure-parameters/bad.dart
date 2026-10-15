int apply(int Function(int) fn) => fn(1);

void each(List<String> names, void Function(String name) action) {}

void entries(void Function(String key, int value) action) {}

void typed(void Function(void Function() inner) action) {}

void f(List<String> names) {
  apply((int x) => x + 1); /* expect: avoid-types-on-closure-parameters */
  each(names, (String name) { /* expect: avoid-types-on-closure-parameters */
    print(name);
  });
  entries((String key, int value) {}); /* expect: avoid-types-on-closure-parameters */ /* expect: avoid-types-on-closure-parameters */
  entries((key, int value) {}); /* expect: avoid-types-on-closure-parameters */
  typed((void inner()) {}); /* expect: avoid-types-on-closure-parameters */
  final int Function(int) inc = (int x) => x + 1; /* expect: avoid-types-on-closure-parameters */
  print(inc);
}

// An `async` body returning `Future<F>` gives a returned closure context `F`.
Future<int Function(int)> later() async {
  return (int x) => x; /* expect: avoid-types-on-closure-parameters */
}

final Future<int Function(int)> Function() deferred = () async {
  return (int x) => x; /* expect: avoid-types-on-closure-parameters */
};

void sink(Object value) {}

// A conditional branch's context is the conditional's own static type.
void conditional(bool b) {
  sink(b ? (int x) => x : (int x) => -x); /* expect: avoid-types-on-closure-parameters */ /* expect: avoid-types-on-closure-parameters */
}

class CallbackBox {
  const CallbackBox.named(this.callback);
  final void Function(int) callback;
}

void contravariantAndFieldFormal() {
  // The annotation need not equal the context's parameter type.
  new CallbackBox.named((Object value) {}); /* expect: avoid-types-on-closure-parameters */
}
