import 'dart:async';
import 'dart:core' as core;

void good(Iterable<int> values, Future<int> future, dynamic maybeNull) {
  values.any((value) => value > 0);
  values.firstWhere((value) => true, orElse: () => 0);
  future.then((value) => value + 1, onError: (error) => 0);
  future.complete(null);
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

class CustomIterable implements core.Iterable<int> {
  bool any(Object? callback) => false;
}

extension CustomAny on String {
  bool any(Object? callback) => false;
}

void overrides(CustomIterable values, String text) {
  values.any(null);
  text.any(null);
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
