// Dart 3.13.5 flow analysis does not promote `c` here: with an unbounded `T`,
// `NonNull(T?)` is `T & Object`, and `value` (a `T`) is not a subtype of it.
T nullableBoundInitializer<T>(T value) {
  T? c = value;
  return c as T;
}

T nullableBoundAssignment<T extends Object?>(T value, T? other) {
  other = value;
  return other as T;
}
