// Analyzer-confirmed AdvB4: cascade real targets and dynamic extended types.
extension E on int? {
  int m() => 1;
}

extension K on dynamic {
  int km() => 1;
}

void f(int? x) {
  x?..m(); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
  x?.km(); /* expect: unnecessary-null-aware-operator-on-extension-on-nullable */
}
