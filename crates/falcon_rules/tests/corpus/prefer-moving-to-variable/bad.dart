// Test cases for prefer-moving-to-variable rule
// Repeated property/getter chains can share a local binding.

void duplicateGetterChain() {
  final a = service.configuration.value;
  final b = service.configuration.value; /* expect: prefer-moving-to-variable */
}

void duplicateFieldAccess() {
  final x = obj.property.value;
  final y = obj.property.value; /* expect: prefer-moving-to-variable */
}

class Processor {
  void process() {
    final first = this.data.value;
    final second = this.data.value; /* expect: prefer-moving-to-variable */
    combine(first, second);
  }
}

void duplicateNullAwareChain() {
  final x = obj?.property?.value;
  final y = obj?.property?.value; /* expect: prefer-moving-to-variable */
}

void duplicateAcrossReadOnlyDeclaration() {
  final v1 = obj.property.value;
  final unrelated = 42;
  final v2 = obj.property.value; /* expect: prefer-moving-to-variable */
}

void duplicateAcrossPlainString() {
  final before = obj.property.value;
  final separator = ', ';
  final after = obj.property.value; /* expect: prefer-moving-to-variable */
}
