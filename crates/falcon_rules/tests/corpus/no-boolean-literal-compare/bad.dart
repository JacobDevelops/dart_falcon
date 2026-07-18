// Test cases for no-boolean-literal-compare.
// Only comparisons whose non-literal operand is PROVABLY boolean are flagged.

void knownBooleanComparisons(bool a, bool b, Object o) {
  if (true == false) {} /* expect: no-boolean-literal-compare */
  if (!a == true) {} /* expect: no-boolean-literal-compare */
  if ((a && b) == true) {} /* expect: no-boolean-literal-compare */
  if ((a || b) != false) {} /* expect: no-boolean-literal-compare */
  if ((a == b) == true) {} /* expect: no-boolean-literal-compare */
  if ((o is String) == true) {} /* expect: no-boolean-literal-compare */
}

// The type-resolution layer widens the check to locals/params whose inferred
// static type is a non-nullable `bool`, not just syntactically-boolean operands.
void resolvedNonNullableBooleans(bool flag) {
  if (flag == true) {} /* expect: no-boolean-literal-compare */
  bool ready = flag;
  if (ready != false) {} /* expect: no-boolean-literal-compare */
  final bool done = flag && ready;
  if (done == false) {} /* expect: no-boolean-literal-compare */
}
