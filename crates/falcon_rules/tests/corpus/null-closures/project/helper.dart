class NullApi {
  void any(Object? callback) {}
}

extension ImportedMap on List<int> {
  Iterable<int> map(Object? callback) => this;
}
