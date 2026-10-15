// Analyzer-confirmed cases from adv-AdvB/repro/up_misc.dart.
class C { int? x; void m() {} }
void f1(Object? o) {}
void f(C? n, C c) {
  var v = (int);
  (n?.x)?.isEven; /* expect: unnecessary-parenthesis */
  (C.new)(); /* expect: unnecessary-parenthesis */
  (f1)(1); /* expect: unnecessary-parenthesis */
  (c.m)(); /* expect: unnecessary-parenthesis */
  print(v);
}

// Capitalization does not turn a resolved value into a type literal.
const Pi = 3.14;
void uppercaseValue() {
  (Pi).abs(); /* expect: unnecessary-parenthesis */
}
