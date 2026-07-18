import 'dart:collection';

void good() {
  var a = [];
  var b = {};
  var c = <int>[];
  var d = <String, int>{};
  var e = List.filled(3, 0);
  var f = Map.from(<int, int>{});
  var g = Set.of([1, 2, 3]);
  var h = List.generate(3, (i) => i);
}

// A `{}`/`{}` literal has static type `Map`/`Set`, which is not assignable back
// to the concrete `LinkedHashMap`/`LinkedHashSet`, so these constructors are
// REQUIRED by their declared context type and the diagnostic is SUPPRESSED. This
// holds for local variables, fields, and return types.
// (This suppression is syntactic and does not need a type index, but the file as
// a whole exercises the user-shadow suppression below, which does.)
LinkedHashMap<int, int> makeMap() => LinkedHashMap();

class Holder {
  LinkedHashSet<int> data = LinkedHashSet();

  LinkedHashMap<int, int> build() {
    LinkedHashMap<int, int> local = LinkedHashMap();
    return local;
  }
}

// A user-declared `Bag` named `Set` shadows core `Set`. Its `type_kind` is
// `Some` (user declarations carry one; core names do not), so `Set()` builds the
// user's type and a `{}` literal would build the wrong one — SUPPRESSED.
// (Requires the corpus harness to attach a TypeIndex for this rule; without one
// the name is Unknown and this line would fire.)
class Set {}

void shadowed() {
  var s = Set();
}
