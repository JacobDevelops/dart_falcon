// Good: using .every() instead of !.where().isEmpty
void validateItems(List<int> items) {
  if (items.every((x) => x > 0)) {
    print('All items are positive');
  }
}

// Good: .every() pattern for checking all elements
void checkAllValid(List<String> values) {
  bool allNonEmpty = values.every((v) => v.isNotEmpty);
  if (allNonEmpty) {
    print('All strings are non-empty');
  }
}

// Good: assignment with .every()
void processCollection() {
  final data = [2, 4, 6, 8];
  final allEven = data.every((n) => n % 2 == 0);
}

// Good: nested every() instead of where().length
void checkMatrix(List<List<int>> matrix) {
  if (matrix.every((row) => row.isNotEmpty)) {
    print('All rows are non-empty');
  }
}

// Good: other iterable methods are fine
void useOtherMethods(List<int> numbers) {
  final filtered = numbers.where((n) => n > 0).toList();
  final hasAny = numbers.any((n) => n > 10);
}
