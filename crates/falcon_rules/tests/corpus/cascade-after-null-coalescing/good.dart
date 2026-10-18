class Box {
  int value = 0;
  void reset() { value = 0; }
}

Box choose(Box? first, Box second) => (first ?? second)..reset();

void cascades(Box? first, Box? middle, Box second, bool condition) {
  final explicitReceiver = (first ?? second)..value = 1;
  final explicitFallback = first ?? (second..reset());
  final multiple = (first ?? second)..reset()..value = 2;
  final nestedReceiver = ((first ?? middle) ?? second)..reset();
  final nestedFallback = first ?? (middle ?? (second..reset()));
  final ordinary = second..reset();
  final nullAware = (first ?? middle)?..reset();
  final conditional = (condition ? first : second)?..reset();
  final methodCall = (first ?? second).reset();
  final ternaryReceiver = (condition ? first : middle ?? second)..reset();
  final ternaryFallback = condition ? second : (middle ?? (second..reset()));
  final parenthesizedElse = condition ? second : (first ?? second)..reset();
  final coalescingThen = condition ? first ?? second : second..reset();
}
