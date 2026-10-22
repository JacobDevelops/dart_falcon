// Bad: filtered emptiness means every item fails the predicate.
void validateItems(List<int> items) {
  if (items.where((x) => x > 0).isEmpty) { /* expect: prefer-iterable-every, msg: "Use `every((element) => !predicate(element))` instead of `where(predicate).isEmpty`." */
    print('No items are positive');
  }
}

// Bad: .where().length == list.length pattern
void checkAllValid(List<String> values) {
  bool allNonEmpty = values.where((v) => v.isNotEmpty).length == values.length; /* expect: prefer-iterable-every, msg: "Use `every(predicate)` instead of comparing the filtered and original iterable lengths." */
  if (allNonEmpty) {
    print('All strings are non-empty');
  }
}

// Bad: the predicate must be negated when replacing filtered emptiness.
void processCollection() {
  final data = [2, 4, 6, 8];
  final allOdd = data.where((n) => n % 2 == 0).isEmpty; /* expect: prefer-iterable-every */
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

// Bad: filtered emptiness in a variable declaration.
void verifyList(List<String> items) {
  bool allEmpty = items.where((i) => i.isNotEmpty).isEmpty; /* expect: prefer-iterable-every */
}

// Regression: the violation must still be found inside Dart 3 containers
// (pattern declaration, labeled statement, switch expression and subject,
// collection if/spread, record field).
void containersRegression(int rcount, List<int> items) {
  final (ra, _) = (items.where((x) => x > 1).isEmpty, 0); /* expect: prefer-iterable-every */
  lbl: {
    final rb = items.where((x) => x > 1).isEmpty; /* expect: prefer-iterable-every */
    print(rb);
  }
  final rc = switch (rcount) {
    0 => items.where((x) => x > 1).isEmpty, /* expect: prefer-iterable-every */
    _ => null,
  };
  final rd = switch (items.where((x) => x > 1).isEmpty) { /* expect: prefer-iterable-every */
    _ => 0,
  };
  final re = [if (rcount > 0) items.where((x) => x > 1).isEmpty]; /* expect: prefer-iterable-every */
  final rf = [...[items.where((x) => x > 1).isEmpty]]; /* expect: prefer-iterable-every */
  final rg = (p: items.where((x) => x > 1).isEmpty, q: 0); /* expect: prefer-iterable-every */
  print([ra, rc, rd, re, rf, rg]);
}

bool parenthesized(List<int> items) =>
    (items.where((x) => x > 0).isEmpty); /* expect: prefer-iterable-every */

void universalComparisons(List<int> items) {
  final equalsTrue = items.where((x) => x > 0).isEmpty == true; /* expect: prefer-iterable-every */
  final differsFalse = items.where((x) => x > 0).isEmpty != false; /* expect: prefer-iterable-every */
  final reversedTrue = true == items.where((x) => x > 0).isEmpty; /* expect: prefer-iterable-every */
  final reversedFalse = false != (items.where((x) => x > 0).isEmpty); /* expect: prefer-iterable-every */
  final negatedComparison = !(items.where((x) => x > 0).isEmpty == false); /* expect: prefer-iterable-every */
  final repeatedComparison = (items.where((x) => x > 0).isEmpty == false) == false; /* expect: prefer-iterable-every */
  final doubleNegation = !(!items.where((x) => x > 0).isEmpty); /* expect: prefer-iterable-every */
}

class CollectionOwner {
  final List<int> items = [];
  final List<int> others = [];

  bool allPositive() =>
      this.items.where((x) => x > 0).length == this.items.length; /* expect: prefer-iterable-every */

  bool unrelated() =>
      this.items.where((x) => x > 0).length == this.others.length;
}
