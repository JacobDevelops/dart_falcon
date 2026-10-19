import 'dart:async' as async;

class Future<T> {}
Future<Future<int>>? custom;
void scoped<Future>(async.Future<Future> value) {}
class Generic<FutureOr> {
  async.Future<FutureOr>? result;
}
void methods() {
  void local<Future>(async.Future<Future> value) {}
}
