class Box {
  final int? v;
  Box(this.v);
}

void shorthandAssignment(int? v, Box box, bool refresh) {
  if (v == null) return;
  if (refresh) { Box(:v) = box; }
  print(v as int);
}

int failedNullExpression(String? s) => switch (s) {
  null => 0,
  _ => (s as String).length, /* expect: avoid-unnecessary-type-casts */
};
void failedNullStatement(String? s) {
  switch (s) {
    case null:
      return;
    default:
      print(s as String); /* expect: avoid-unnecessary-type-casts */
  }
  print(s as String); /* expect: avoid-unnecessary-type-casts */
}

void ifNullExit(int? x) {
  if (x case null) return;
  print(x as int); /* expect: avoid-unnecessary-type-casts */
}
