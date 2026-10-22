class Flag {
  const Flag(this.value);
  final bool value;
  static const yes = Flag(true);
}

// The defaults() and patterns() examples intentionally omit required `const`.
// They are not implicit const contexts; Dart's analyzer diagnoses them separately.
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

// Explicit constant patterns do establish constant contexts for their operands.
void constantPatterns(Object v) {
  switch (v) {
    case const (Flag(true)): /* expect: use-named-constants */
      break;
    default:
      break;
  }
  if (v case const (Flag(true))) { /* expect: use-named-constants */
    print(v);
  }
}
