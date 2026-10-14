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
