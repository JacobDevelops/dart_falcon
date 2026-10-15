// Dart 3.13.5 `dart analyze` reports no cast_nullable_to_non_nullable here: a
// non-null write promotes a nullable local to a non-null type of interest.
void assignedNonNull(String? s) {
  s = 'x';
  print(s as String);
}

void assignedObject(Object? o) {
  o = 'a';
  print(o as String);
}

void ifNullAssigned(int? value) {
  value ??= 0;
  print(value as int);
}

void initializedNonNull() {
  int? value = 1;
  value = 2;
  print(value as int);
}
