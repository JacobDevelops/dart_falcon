void example(bool left, bool right) {
  final a = left & right; /* expect: avoid-bitwise-operators-with-booleans */
  final b = left | right; /* expect: avoid-bitwise-operators-with-booleans */
  final c = true & false; /* expect: avoid-bitwise-operators-with-booleans */
  final local = true;
  final d = local | right; /* expect: avoid-bitwise-operators-with-booleans */
  final e = (left == right) & (a != b); /* expect: avoid-bitwise-operators-with-booleans */
  final f = (!left) | (!right); /* expect: avoid-bitwise-operators-with-booleans */
}
