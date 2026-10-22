// Good: using .every() for a universal predicate.
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

// Negated filtered emptiness is any(predicate), not every(predicate).
void existentialChecks(List<int> items) {
  final hasPositive = !items.where((x) => x > 0).isEmpty;
  final parenthesized = !(items.where((x) => x > 0).isEmpty);
  final nested = !((items.where((x) => x > 0).isEmpty));
  final allNonPositive = items.every((x) => !(x > 0));
  final empty = <int>[].every((x) => !(x > 0));
  print([hasPositive, parenthesized, nested, allNonPositive, empty]);
}
