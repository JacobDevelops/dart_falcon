void good() {
  if (list.isEmpty) return;
  if (items.length == 2) return;
  if (str.length > 0) return;
  if (map.length != 0) return;
  if (set.length >= 1) return;
  if (count == 0) return;
}

// A user class with only a `length` getter — no `isEmpty`, and not a core
// collection/string. Its receiver type is positively proven, so `length == 0`
// is SUPPRESSED: suggesting `isEmpty` would reference a member it does not have.
// (Requires the corpus harness to attach a TypeIndex for this rule; without one
// the receiver is Unknown and this line would fire.)
class Ruler {
  int get length => 3;
}

void suppressed(Ruler r) {
  if (r.length == 0) return;
}

// A named constructor `Box.zero()` still resolves to a `Box` instance (a real
// constructor is not a declared member), and `Box` has only `length` and is no
// collection — so this stays SUPPRESSED, unlike a static-method call.
class Box {
  Box.zero();
  int get length => 0;
}

void suppressedCtor() {
  if (Box.zero().length == 0) return;
}
