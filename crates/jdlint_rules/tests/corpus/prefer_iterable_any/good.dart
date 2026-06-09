// Good: using .any() instead of .where().isNotEmpty
void checkItems(List<int> items) {
  if (items.any((x) => x > 5)) {
    print('Found item greater than 5');
  }
}

// Good: .any() pattern for checking conditions
void findValue(List<String> values) {
  bool hasEmpty = values.any((v) => v.isEmpty);
  if (hasEmpty) {
    print('Found empty string');
  }
}

// Good: assignment with .any()
void processCollection() {
  final data = [1, 2, 3, 4, 5];
  final hasLarge = data.any((n) => n > 3);
}

// Good: nested any() instead of where().isNotEmpty
void checkNested(List<List<int>> matrix) {
  if (matrix.any((row) => row.isNotEmpty)) {
    print('Matrix has non-empty rows');
  }
}

// Good: other iterable methods are fine
void useOtherMethods(List<int> numbers) {
  final filtered = numbers.where((n) => n > 0).toList();
  final mapped = numbers.map((n) => n * 2);
}
