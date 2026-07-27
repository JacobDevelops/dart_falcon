import 'dart:async';

void accept<T>(T value) {}
void acceptFuture<T>(Future<T> value) {}
void acceptFutureOr<T>(FutureOr<T> value) {}
void acceptCallback<T>(T Function() callback) {}
int makeInt() => 1;

class Box<T> {
  Box(T value);
  T field;
  void setValue(T value) {}
}

void bad(Future<int> integerFuture) {
  accept<void>(1); /* expect: void-checks */
  accept<void>(makeInt()); /* expect: void-checks */
  acceptFuture<void>(integerFuture); /* expect: void-checks */
  acceptFutureOr<void>('text'); /* expect: void-checks */
  acceptCallback<void>(() => 1); /* expect: void-checks */
  acceptCallback<void>(() { return 'x'; }); /* expect: void-checks */
  Box<void>(1); /* expect: void-checks */
  Box<void> box = Box<void>(null);
  box.setValue(1); /* expect: void-checks */
  box.field = 1; /* expect: void-checks */
  List<void> list = [];
  list.add(1); /* expect: void-checks */
  list[0] = 1; /* expect: void-checks */
  final inferredCallback = () => 3;
  acceptCallback<void>(inferredCallback); /* expect: void-checks */
  acceptCallback<void>(() {
    if (true) {
      return 'nested'; /* expect: void-checks */
    }
    for (final value in <int>[1]) {
      print(value);
      return 2; /* expect: void-checks */
    }
    return null;
  });
}
