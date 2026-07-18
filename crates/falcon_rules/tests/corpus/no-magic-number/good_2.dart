// Non-magic counterparts of bad_2.dart — must not fire once the literals are
// named constants.
const kAnswer = 42;
const kLimit = 500;
const kX = 3;
const kY = 4;
const kFallback = 99;

void cascade(StringBuffer b) {
  b
    ..writeln('x')
    ..write(kAnswer);
}

(int, int) pointRecord() {
  return (kX, kY);
}

int classify(int x) {
  return switch (x) {
    _ => kFallback,
  };
}

void check(int n) {
  assert(n < kLimit);
}
