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
