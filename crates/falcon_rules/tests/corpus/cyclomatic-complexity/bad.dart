// Each function below exceeds the configured max_complexity of 3.
// Complexity = 1 + decision points (if / && / || / ?? / for / while / case / catch / ternary).

int classifyNumber(int a, int b) { /* expect: cyclomatic-complexity */
  if (a > 0) return 1;
  if (b > 0) return 2;
  if (a > b && b > 0) return 3;
  return 0;
}

int loopScan(List<int> xs, int limit) { /* expect: cyclomatic-complexity */
  var total = 0;
  for (final x in xs) total += x;
  while (total > limit) total -= 1;
  final capped = total > 0 || limit < 0;
  final safe = total ?? limit;
  return capped ? safe : 0;
}

int nestedTernary(int a, int b, int c) { /* expect: cyclomatic-complexity */
  final first = a > 0 ? 1 : (b > 0 ? 2 : 3);
  final second = c > 0 && a > 0 ? 4 : 5;
  return first + second;
}

String describe(int code) { /* expect: cyclomatic-complexity */
  switch (code) {
    case 1:
      return 'one';
    case 2:
      return 'two';
    case 3:
      return 'three';
    default:
      return 'other';
  }
}

int guarded(Object value) { /* expect: cyclomatic-complexity */
  try {
    if (value == null) return -1;
  } on FormatException {
    return -2;
  } on StateError {
    return -3;
  }
  return value.hashCode > 0 && value.hashCode < 100 ? 1 : 0;
}

class Calculator {
  int compute(int a, int b) { /* expect: cyclomatic-complexity */
    if (a > b) return a - b;
    if (a < b) return b - a;
    if (a == 0 || b == 0) return 0;
    return a + b;
  }
}

// Decision points inside a labeled loop must still count.
int labeledBranches(List<int> xs) { /* expect: cyclomatic-complexity */
  outer:
  while (xs.isNotEmpty) {
    if (xs.first > 0) return 1;
    if (xs.last < 0) return 2;
    if (xs.length > 5 && xs.first == 0) break outer;
    xs.removeAt(0);
  }
  return 0;
}
