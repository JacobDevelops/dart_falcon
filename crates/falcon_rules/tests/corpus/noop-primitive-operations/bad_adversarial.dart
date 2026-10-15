import 'dart:core' as core;
import 'dart:core';

// Analyzer-confirmed AdvB3 cascade and prefixed-print regressions.
void cascade(String s) {
  s..toString(); /* expect: noop-primitive-operations */
}

void prefixedPrint(Object o) {
  core.print(o.toString()); /* expect: noop-primitive-operations */
}

// Analyzer-confirmed AdvB3 receiver inference regressions.
void genericMember(List<int> l) {
  l.first.round(); /* expect: noop-primitive-operations */
}


var mapped = <int>[].map((x) => x.round()); /* expect: noop-primitive-operations */

void indexed(List<int> l, String s) {
  l[0].round(); /* expect: noop-primitive-operations */
}

