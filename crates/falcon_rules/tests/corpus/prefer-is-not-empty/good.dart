void good() {
  if (list.isNotEmpty) return;
  if (items.length == 0) return;
  if (str.length < 1) return;
  if (map.length == 3) return;
  if (set.length > 5) return;
  if (count != 0) return;
}

// A user class with only a `length` getter — no `isNotEmpty`, and not a core
// collection/string. Its receiver type is positively proven, so `length != 0`
// is SUPPRESSED: suggesting `isNotEmpty` would reference a member it lacks.
// (Requires the corpus harness to attach a TypeIndex for this rule; without one
// the receiver is Unknown and this line would fire.)
class Ruler {
  int get length => 3;
}

void suppressed(Ruler r) {
  if (r.length != 0) return;
}
