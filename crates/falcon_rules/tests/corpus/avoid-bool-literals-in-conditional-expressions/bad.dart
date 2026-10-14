void f(bool a, bool b, int i) {
  final x = a ? true : b; /* expect: avoid-bool-literals-in-conditional-expressions */
  final y = a ? b : false; /* expect: avoid-bool-literals-in-conditional-expressions */
  final z = a ? false : b; /* expect: avoid-bool-literals-in-conditional-expressions */
  final w = a ? b : true; /* expect: avoid-bool-literals-in-conditional-expressions */
  final v = i > 0 ? true : false; /* expect: avoid-bool-literals-in-conditional-expressions */
  final u = a ? i > 2 : false; /* expect: avoid-bool-literals-in-conditional-expressions */
  print([x, y, z, w, v, u]);
}
