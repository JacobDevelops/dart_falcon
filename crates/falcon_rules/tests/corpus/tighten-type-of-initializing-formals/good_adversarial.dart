// Analyzer-confirmed AdvB4: parenthesized operands are not identifiers.
class C {
  final int? n;
  C(this.n) : assert((n) != null);
}
