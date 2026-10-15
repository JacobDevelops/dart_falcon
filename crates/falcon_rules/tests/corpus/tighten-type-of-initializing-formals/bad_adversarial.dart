// Analyzer-confirmed AdvB4: repeated asserts report the formal only once.
class C {
  final int? a;
  C(this.a) /* expect: tighten-type-of-initializing-formals */
      : assert(a != null),
        assert(a != null);
}

extension type ET(int? i) {
  ET.n(this.i) : assert(i != null); /* expect: tighten-type-of-initializing-formals */
}
