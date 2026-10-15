import 'dart:async';

// A source-defined member makes these tests independent of loaded SDK members.
extension ObserveFuture<T> on Future<T> {
  Future<void> observe() async {}
}

void negativeBranch<T>(FutureOr<T> value) {
  if (value is T) {
    return;
  } else {
    value.observe(); /* expect: discarded-futures */
  }
}

void negatedTest<T>(FutureOr<T> value) {
  if (value is! T) {
    value.observe(); /* expect: discarded-futures */
  }
}

void nullableUnion<T>(FutureOr<T>? value) {
  if (value is T?) return;
  value.observe(); /* expect: discarded-futures */
}
