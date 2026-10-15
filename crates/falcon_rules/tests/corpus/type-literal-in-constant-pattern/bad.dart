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

void dynamicTypePattern(Object? value) {
  switch (value) {
    case dynamic: /* expect: type-literal-in-constant-pattern */
      break;
    default:
      break;
  }
}

void dynamicNestedPatterns(Object? value) {
  if (value case dynamic) {} /* expect: type-literal-in-constant-pattern */
  if (value case [dynamic]) {} /* expect: type-literal-in-constant-pattern */
  final result = switch (value) {
    dynamic => true, /* expect: type-literal-in-constant-pattern */
    _ => false,
  };
  print(result);
}
