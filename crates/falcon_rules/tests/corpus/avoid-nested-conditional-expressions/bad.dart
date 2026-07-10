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
