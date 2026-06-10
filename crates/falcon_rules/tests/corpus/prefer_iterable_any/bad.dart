// Bad: using .where(predicate).isNotEmpty instead of .any()
void checkItems(List<int> items) {
  if (items.where((x) => x > 5).isNotEmpty) { /* expect: prefer_iterable_any */
    print('Found item greater than 5');
  }
}

// Bad: .where().isNotEmpty pattern
void findValue(List<String> values) {
  bool hasEmpty = values.where((v) => v.isEmpty).isNotEmpty; /* expect: prefer_iterable_any */
  if (hasEmpty) {
    print('Found empty string');
  }
}

// Bad: assignment with .where().isNotEmpty
void processCollection() {
  final data = [1, 2, 3, 4, 5];
  final hasLarge = data.where((n) => n > 3).isNotEmpty; /* expect: prefer_iterable_any */
}

// Bad: nested where with isNotEmpty
void checkNested(List<List<int>> matrix) {
  if (matrix.where((row) => row.isNotEmpty).isNotEmpty) { /* expect: prefer_iterable_any */
    print('Matrix has non-empty rows');
  }
}
