// Positions the analyzer's expected-type helper leaves without a type.
class Fields<T> {
  T? value;
  late final field = value!;
}
void sameField<T>(Fields<T> fields) {
  fields.value = fields.value!;
}
void closures<T>(T? value, bool flag) {
  final single = () {
    return value!;
  };
  final withNull = () {
    if (flag) return null;
    return value!;
  };
  final arrow = () => value!;
  print([single, withNull, arrow]);
}
class Adder<T> {
  int operator +(T value) => 0;
}
Future<void> awaitedOperand<T>(Adder<T> adder, T? value) async {
  print(adder + await value!);
}
Future<void> awaitedKey<T>(T? value, int number) async {
  final Map<T, int> keys = {await value!: number};
  print(keys);
}
void positional<T>(T value) {}
Future<void> awaitedPositional<T>(T? value) async {
  positional(await value!);
}
void nonNullableBound<S extends Object, T extends S>(T? value) {
  final T result = value!;
  print(result);
}
void promoted<T>(T? value, bool flag) {
  value!;
  final T result = value!;
  print(result);
}
void promotedBranch<T>(T? value, bool flag) {
  if (flag) {
    value!;
    final T inner = value!;
    print(inner);
  }
}
void use(Object? value) {}
void intersectionOnly<T>(T? value, bool flag) {
  use(() sync* {
    yield value!;
  });
  use(() {
    if (flag) return value!;
    throw StateError('unreachable');
  });
  use(() {
    if (flag) return value!;
  });
}
