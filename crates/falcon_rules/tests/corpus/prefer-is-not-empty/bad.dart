void bad() {
  if (!list.isEmpty) return; /* expect: prefer-is-not-empty */
  if (!(items.isEmpty)) return; /* expect: prefer-is-not-empty */
  if (!str.isEmpty) return; /* expect: prefer-is-not-empty */
  if (!map.isEmpty) return; /* expect: prefer-is-not-empty */
  if (!set.isEmpty) return; /* expect: prefer-is-not-empty */
  if (!names.isEmpty) return; /* expect: prefer-is-not-empty */
}

// A class implementing a core collection still inherits `isNotEmpty` (from
// Iterable), so the suggestion is valid and the diagnostic keeps firing.
class Bag implements Iterable<int> {
  int get length => 0;
}

// A class whose supertype chain leaves the project is `Unknown`, never
// `ProvenAbsent` — an unresolved ancestor could contribute `isNotEmpty` — so the
// diagnostic keeps firing (type knowledge suppresses only on positive proof).
class Offsite extends Frobnicator {
  bool get isEmpty => true;
}

void typedReceivers(Bag b, Offsite o) {
  if (!b.isEmpty) return; /* expect: prefer-is-not-empty */
  if (!o.isEmpty) return; /* expect: prefer-is-not-empty */
}
