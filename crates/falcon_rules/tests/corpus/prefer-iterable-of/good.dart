void example(List<int> other, Set<String> names) {
  final a = List<int>.of(other);
  final b = Set<String>.of(names);
  final c = List.of(other);
}

class Repo {
  List<int> copy(Iterable<int> src) {
    return List<int>.of(src);
  }

  void copySet(Iterable<String> items) {
    final s = Set<String>.of(items);
  }

  Iterable<dynamic> copyIterable(Iterable<dynamic> data) {
    return Iterable.of(data);
  }
}

void spreadOperator(List<int> values) {
  final list = [...values];
}

void mapFromIsOk() {
  final map = Map.from({1: 'a', 2: 'b'});
}

void plainConstructors() {
  final a = List<int>();
  final b = Set<String>();
  final c = Iterable<int>();
}

void newOfIsOk(List<int> other) {
  final a = new List<int>.of(other);
  final b = new Set.of(other);
}

void newNonIterableFromIsOk(Map<int, String> m) {
  final a = new Map.from(m);
  final b = new MyThing.from(m);
}
