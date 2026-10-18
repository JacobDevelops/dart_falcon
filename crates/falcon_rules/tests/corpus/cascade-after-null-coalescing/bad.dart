class Box {
  int value = 0;
  void reset() { value = 0; }
}

Box choose(Box? first, Box second) => first ?? second..reset(); /* expect: cascade-after-null-coalescing */

void cascades(Box? first, Box? middle, Box second) {
  final assigned = first ?? second..value = 1; /* expect: cascade-after-null-coalescing */
  first ?? second..reset()..value = 2; /* expect: cascade-after-null-coalescing */
  final nested = first ?? middle ?? second..reset(); /* expect: cascade-after-null-coalescing */
  final list = [first ?? second..reset()]; /* expect: cascade-after-null-coalescing */
  final wrapped = (first ?? second..reset()); /* expect: cascade-after-null-coalescing */
  final nullable = first ?? middle?..reset(); /* expect: cascade-after-null-coalescing */
  final operandOnly = first ?? (second)..reset(); /* expect: cascade-after-null-coalescing */
  final ternary = first == null ? second : middle ?? second..reset(); /* expect: cascade-after-null-coalescing */
  final deepTernary = first == null ? second : middle == null ? second : first ?? second..reset(); /* expect: cascade-after-null-coalescing */
}
