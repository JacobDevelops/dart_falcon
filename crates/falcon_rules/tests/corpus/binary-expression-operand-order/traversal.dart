@Example(0 == value) /* expect: binary-expression-operand-order */
class Initializers {
  final field = 0 == value; /* expect: binary-expression-operand-order */
  final bool valid;

  Initializers() : valid = 1 < value; /* expect: binary-expression-operand-order */

  bool operator +(Object other) => 2 <= value; /* expect: binary-expression-operand-order */
}

void defaults([bool valid = 0 == 1]) {} /* expect: binary-expression-operand-order */

void expressions() {
  final list = [0 == value]; /* expect: binary-expression-operand-order */
  final set = {1 < value}; /* expect: binary-expression-operand-order */
  final map = {'valid': 2 <= value}; /* expect: binary-expression-operand-order */
  final record = (3 > value,); /* expect: binary-expression-operand-order */
  final negated = !(4 >= value); /* expect: binary-expression-operand-order */
  final indexed = values[5 != value]; /* expect: binary-expression-operand-order */
  final constructed = new Example(6 == value); /* expect: binary-expression-operand-order */
  final interpolated = '${7 < value}'; /* expect: binary-expression-operand-order */
  for (valid = 8 < value; ready; ) {} /* expect: binary-expression-operand-order */
  for (final valid in [9 < value]) {} /* expect: binary-expression-operand-order */
  switch (value) {
    case 0:
      consume(10 < value); /* expect: binary-expression-operand-order */
  }
}

Iterable<bool> comparisons() sync* {
  yield 0 == value; /* expect: binary-expression-operand-order */
}
