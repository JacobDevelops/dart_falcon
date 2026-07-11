// Additional no-magic-number coverage folded in from the pyramid twin: magic
// numbers reached through cascades, records, switch expressions, and asserts.

void cascade(StringBuffer b) {
  b
    ..writeln('x')
    ..write(42); /* expect: no-magic-number */
}

(int, int) pointRecord() {
  return (3, 4); /* expect: no-magic-number */ /* expect: no-magic-number */
}

int classify(int x) {
  return switch (x) {
    _ => 99, /* expect: no-magic-number */
  };
}

void check(int n) {
  assert(n < 500); /* expect: no-magic-number */
}
