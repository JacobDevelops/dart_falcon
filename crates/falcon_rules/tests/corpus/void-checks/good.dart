import 'dart:async';

void accept<T>(T value) {}
void acceptFuture<T>(Future<T> value) {}
void acceptFutureOr<T>(FutureOr<T> value) {}
void acceptCallback<T>(T Function() callback) {}

class Box<T> {
  Box(T value);
  T field;
  void setValue(T value) {}
}

void ordinaryVoid() {
  return 1;
}

void good(
  Future<void> voidFuture,
  Future<Null> nullFuture,
  dynamic dynamicValue,
) {
  accept<void>(null);
  acceptFuture<void>(voidFuture);
  acceptFuture<void>(nullFuture);
  acceptFutureOr<void>(voidFuture);
  acceptFutureOr<void>(dynamicValue);
  acceptCallback<void>(() => null);
  Box<void> box = Box<void>(null);
  box.setValue(null);
  box.field = null;
}
