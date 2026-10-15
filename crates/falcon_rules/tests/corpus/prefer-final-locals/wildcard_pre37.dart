// @dart=3.6
// Analyzer-confirmed AdvB4 pfl_wildcard_pre37.dart.
void f() {
  var _ = 1; /* expect: prefer-final-locals */
}
