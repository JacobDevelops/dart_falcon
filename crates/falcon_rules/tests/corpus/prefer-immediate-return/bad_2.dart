// Bad: intermediate variable before return inside a switch case.
String describe(int code) {
  switch (code) {
    case 1:
      final label = compute(); /* expect: prefer-immediate-return */
      return label;
    default:
      return 'other';
  }
}

// Bad: intermediate variable before return inside a local function.
int outer() {
  int helper() {
    final v = compute(); /* expect: prefer-immediate-return */
    return v;
  }
  return helper();
}
