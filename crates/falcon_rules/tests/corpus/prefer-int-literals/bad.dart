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

// Field formals resolve even when their fields follow a named constructor.
class Insets {
  const Insets.fromSTEB(this.start, this.top, this.end, this.bottom);
  final double start, top, end, bottom;
}

class Matrix {
  void setEntry(int row, int column, double value) {}
}

void constructorAndCascadeArguments(Matrix matrix) {
  const Insets.fromSTEB(12.0, 4.0, 12.0, 4.0); /* expect: prefer-int-literals */ /* expect: prefer-int-literals */ /* expect: prefer-int-literals */ /* expect: prefer-int-literals */
  matrix
    ..setEntry(3, 2, -1.0) /* expect: prefer-int-literals */
    ..setEntry(0, 0, 2.0); /* expect: prefer-int-literals */
}

// Super/redirect initializer arguments use the same resolved contexts as calls.
class NumericBase {
  final double pressure, elevation;
  const NumericBase({required this.pressure, this.elevation = 0});
  const NumericBase.named(this.pressure, {this.elevation = 0});
}

class NumericChild extends NumericBase {
  const NumericChild() : super(pressure: 0.0, elevation: 8.0); /* expect: prefer-int-literals */ /* expect: prefer-int-literals */
  const NumericChild.named() : super.named(-2.0, elevation: 4.0); /* expect: prefer-int-literals */ /* expect: prefer-int-literals */
  const NumericChild.value(double value) : super(pressure: value);
  const NumericChild.redirect() : this.value(9.0); /* expect: prefer-int-literals */
}

class GenericNumericBase<T> {
  final T value;
  const GenericNumericBase(this.value);
  const GenericNumericBase.named({required this.value});
}

class GenericNumericChild extends GenericNumericBase<double> {
  const GenericNumericChild() : super(5.0); /* expect: prefer-int-literals */
  const GenericNumericChild.named() : super.named(value: 6.0); /* expect: prefer-int-literals */
}

class InitializedNumericChild extends NumericBase {
  final int marker;
  const InitializedNumericChild() : marker = 0, super(pressure: 10.0); /* expect: prefer-int-literals */
}
