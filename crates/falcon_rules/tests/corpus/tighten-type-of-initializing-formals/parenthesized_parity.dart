// Analyzer-confirmed adv-AdvB4 tt_paren and tt_duplicate_assert cases.
class Parenthesized {
  final int? n;
  Parenthesized(this.n) : assert((n) != null);
}
class Duplicate {
  final int? a;
  Duplicate(this.a) /* expect: tighten-type-of-initializing-formals */
      : assert(a != null),
        assert(a != null);
}

// Analyzer-confirmed adv-AdvB4 tt_extension_type.dart case.
extension type ET(int? i) {
  ET.n(this.i) : assert(i != null); /* expect: tighten-type-of-initializing-formals */
}
