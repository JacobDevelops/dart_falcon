// Each function stays at or below a cyclomatic complexity of 3
// (at most two decision points), so none trigger the rule.

int addTwo(int a, int b) {
  return a + b;
}

int atMostOne(int a) {
  if (a > 0) return a;
  return 0;
}

int clamp(int a, int max) {
  final over = a > max;
  return over ? max : a;
}

int fallback(int? a, int b) {
  return a ?? b;
}

int sumList(List<int> xs) {
  var total = 0;
  for (final x in xs) total += x;
  return total;
}

class Counter {
  int value = 0;

  int increment() {
    value += 1;
    return value;
  }
}
