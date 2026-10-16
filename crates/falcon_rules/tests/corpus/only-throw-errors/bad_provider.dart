import 'dart:async';

class Dependency<T> {
  Dependency(T Function(Reader) create);
}

class Reader {
  T watch<T>(Dependency<T> dependency) => throw StateError('unused');
}

class Deferred<T> {
  Deferred(FutureOr<T> Function(Reader) create);
  R select<R>(R Function(T) selector) => throw StateError('unused');
}

void throwsInDeferredSelector() {
  final dependency = Dependency((ref) => 0);
  final provider = Deferred((ref) async => ref.watch(dependency));
  provider.select((data) => throw data); /* expect: only-throw-errors */
}
