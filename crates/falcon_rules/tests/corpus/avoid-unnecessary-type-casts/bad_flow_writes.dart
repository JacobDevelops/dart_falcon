// With a non-nullable bound, `NonNull(T?)` is `T`, so initializing or assigning
// a `T` promotes the local to `T`; Dart 3.13.5 reports unnecessary_cast.
T nonNullableBoundInitializer<T extends Object>(T value) {
  T? c = value;
  return c as T; /* expect: avoid-unnecessary-type-casts */
}

void assignedNonNull(String? s) {
  s = 'x';
  print(s as String); /* expect: avoid-unnecessary-type-casts */
}

void ifNullAssigned(int? value) {
  value ??= 0;
  print(value as int); /* expect: avoid-unnecessary-type-casts */
}

void compoundAssignmentKeepsPromotion(num n) {
  if (n is int) {
    n += 1;
    print(n as int); /* expect: avoid-unnecessary-type-casts */
  }
}
