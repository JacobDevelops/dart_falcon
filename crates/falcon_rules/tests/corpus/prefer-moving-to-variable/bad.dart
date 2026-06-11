// Test cases for prefer-moving-to-variable rule
// violations: same non-trivial expression used to initialize two different variables

void duplicateComputation() {
  final a = calculateExpensive(10);
  final b = calculateExpensive(10); /* expect: prefer-moving-to-variable */
}

void duplicateFieldAccess() {
  final x = obj.property.value;
  final y = obj.property.value; /* expect: prefer-moving-to-variable */
}

class Processor {
  void process() {
    final first = getResult(data);
    final second = getResult(data); /* expect: prefer-moving-to-variable */
    combine(first, second);
  }
}

void duplicateBinaryExpression() {
  final x = a + b;
  final y = a + b; /* expect: prefer-moving-to-variable */
}

void duplicateMethodCall() {
  final v1 = list.where((e) => e > 5).toList();
  final v2 = list.where((e) => e > 5).toList(); /* expect: prefer-moving-to-variable */
}
