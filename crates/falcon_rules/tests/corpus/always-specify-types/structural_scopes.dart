class Box<T> {}

void scopes(Object value) {
  void local<Box>(Box item) {
    Box copy = item;
    (copy,) = (item,);
    if (item case final afterAssignment) {} /* expect: always-specify-types */
    (copy,) = (() {
      if (item case final insideValue) {} /* expect: always-specify-types */
      return item;
    }(),);
    if (item case final afterValue) {} /* expect: always-specify-types */
  }

  dynamic callback = <Box>(Box item) => item;
  Box? afterLocal; /* expect: always-specify-types */
  bool tested = value is Box;
  print(<Object?>[callback, afterLocal, tested, local]);
}
