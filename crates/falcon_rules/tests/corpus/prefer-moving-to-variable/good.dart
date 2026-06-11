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

// Only one assignment of complex expression
void singleComplexAssignment() {
  final x = obj.method().chain().result;
  print(x);
}

// Identifiers (trivial) don't trigger violation
void identifierDuplicates() {
  final a = variable1;
  final b = variable1;
}

// Different chained calls
void differentChains() {
  final c1 = obj.method1().result;
  final c2 = obj.method2().result;
}
