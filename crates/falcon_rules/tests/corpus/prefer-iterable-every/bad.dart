// Bad: using !.where().isEmpty instead of .every()
void validateItems(List<int> items) {
  if (!items.where((x) => x > 0).isEmpty) { /* expect: prefer-iterable-every */
    print('All items are positive');
  }
}

// Bad: .where().length == list.length pattern
void checkAllValid(List<String> values) {
  bool allNonEmpty = values.where((v) => v.isNotEmpty).length == values.length; /* expect: prefer-iterable-every */
  if (allNonEmpty) {
    print('All strings are non-empty');
  }
}

// Bad: assignment with negated where isEmpty
void processCollection() {
  final data = [2, 4, 6, 8];
  final allEven = !data.where((n) => n % 2 == 0).isEmpty; /* expect: prefer-iterable-every */
}

// Bad: nested where with length comparison
void checkMatrix(List<List<int>> matrix) {
  if (matrix.where((row) => row.isNotEmpty).length == matrix.length) { /* expect: prefer-iterable-every */
    print('All rows are non-empty');
  }
}

// Bad: where with complex predicate and length check
void checkAllPositive(List<int> numbers) {
  final allPositive = numbers.where((n) => n > 0 && n < 100).length == numbers.length; /* expect: prefer-iterable-every */
}

// Bad: negated where isEmpty in variable declaration
void verifyList(List<String> items) {
  bool hasContent = !items.where((i) => i.isNotEmpty).isEmpty; /* expect: prefer-iterable-every */
}
