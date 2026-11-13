class A { const A.c({bool f = true}); }
void test() {
  const A.c(f: true); /* expect: avoid-redundant-argument-values */
  new A.c(f: true); /* expect: avoid-redundant-argument-values */
  final a = const A.c(f: true); /* expect: avoid-redundant-argument-values */
  A.c(f: true); /* expect: avoid-redundant-argument-values */
}
