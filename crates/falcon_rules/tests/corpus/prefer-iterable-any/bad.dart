// Bad: using .where(predicate).isNotEmpty instead of .any()
void checkItems(List<int> items) {
  if (items.where((x) => x > 5).isNotEmpty) { /* expect: prefer-iterable-any */
    print('Found item greater than 5');
  }
}

// Bad: .where().isNotEmpty pattern
void findValue(List<String> values) {
  bool hasEmpty = values.where((v) => v.isEmpty).isNotEmpty; /* expect: prefer-iterable-any */
  if (hasEmpty) {
    print('Found empty string');
  }
}

// Bad: assignment with .where().isNotEmpty
void processCollection() {
  final data = [1, 2, 3, 4, 5];
  final hasLarge = data.where((n) => n > 3).isNotEmpty; /* expect: prefer-iterable-any */
}

// Bad: nested where with isNotEmpty
void checkNested(List<List<int>> matrix) {
  if (matrix.where((row) => row.isNotEmpty).isNotEmpty) { /* expect: prefer-iterable-any */
    print('Matrix has non-empty rows');
  }
}

// Bad: where with complex predicate
void filterByStatus(List<String> statuses) {
  final hasActive = statuses.where((s) => s == 'active' || s == 'pending').isNotEmpty; /* expect: prefer-iterable-any */
}

// Bad: where on map/set
void checkMapKeys(Map<String, int> data) {
  if (data.keys.where((k) => k.startsWith('test')).isNotEmpty) { /* expect: prefer-iterable-any */
    print('Has test keys');
  }
}

// Regression: the violation must still be found inside Dart 3 containers
// (pattern declaration, pattern assignment, labeled statement, switch
// expression, collection if/spread, record field, assert).
void containersRegression(int rcount, List<int> items) {
  final (ra, _) = (items.where((x) => x > 1).isNotEmpty, 0); /* expect: prefer-iterable-any */
  lbl: {
    final rb = items.where((x) => x > 1).isNotEmpty; /* expect: prefer-iterable-any */
    print(rb);
  }
  final rc = switch (rcount) {
    0 => items.where((x) => x > 1).isNotEmpty, /* expect: prefer-iterable-any */
    _ => null,
  };
  final rd = switch (items.where((x) => x > 1).isNotEmpty) { /* expect: prefer-iterable-any */
    _ => 0,
  };
  final re = [if (rcount > 0) items.where((x) => x > 1).isNotEmpty]; /* expect: prefer-iterable-any */
  final rf = [...[items.where((x) => x > 1).isNotEmpty]]; /* expect: prefer-iterable-any */
  final rg = (p: items.where((x) => x > 1).isNotEmpty, q: 0); /* expect: prefer-iterable-any */
  print([ra, rc, rd, re, rf, rg]);
}
