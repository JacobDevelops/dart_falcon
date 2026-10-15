// Dart 3.13.5 `dart analyze` reports no cast_nullable_to_non_nullable here:
// a promotion holds in a branch the null check cannot reach, and a promotion
// made after a reassignment survives the join.
void promotedElseBranch(int? value) {
  if (value == null) return;
  if (value != null) {
    print(value);
  } else {
    value as int;
  }
}

void repromotedAfterJoin(int? value, int? replacement) {
  if (value == null) return;
  if (value != null) {
    value = replacement;
    value!;
  }
  value as int;
}
