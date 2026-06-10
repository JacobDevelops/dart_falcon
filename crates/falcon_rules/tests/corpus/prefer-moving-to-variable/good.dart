// Test cases for prefer-moving-to-variable rule
// No violations: each variable has a distinct initializer or trivial literal

void distinctComputations() {
  final a = calculateExpensive(10);
  final b = calculateExpensive(20);
}

void singleUse() {
  final result = getResult(data);
  print(result);
}

// Trivial literals are not flagged
void trivialLiterals() {
  final x = 42;
  final y = 42;
  final s1 = "hello";
  final s2 = "hello";
}

class Processor {
  void process() {
    final first = getResult(dataA);
    final second = getResult(dataB);
    combine(first, second);
  }
}
