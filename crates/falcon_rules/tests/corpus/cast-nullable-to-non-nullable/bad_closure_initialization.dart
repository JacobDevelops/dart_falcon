// The analyzer reports the final nullable cast, but not the promoted const cast.
void explicitlyTypedFinalAndConst() {
  final String? name = 'x';
  print(name as String); /* expect: cast-nullable-to-non-nullable */
  const int? count = 1;
  print(count as int);
  String? mutable = 'x';
  print(mutable as String);
  final inferred = 'x';
  print(inferred as String);
}

// A later sibling closure and a closure's own write both prevent promotion.
void laterWritingClosure(int? value, List<Function> callbacks) {
  callbacks.add(() {
    if (value != null) print(value as int); /* expect: cast-nullable-to-non-nullable */
  });
  callbacks.add(() => value = null);
}

void selfWritingClosure(int? value, List<Function> callbacks) {
  callbacks.add(() {
    if (value != null) print(value as int); /* expect: cast-nullable-to-non-nullable */
    value = null;
  });
}
