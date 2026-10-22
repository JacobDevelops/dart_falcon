class Flag {
  const Flag(this.value);
  final bool value;
  static const yes = Flag(true);
}

// These intentionally invalid non-constant expressions are not implicit const
// contexts. This lint must not mistake the constant-expression requirement for
// implicit const; Dart's analyzer diagnoses the missing `const` separately.
void defaults([Flag value = Flag(true)]) {}

void patterns(Object value) {
  switch (value) {
    case {Flag(true): _}:
      break;
    case == Flag(true):
      break;
    default:
      break;
  }
}
