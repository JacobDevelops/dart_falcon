void main() {
  print(IterableImplementation);
  print(ComparableImplementation);
}

abstract class IterableImplementation extends Iterable<int> {
  Iterator<int> get iterator => throw StateError('not instantiated');
}

class ComparableImplementation implements Comparable<int> {
  int compareTo(int other) => 0;
}
