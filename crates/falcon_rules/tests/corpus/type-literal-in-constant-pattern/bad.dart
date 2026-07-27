class LocalType {}

void check(Object? value) {
  if (value case int) { /* expect: type-literal-in-constant-pattern */
    print('int');
  }
  if (value case LocalType) { /* expect: type-literal-in-constant-pattern */
    print('local');
  }
  switch (value) {
    case String: /* expect: type-literal-in-constant-pattern */
      break;
    default:
      break;
  }
}
