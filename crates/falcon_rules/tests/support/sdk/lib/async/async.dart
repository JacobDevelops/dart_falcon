// Declaration-only SDK excerpt for SDK-free semantic tests.
library dart.async;
abstract class Future<T> {
  external factory Future.value([FutureOr<T>? value]);
  external factory Future.error(Object error, [StackTrace? stackTrace]);
}
abstract class FutureOr<T> {}
// Callback signature excerpt pinned to Dart SDK 3.13.5:
// https://github.com/dart-lang/sdk/blob/3.13.5/sdk/lib/async/future.dart
extension FutureExtensions<T> on Future<T> {
  external Future<T> onError<E extends Object>(
    FutureOr<T> handleError(E error, StackTrace stackTrace), {
    bool test(E error)?,
  });
}
abstract class Stream<T> {}
abstract class StreamController<T> implements Sink<T> {
  external factory StreamController();
  Stream<T> get stream;
  Future close();
}
abstract class Timer {
  external factory Timer(Duration duration, void Function() callback);
  void cancel();
}
abstract class Completer<T> {
  external factory Completer();
  external factory Completer.sync();
  void complete([FutureOr<T>? value]);
  void completeError(Object error, [StackTrace? stackTrace]);
  Future<T> get future;
  bool get isCompleted;
}
