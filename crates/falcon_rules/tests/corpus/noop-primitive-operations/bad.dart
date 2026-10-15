void ints(int i) {
  i.toInt(); /* expect: noop-primitive-operations */
  i.round(); /* expect: noop-primitive-operations */
  i.ceil(); /* expect: noop-primitive-operations */
  i.floor(); /* expect: noop-primitive-operations */
  i.truncate(); /* expect: noop-primitive-operations */
}

void doubles(double d) {
  d.toDouble(); /* expect: noop-primitive-operations */
}

void strings(String s, Object o) {
  s.toString(); /* expect: noop-primitive-operations */
  print('value: ${o.toString()}'); /* expect: noop-primitive-operations */
  print(o.toString()); /* expect: noop-primitive-operations */
  final joined = 'a' '' 'b'; /* expect: noop-primitive-operations */
  print(joined);
}

class Stepper {
  Stepper(this.value);
  final double? value;
  static const double _min = 0;
  static const double _max = 24;

  // `??` and `clamp` keep the operands' `double` type.
  void step() {
    final current = value ?? _min;
    print(current.clamp(_min, _max).toDouble()); /* expect: noop-primitive-operations */
    print((current + 0.5).toDouble()); /* expect: noop-primitive-operations */
  }
}

void clamps(int i, double d) {
  i.clamp(0, 1).toInt(); /* expect: noop-primitive-operations */
  d.clamp(0.0, 1.0).toDouble(); /* expect: noop-primitive-operations */
}
