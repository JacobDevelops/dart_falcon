import 'dart:async' as async;

class Stream<T> {}
class Future<T> {}
Stream<Future<int>>? custom;
void scoped<Stream>(async.Future<Stream> value) {}
class Generic<Future> {
  async.Stream<Future>? values;
}
void methods() {
  void local<Stream>(async.Future<Stream> value) {}
}
