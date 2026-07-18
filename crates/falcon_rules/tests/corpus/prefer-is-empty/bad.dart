void bad() {
  if (list.length == 0) return; /* expect: prefer-is-empty */
  if (0 == items.length) return; /* expect: prefer-is-empty */
  if (str.length < 1) return; /* expect: prefer-is-empty */
  if (map.length <= 0) return; /* expect: prefer-is-empty */
  if (1 > set.length) return; /* expect: prefer-is-empty */
  if (0 >= names.length) return; /* expect: prefer-is-empty */
}

// A class implementing a core collection still inherits `isEmpty` (from
// Iterable), so the suggestion is valid and the diagnostic keeps firing.
class Bag implements Iterable<int> {
  int get length => 0;
}

// A class whose supertype chain leaves the project is `Unknown`, never
// `ProvenAbsent` — an unresolved ancestor could contribute `isEmpty` — so the
// diagnostic keeps firing (type knowledge suppresses only on positive proof).
class Offsite extends Frobnicator {
  int get length => 0;
}

void typedReceivers(Bag b, Offsite o) {
  if (b.length == 0) return; /* expect: prefer-is-empty */
  if (o.length == 0) return; /* expect: prefer-is-empty */
}

// A static method returns some other type (a Map here), not the enclosing class,
// so `Config.load()` is Unknown, never `Config`. Typing it as `Config` (a named
// constructor guess) would wrongly suppress; the diagnostic must keep firing.
class Config {
  static Map<String, String> load() => {};
}

void staticCallReceiver() {
  if (Config.load().length == 0) return; /* expect: prefer-is-empty */
}
