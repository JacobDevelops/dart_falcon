// Test cases for avoid-nested-conditional-expressions rule
// All violations are annotated inline

void testNestedTernary() {
  final x = a ? (b ? c : d) : e; /* expect: avoid-nested-conditional-expressions */
  final y = condition ? (isValid ? "yes" : "no") : "default"; /* expect: avoid-nested-conditional-expressions */
  final z = (first ? (second ? 1 : 2) : 3) + 4; /* expect: avoid-nested-conditional-expressions */
}

String getStatus() {
  return active ? (verified ? (premium ? "premium" : "standard") : "unverified") : "inactive"; /* expect: avoid-nested-conditional-expressions */
}

class StatusHelper {
  String describe(bool a, bool b, bool c) {
    return a ? (b ? (c ? "all" : "a,b") : "a") : "none"; /* expect: avoid-nested-conditional-expressions */
  }
}

int calculate(bool x, bool y) {
  return x ? (y ? (100) : (50)) : (0); /* expect: avoid-nested-conditional-expressions */
}

void complexNesting() {
  final result = condition1 /* expect: avoid-nested-conditional-expressions */
    ? (condition2 ? (condition3 ? "deep" : "mid") : "shallow")
    : "none";
}

List<String> getItems(bool filter) {
  return filter ? (items.isNotEmpty ? (items.where((i) => i != null).toList()) : []) : items; /* expect: avoid-nested-conditional-expressions */
}
