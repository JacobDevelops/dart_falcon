// Each conditional nested inside another conditional is flagged (the inner
// conditional, matching dart_code_linter's nesting-level reporting).

void doubleNesting() {
  final x = a ? (b ? c : d) : e; /* expect: avoid-nested-conditional-expressions */
  final y = condition ? (isValid ? "yes" : "no") : "default"; /* expect: avoid-nested-conditional-expressions */
  final z = (first ? (second ? 1 : 2) : 3) + 4; /* expect: avoid-nested-conditional-expressions */
}

// Triple nesting flags both inner conditionals (nesting levels 2 and 3).
String getStatus() {
  return active ? (verified ? (premium ? "premium" : "standard") : "unverified") : "inactive"; /* expect: avoid-nested-conditional-expressions */ /* expect: avoid-nested-conditional-expressions */
}

class StatusHelper {
  String describe(bool a, bool b, bool c) {
    return a ? (b ? (c ? "all" : "a,b") : "a") : "none"; /* expect: avoid-nested-conditional-expressions */ /* expect: avoid-nested-conditional-expressions */
  }
}

int calculate(bool x, bool y) {
  return x ? (y ? (100) : (50)) : (0); /* expect: avoid-nested-conditional-expressions */
}

List<String> getItems(bool filter) {
  return filter ? (items.isNotEmpty ? (items.toList()) : []) : items; /* expect: avoid-nested-conditional-expressions */
}

// Regression: the violation must still be found inside Dart 3 containers
// (pattern declaration, pattern assignment, labeled statement, switch
// expression, collection if/spread, record field, assert).
void containersRegression(int rcount) {
  final (ra, _) = ((rcount > 0 ? (rcount > 1 ? 1 : 2) : 3), 0); /* expect: avoid-nested-conditional-expressions */
  lbl: {
    final rb = (rcount > 0 ? (rcount > 1 ? 1 : 2) : 3); /* expect: avoid-nested-conditional-expressions */
    print(rb);
  }
  final rc = switch (rcount) {
    0 => (rcount > 0 ? (rcount > 1 ? 1 : 2) : 3), /* expect: avoid-nested-conditional-expressions */
    _ => null,
  };
  final rd = switch ((rcount > 0 ? (rcount > 1 ? 1 : 2) : 3)) { /* expect: avoid-nested-conditional-expressions */
    _ => 0,
  };
  final re = [if (rcount > 0) (rcount > 0 ? (rcount > 1 ? 1 : 2) : 3)]; /* expect: avoid-nested-conditional-expressions */
  final rf = [...[(rcount > 0 ? (rcount > 1 ? 1 : 2) : 3)]]; /* expect: avoid-nested-conditional-expressions */
  final rg = (p: (rcount > 0 ? (rcount > 1 ? 1 : 2) : 3), q: 0); /* expect: avoid-nested-conditional-expressions */
  print([ra, rc, rd, re, rf, rg]);
}
