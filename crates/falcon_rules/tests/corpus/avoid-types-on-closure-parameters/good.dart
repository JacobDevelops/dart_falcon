import 'dart:async';

int apply(int Function(int) fn) => fn(1);

void each(List<String> names, void Function(String name) action) {}

void dyn(void Function(dynamic value) action) {}

void untypedContext(Function action) {}

void f(List<String> names) {
  apply((x) => x + 1);
  each(names, (name) {
    print(name);
  });
  dyn((dynamic value) {});
  untypedContext((int x) => x);
  final g = (int x) => x;
  var h = (String s) {};
  print([g, h]);
}

// The official context approximation does not reach returned values, unless
// an `async` body returns `Future<F>`, nor operands or switch expression arms.
int Function(int) returned() {
  return (int x) => x;
}

FutureOr<int Function(int)> eventually() async {
  return (int x) => x;
}

void operand(List<String> names, int Function(String a, String b)? compare) {
  names.sort(compare ?? (String a, String b) => a.compareTo(b));
}

void arms(bool flag, List<int> values) {
  values.where(switch (flag) {
    true => (int value) => value > 0,
    false => (int value) => value < 0,
  });
}

// Nor record literal fields, even under a record context type.
void fields(List<(int, void Function(String))> entries) {
  entries.add((1, (String s) {}));
  final (int, {void Function(String) named}) record = (2, named: (String s) {});
  print(record);
}

typedef Any = dynamic;

void aliasContext(void Function(dynamic) callback) {}

void dynamicAlias() {
  aliasContext((Any value) {});
}

class Sequence<T extends Object> {
  const Sequence.single(T value);
}

void consumeSequences(List<Sequence> values) {}

void downwardConstructorContext() {
  // The raw contextual Sequence fixes T to Object, not the closure's type.
  consumeSequences([Sequence.single((Object value) {})]);
}
