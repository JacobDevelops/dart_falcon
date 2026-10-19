class Completer<T> {
  void completeError(Object error) {}
}

void custom(Completer<int> value) {
  value.completeError('custom receiver');
  Completer<int>().completeError('custom constructor');
}
