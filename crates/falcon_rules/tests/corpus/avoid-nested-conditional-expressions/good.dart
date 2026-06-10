// Good cases for avoid-nested-conditional-expressions rule
// No violations expected

void testSingleLevelTernary() {
  final x = a ? b : c;
  final y = condition ? "yes" : "no";
  final z = isValid ? 10 : 20;
}

String getStatus() {
  if (!active) return "inactive";
  if (!verified) return "unverified";
  return premium ? "premium" : "standard";
}

class StatusHelper {
  String describe(bool a, bool b, bool c) {
    if (!a) return "none";
    if (!b) return "a";
    return c ? "all" : "a,b";
  }
}

int calculate(bool x, bool y) {
  if (!x) return 0;
  return y ? 100 : 50;
}

void cleanNesting() {
  if (!condition1) {
    return "none";
  }
  if (!condition2) {
    return "shallow";
  }
  return condition3 ? "deep" : "mid";
}

List<String> getItems(bool filter) {
  if (!filter) return items;
  return items.isNotEmpty ? items : [];
}

void mixedApproach(bool condition) {
  final result = condition ? "active" : "inactive";
  print(result);
}

bool isEligible(bool verified, bool premium) {
  return verified && premium;
}
