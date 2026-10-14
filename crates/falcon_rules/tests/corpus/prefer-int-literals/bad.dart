const double myDouble = 8.0; /* expect: prefer-int-literals */
double? maybe = 1.0; /* expect: prefer-int-literals */

class Box {
  final double width = 2.0; /* expect: prefer-int-literals */
  Box(double size);
  Box.named({required double size});
  double get area => 3.0; /* expect: prefer-int-literals */
  double scale() {
    return 4.0; /* expect: prefer-int-literals */
  }
  double operator +(Box other) => 5.0; /* expect: prefer-int-literals */
}

void takesDouble(double value, {double named = 0}) {}

double negative() => -6.0; /* expect: prefer-int-literals */

void main() {
  double local = 7.0; /* expect: prefer-int-literals */
  var list = <double>[1.0, 2.5]; /* expect: prefer-int-literals */
  takesDouble(9.0); /* expect: prefer-int-literals */
  takesDouble(1, named: 10.0); /* expect: prefer-int-literals */
  Box(11.0); /* expect: prefer-int-literals */
  Box.named(size: 12.0); /* expect: prefer-int-literals */
  double inner() {
    return 1e3; /* expect: prefer-int-literals */
  }
  for (double i = 0.0; i < 1; i++) {} /* expect: prefer-int-literals */
  print([local, list, inner]);
}
