const k = true;
bool get getter => true;
class A { static bool s = true; }
void compare() {
  print(k == true); /* expect: no-literal-bool-comparisons */
  print(getter == true); /* expect: no-literal-bool-comparisons */
  print(A.s == true); /* expect: no-literal-bool-comparisons */
  print(const bool.fromEnvironment('flag') == true); /* expect: no-literal-bool-comparisons */
}
void recordField((bool, int) r) {
  print(r.$1 == true); /* expect: no-literal-bool-comparisons */
}

class MethodOwner {
  bool foo() => true;
  void compareMethodTearoffs(MethodOwner a) {
    print(foo == true);
    print(a.foo == true);
    print(bar == true); /* expect: no-literal-bool-comparisons */
    print(a.bar == true); /* expect: no-literal-bool-comparisons */
  }
}
extension GetterFallback on MethodOwner {
  bool get foo => true;
  bool get bar => true;
}
