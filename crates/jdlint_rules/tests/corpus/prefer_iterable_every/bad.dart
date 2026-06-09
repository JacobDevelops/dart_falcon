// Bad: using !.where().isEmpty instead of .every()
void validateItems(List<int> items) {
  if (!items.where((x) => x > 0).isEmpty) { /* expect: prefer_iterable_every */
    print('All items are positive');
  }
}

// Bad: .where().length == list.length pattern
void checkAllValid(List<String> values) {
  bool allNonEmpty = values.where((v) => v.isNotEmpty).length == values.length; /* expect: prefer_iterable_every */
  if (allNonEmpty) {
    print('All strings are non-empty');
  }
}

// Bad: assignment with negated where isEmpty
void processCollection() {
  final data = [2, 4, 6, 8];
  final allEven = !data.where((n) => n % 2 == 0).isEmpty; /* expect: prefer_iterable_every */
}

// Bad: nested where with length comparison
void checkMatrix(List<List<int>> matrix) {
  if (matrix.where((row) => row.isNotEmpty).length == matrix.length) { /* expect: prefer_iterable_every */
    print('All rows are non-empty');
  }
}
