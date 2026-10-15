void inspect(Object value, Iterable<Object> values) {
  for (final item in values) {
    if (item is double) {} else if (item is int) {} /* expect: avoid-double-and-int-checks */
  }
  if (value case final local) {
    if (local is double) {} else if (local is int) {} /* expect: avoid-double-and-int-checks */
  }
  values.map((item) {
    if (item is double) {} else if (item is int) {} /* expect: avoid-double-and-int-checks */
    return item;
  });
}

class Holder {
  Object get item => 0;
  void inspect() {
    if (item is double) {} else if (item is int) {}
  }
}
