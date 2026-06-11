void example(List<int> other, Set<String> names) {
  final a = List<int>.from(other); /* expect: prefer-iterable-of */
  final b = Set<String>.from(names); /* expect: prefer-iterable-of */
  final c = List.from(other); /* expect: prefer-iterable-of */
}

class Repo {
  List<int> copy(Iterable<int> src) {
    return List<int>.from(src); /* expect: prefer-iterable-of */
  }

  void copySet(Iterable<String> items) {
    final s = Set<String>.from(items); /* expect: prefer-iterable-of */
  }

  Iterable<dynamic> copyIterable(Iterable<dynamic> data) {
    return Iterable.from(data); /* expect: prefer-iterable-of */
  }
}

void nestedConstructors(List<int> values) {
  final outer = List<int>.from(List.from(values)); /* expect: prefer-iterable-of */ /* expect: prefer-iterable-of */
  final inner = List<int>.from(List.from(values)); /* expect: prefer-iterable-of */ /* expect: prefer-iterable-of */
}

// Legacy explicit `new` constructor form is still flagged.
void legacyNew(List<int> other, Set<String> names) {
  final a = new List<int>.from(other); /* expect: prefer-iterable-of */
  final b = new Set.from(names); /* expect: prefer-iterable-of */
  final c = new Iterable.from(other); /* expect: prefer-iterable-of */
}
