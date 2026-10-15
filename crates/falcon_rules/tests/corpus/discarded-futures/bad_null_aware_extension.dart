// Null-aware extension dispatch uses the non-null receiver type, but its
// result remains nullable. No SDK member signatures are needed by this fixture.
extension Recovery<T> on Future<T> {
  Future<T> recover<E extends Object>(void Function(E) handler) => this;
}

void discardRecovery(Future<void>? pending) {
  pending?.recover<Object>((error) {}); /* expect: discarded-futures */
}

void discardDirectRecovery(Future<void> pending) {
  pending.recover<Object>((error) {}); /* expect: discarded-futures */
}

// Nullable type-parameter bounds also need a non-null receiver for dispatch.
extension E on Object {
  Future<void> fire() => throw UnimplementedError();
}

void g<T>(T? v) {
  v?.fire(); /* expect: discarded-futures */
}

void h<T>(T v) {
  v?.fire(); /* expect: discarded-futures */
}
