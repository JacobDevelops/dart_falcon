Future<void>? nullableBlock() {
  return null;
}

Future<void>? nullableArrow() => null;

Future<void> nonnullable() async {
  return null; /* expect: avoid-returning-null-for-void */
}

class NullableMethods {
  Future<void>? method() => null;
  Future<void>? get value => null;
}

void localFunctions() {
  Future<void>? local() => null;
  local();
}

Future<void>? nullableAsyncBlock() async {
  return null; /* expect: avoid-returning-null-for-void */
}

Future<void>? nullableAsyncArrow() async => null; /* expect: avoid-returning-null-for-void */

void asyncVoidBlock() async {
  return null;
}

void asyncVoidArrow() async => null;

void syncVoidBlock() {
  return null; /* expect: avoid-returning-null-for-void */
}

Future<void> asyncFutureArrow() async => null; /* expect: avoid-returning-null-for-void */

class VoidOperator {
  void operator []=(int index, int value) {
    return null; /* expect: avoid-returning-null-for-void */
  }
}

class ArrowVoidOperator {
  void operator []=(int index, int value) => null; /* expect: avoid-returning-null-for-void */
}

class NullableOperator {
  int? operator [](int index) => null;
}
