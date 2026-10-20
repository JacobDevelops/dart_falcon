bool direct(List<int> values) => values.every((value) => value <= 0);
bool directAny(List<int> values) => values.any((value) => value > 0);

// Existing prefer-iterable-any owns the complementary chain.
bool anyMatch(List<int> values) => values.where((value) => value > 0).isNotEmpty;
bool unfiltered(List<int> values) => values.isEmpty;
bool unresolved(dynamic values) => values.where((value) => true).isEmpty;
bool nullable(List<int>? values) => values?.where((value) => true)?.isEmpty ?? true;

class Lookalike {
  Lookalike where(bool Function(int) predicate) => this;
  bool get isEmpty => true;
}
bool unrelated(Lookalike values) => values.where((value) => true).isEmpty;

// Manual loops are not the DCM rule's call-chain contract.
bool manual(List<int> values) {
  for (final value in values) {
    if (value > 0) return true;
  }
  return false;
}
