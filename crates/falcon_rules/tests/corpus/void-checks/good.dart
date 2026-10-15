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
  return;
}

void good(
  Future<int> integerFuture,
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
  // Only `void` and `FutureOr<void>` locations are checked.
  acceptFuture<void>(integerFuture);
  // Function literals are inferred against their `void Function()` context,
  // and function-typed locations never conflict.
  acceptCallback<void>(() => 1);
  acceptCallback<void>(() { return 'x'; });
  final inferredCallback = () => 3;
  acceptCallback<void>(inferredCallback);
  acceptCallback<void>(() {
    if (true) {
      return 'nested';
    }
    for (final value in <int>[1]) {
      print(value);
      return 2;
    }
    return null;
  });
}
