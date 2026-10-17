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

void restoreExemptions(List<int> values) {
  print(const [23]);
  print(24); /* expect: no-magic-number */
  print(values[25]);
  print(26); /* expect: no-magic-number */
  print(DateTime(2024, 2, 3));
  print(27); /* expect: no-magic-number */
  print([28, compute(29)]); /* expect: no-magic-number */
  values..[30] = 31; /* expect: no-magic-number */
}
