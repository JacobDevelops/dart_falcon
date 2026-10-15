import 'dart:async';

void good(
  Iterable<int> values,
  Future<int> future,
  Completer<int> completer,
  dynamic maybeNull,
) {
  values.any((value) => value > 0);
  values.firstWhere((value) => true, orElse: () => 0);
  future.then((value) => value + 1, onError: (error) => 0);
  completer.complete(null);
  Future.microtask(() => 1);
  scheduleMicrotask(() {});
  values.any(maybeNull);
}

class Iterable<T> {
  bool any(Object? callback) => false;
}

void shadowed(Iterable<int> values) {
  values.any(null);
}

extension CustomAny on String {
  bool any(Object? callback) => false;
}

void overrides(String text) {
  text.any(null);
}

// Neither `Future.delayed` nor the top-level `scheduleMicrotask` is on the
// official list, and cascaded calls have no target to check.
void unlisted(List<int> list) {
  Future.delayed(Duration.zero, null);
  scheduleMicrotask(null);
  list..removeWhere(null);
}

class AsyncShadow {
  final FutureFactory Future = FutureFactory();
}

class FutureFactory {
  void microtask(Object? callback) {}
}

void prefixedSdkShadow(AsyncShadow async) {
  async.Future.microtask(null);
}
