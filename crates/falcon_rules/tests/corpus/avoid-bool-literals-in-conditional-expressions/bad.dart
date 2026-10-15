void f(bool a, bool b, int i) {
  final x = a ? true : b; /* expect: avoid-bool-literals-in-conditional-expressions */
  final y = a ? b : false; /* expect: avoid-bool-literals-in-conditional-expressions */
  final z = a ? false : b; /* expect: avoid-bool-literals-in-conditional-expressions */
  final w = a ? b : true; /* expect: avoid-bool-literals-in-conditional-expressions */
  final v = i > 0 ? true : false; /* expect: avoid-bool-literals-in-conditional-expressions */
  final u = a ? i > 2 : false; /* expect: avoid-bool-literals-in-conditional-expressions */
  print([x, y, z, w, v, u]);
}

// Source-declared numeric getter contracts keep these promotion cases SDK-free.
class Number {
  bool get isInfinite => false;
  bool get isNaN => false;
}

bool infinite(dynamic number) =>
    number is Number ? number.isInfinite : false; /* expect: avoid-bool-literals-in-conditional-expressions */

bool notANumber(dynamic number) =>
    number is Number ? number.isNaN : false; /* expect: avoid-bool-literals-in-conditional-expressions */

bool booleanPromotion(Object? value) =>
    value is bool ? value : false; /* expect: avoid-bool-literals-in-conditional-expressions */

bool inversePromotion(Object? value) =>
    value is! bool ? true : value; /* expect: avoid-bool-literals-in-conditional-expressions */
