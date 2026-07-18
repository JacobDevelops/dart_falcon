void bad() {
  var a = items.where((e) => e is String); /* expect: prefer-iterable-where-type */
  var b = list.where((x) => x is int); /* expect: prefer-iterable-where-type */
  var c = things.where((t) => t is Widget); /* expect: prefer-iterable-where-type */
  var d = values.where((v) => v is double).toList(); /* expect: prefer-iterable-where-type */
  var e = data.where((item) => item is Map); /* expect: prefer-iterable-where-type */
}

// A `List` receiver IS an Iterable (`is_subtype(List, Iterable)` is `Yes`, not
// `ProvenNo`), so `whereType<T>()` applies and the diagnostic keeps firing even
// with type knowledge attached.
void iterableReceiver(List<int> xs) {
  xs.where((e) => e is int); /* expect: prefer-iterable-where-type */
}

// `Custom` is not an Iterable, but it carries its own `whereType` — so
// `member_lookup` is `Found`, never `ProvenAbsent`, and the rewrite is valid.
// The diagnostic keeps firing (type knowledge suppresses only on positive proof
// that `whereType` is absent).
class Custom {
  Custom where(bool Function(dynamic) test) => this;
  Iterable<T> whereType<T>() => throw '';

  void run() {
    this.where((e) => e is String); /* expect: prefer-iterable-where-type */
  }
}
