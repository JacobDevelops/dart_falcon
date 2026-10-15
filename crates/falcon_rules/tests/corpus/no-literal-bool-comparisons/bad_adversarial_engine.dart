// Covers inferred constants, getters, static members, SDK constructors,
// extension implicit receivers, record fields, and contextual closure parameters.
const k = true;
bool get getter => true;
class A { static bool s = true; }
void compare() {
  print(k == true); /* expect: no-literal-bool-comparisons */
  print(getter == true); /* expect: no-literal-bool-comparisons */
  print(A.s == true); /* expect: no-literal-bool-comparisons */
  print(const bool.fromEnvironment('flag') == true); /* expect: no-literal-bool-comparisons */
}
extension E on List<int> {
  bool m() => isNotEmpty == true; /* expect: no-literal-bool-comparisons */
}
void recordField((bool, int) r) {
  print(r.$1 == true); /* expect: no-literal-bool-comparisons */
}
void closureParameter(List<bool> l) {
  l.any((e) => e == true); /* expect: no-literal-bool-comparisons */
}
