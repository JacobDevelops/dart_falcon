void bad() {
  if (list.length != 0) return; /* expect: prefer-is-not-empty */
  if (0 != items.length) return; /* expect: prefer-is-not-empty */
  if (str.length > 0) return; /* expect: prefer-is-not-empty */
  if (0 < map.length) return; /* expect: prefer-is-not-empty */
  if (set.length >= 1) return; /* expect: prefer-is-not-empty */
  if (1 <= names.length) return; /* expect: prefer-is-not-empty */
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
  int get length => 0;
}

void typedReceivers(Bag b, Offsite o) {
  if (b.length != 0) return; /* expect: prefer-is-not-empty */
  if (o.length != 0) return; /* expect: prefer-is-not-empty */
}
