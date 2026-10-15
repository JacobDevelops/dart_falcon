import 'dart:async';

extension FutureWork<T> on Future<T> {
  Future<void> work(void Function(T) callback) => throw UnimplementedError();
}

void complete<T>(FutureOr<T>? value) {
  if (value is! Future<T>) return;
  value.work((result) {}); /* expect: discarded-futures */
}

void listen(FutureOr<void> value) {
  if (value is Future<void>) {
    value.work((result) {}); /* expect: discarded-futures */
  }
}

void covariantAssignment(Object value, FutureOr<int> replacement) {
  if (value is! FutureOr<num>) return;
  value = replacement;
  if (value is Future<num>) {
    value.work((result) {}); /* expect: discarded-futures */
  }
}

void futureBound<F extends Future<int>>(FutureOr<int> value, F replacement) {
  value = replacement;
  if (value is Future<int>) {
    value.work((result) {}); /* expect: discarded-futures */
  }
}

void nullableValueBranch(FutureOr<int?> value) {
  if (value is Future<int?>) {
    value.work((result) {}); /* expect: discarded-futures */
  }
}

void valueBranch(FutureOr<int> value) {
  if (value is int) {
    value.toString();
  }
}

void nullableFuture(Future<int>? value) {
  if (value == null) return;
  value.work((result) {}); /* expect: discarded-futures */
}
