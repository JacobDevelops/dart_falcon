int handle(int x) => switch (x) {
  0 => switch (x + 1) { 1 => 2, _ => 3 }, /* expect: avoid-nested-switch-expressions */
  _ => 4,
};
