int compute(int value) => switch (value) {
  0 => (() => switch (value) { 1 => 2, _ => 3 })(),
  _ => 4,
};
