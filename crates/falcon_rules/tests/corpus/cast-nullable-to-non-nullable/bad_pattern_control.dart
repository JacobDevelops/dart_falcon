class Box {
  final int? v;
  Box(this.v);
}

// Object-pattern shorthand assigns the existing variable.
void shorthandAssignment(int? v, Box box, bool refresh) {
  if (v == null) return;
  if (refresh) { Box(:v) = box; }
  print(v as int); /* expect: cast-nullable-to-non-nullable */
}

// Earlier failed null matches promote the later switch scrutinee.
int failedNullExpression(String? s) => switch (s) {
  null => 0,
  _ => (s as String).length,
};
void failedNullStatement(String? s) {
  switch (s) {
    case null:
      return;
    default:
      print(s as String);
  }
  print(s as String);
}

// A guard failure must retain the nullable alternative.
void guardedNull(int? x, bool accept) {
  if (x case null when accept) return;
  print(x as int); /* expect: cast-nullable-to-non-nullable */
}

void ifNullExit(int? x) {
  if (x case null) return;
  print(x as int);
}
