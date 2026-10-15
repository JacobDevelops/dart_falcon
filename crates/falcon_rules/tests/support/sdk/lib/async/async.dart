// Declaration-only SDK excerpt for SDK-free semantic tests.
library dart.async;
abstract class Future<T> {
  external factory Future.value([FutureOr<T>? value]);
}
abstract class FutureOr<T> {}
