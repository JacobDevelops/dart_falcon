import 'dart:math' as math;

bool andGuard(List<int> xs) => xs.isNotEmpty && xs.reduce(math.max) > 10;
bool orGuard(List<int> xs) => xs.isEmpty || xs.reduce(math.max) > 10;

void continueGuard(List<List<int>> groups) {
  for (final g in groups) {
    if (g.isEmpty) continue;
    final maximum = g.reduce(math.max);
    print(maximum);
  }
}

void breakGuard(List<List<int>> groups) {
  outer: for (final g in groups) {
    if (g.isEmpty) {
      break outer;
    }
    final maximum = g.reduce(math.max);
    print(maximum);
  }
}
