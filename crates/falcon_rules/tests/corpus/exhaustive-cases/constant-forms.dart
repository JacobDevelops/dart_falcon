// Analyzer-confirmed AdvB repros and official parenthesized/dot-shorthand ports.
class Cases {
  final int value;
  const Cases._(this.value);
  static const a = Cases._(1);
  static const b = Cases._(2);
  static const alias = a;
}
class Other { static const b = Cases.b; }
const topA = Cases.a;
void covered(Cases value) {
  switch (value) {
    case topA:
    case Cases.b:
      break;
  }
  switch (value) {
    case const (Cases.a):
    case ((Cases.b)):
      break;
  }
  switch (value) {
    case Cases.alias:
    case Other.b:
      break;
  }
  switch (value) {
    case .a:
    case .b:
      break;
  }
  switch (value) { /* expect: exhaustive-cases */
    case .a:
      break;
  }
}
// Only extends in the defining unit disqualifies an enum-like class.
class Implementation implements Cases {
  final int value = 3;
}
mixin Constraint on Cases {}
