const empty = null;
Object? arrow() => null; /* expect: function-always-returns-null */
Object? direct() { return null; } /* expect: function-always-returns-null */
Object? both(bool flag) { /* expect: function-always-returns-null */
  if (flag) { return null; } else { return null; }
}
Object? trailing(bool flag) { /* expect: function-always-returns-null */
  if (flag) return null;
  return null;
}
Object? lexical() { /* expect: function-always-returns-null */
  return empty;
}
Object? nested(bool flag) { /* expect: function-always-returns-null */
  Object inner() => 4;
  if (flag) { if (flag) return null; else return null; }
  return null;
}
class Example {
  Object? method() => null; /* expect: function-always-returns-null */
}

Object? constListEquality() => const <int>[] == const <int>[] ? null : 1; /* expect: function-always-returns-null */
