// Analyzer-confirmed AdvB4 pfl_object_pattern_decl.dart.
class A {
  int a;
  A(this.a);
}
void f() {
  var A(a: b) = A(1); /* expect: prefer-final-locals */
  var (A(a: c), d) = (A(1), 2); /* expect: prefer-final-locals */
  print([b, c, d]);
}
