// Constructor-initializer arguments, field and `for` initializers, prefixed
// assignments, closure returns inferring `T`, awaited positions, and nested
// type-parameter bounds.
class Base<T> {
  Base(T value);
  Base.named(T? value) : this(value!); /* expect: null-check-on-nullable-type-parameter */
}
class Sub<T> extends Base<T> {
  Sub(T? value) : super(value!); /* expect: null-check-on-nullable-type-parameter */
}
class Fields<T> {
  T? value;
  late T field = value!; /* expect: null-check-on-nullable-type-parameter */
  void own() {
    this.value = value!; /* expect: null-check-on-nullable-type-parameter */
  }
}
void typedFor<T>(T? value) {
  for (final T item = value!; ;) { /* expect: null-check-on-nullable-type-parameter */
    print(item);
    break;
  }
}
void untypedFor<T>(T? value) {
  for (var item = value!; ;) { /* expect: null-check-on-nullable-type-parameter */
    print(item);
    break;
  }
}
void otherField<T>(Fields<T> left, Fields<T> right) {
  left.value = right.value!; /* expect: null-check-on-nullable-type-parameter */
}
void closureUpperBound<T>(T? value, T other, bool flag) {
  final callback = () {
    if (flag) return other;
    return value!; /* expect: null-check-on-nullable-type-parameter */
  };
  print(callback);
}
void closureNullable<T>(T? value, T? other, bool flag) {
  final callback = () {
    if (flag) return other;
    return value!; /* expect: null-check-on-nullable-type-parameter */
  };
  print(callback);
}
void asyncClosure<T>(T? value, T other, bool flag) {
  final callback = () async {
    if (flag) return other;
    return value!; /* expect: null-check-on-nullable-type-parameter */
  };
  print(callback);
}
void untypedLocal<T>(T? value, T other, bool flag) {
  local() {
    if (flag) return other;
    return value!; /* expect: null-check-on-nullable-type-parameter */
  }
  print(local);
}
class Adder<T> {
  int operator +(T value) => 0;
}
void operand<T>(Adder<T> adder, T? value) {
  print(adder + value!); /* expect: null-check-on-nullable-type-parameter */
}
Future<void> awaitedKey<T>(T? value, T other) async {
  final Map<T, T> map = {await value!: other}; /* expect: null-check-on-nullable-type-parameter */
  print(map);
}
Future<void> awaitedValue<T>(T? value, int number) async {
  final Map<int, T> map = {number: await value!}; /* expect: null-check-on-nullable-type-parameter */
  print(map);
}
void named<T>({required T value}) {}
Future<void> awaitedNamed<T>(T? value) async {
  named(value: await value!); /* expect: null-check-on-nullable-type-parameter */
}
Future<void> awaitedAssignment<T>(T? value) async {
  late T result;
  result = await value!; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
void nestedBound<S, T extends S>(T? value) {
  final T typed = value!; /* expect: null-check-on-nullable-type-parameter */
  print(typed);
}
void nestedInferred<S, T extends S>(T? value) {
  final inferred = value!; /* expect: null-check-on-nullable-type-parameter */
  print(inferred);
}
void joinedPromotion<T>(T? value, bool flag) {
  if (flag) value!;
  final T result = value!; /* expect: null-check-on-nullable-type-parameter */
  print(result);
}
void use(Object? value) {}
void fallThrough<T>(T? value, T other, bool first, bool second) {
  use(() {
    if (first) return other;
    if (second) return value!; /* expect: null-check-on-nullable-type-parameter */
  });
}
void ifElse<T>(T? value, T other, bool flag) {
  use(() {
    if (flag) {
      return other;
    } else {
      return value!; /* expect: null-check-on-nullable-type-parameter */
    }
  });
}
void switchReturns<T>(T? value, T other, bool flag) {
  use(() {
    switch (flag) {
      case true:
        return other;
      case false:
        return value!; /* expect: null-check-on-nullable-type-parameter */
    }
  });
}
void generator<T>(T? value, T other, bool flag) {
  use(() sync* {
    if (flag) yield other;
    yield value!; /* expect: null-check-on-nullable-type-parameter */
  });
}
void asyncGenerator<T>(T? value, T other, bool flag) {
  use(() async* {
    if (flag) yield other;
    yield value!; /* expect: null-check-on-nullable-type-parameter */
  });
}
void localIfElse<T>(T? value, T other, bool flag) {
  local() {
    if (flag) {
      return other;
    } else {
      return value!; /* expect: null-check-on-nullable-type-parameter */
    }
  }
  use(local);
}
