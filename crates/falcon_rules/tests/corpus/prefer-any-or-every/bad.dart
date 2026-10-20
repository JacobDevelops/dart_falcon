bool noPositive(List<int> values) =>
    values.where((value) => value > 0).isEmpty; /* expect: prefer-any-or-every */

bool noEmpty(Set<String> values) =>
    values.where((value) => value.isEmpty).isEmpty; /* expect: prefer-any-or-every */

bool noEven(Iterable<int> values) =>
    values.where((value) => value.isEven).isEmpty; /* expect: prefer-any-or-every */

bool noneMatch(List<int> values, bool Function(int) predicate) =>
    values.where(predicate).isEmpty; /* expect: prefer-any-or-every */

bool parenthesized(List<int> values) =>
    ((values.where((value) => value < 0))).isEmpty; /* expect: prefer-any-or-every */
