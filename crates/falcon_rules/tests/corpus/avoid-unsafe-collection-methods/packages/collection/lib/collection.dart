library collection;

extension IterableNumberExtension<T extends num> on Iterable<T> {
  T get max => throw StateError('No elements');
  T get min => throw StateError('No elements');
}
