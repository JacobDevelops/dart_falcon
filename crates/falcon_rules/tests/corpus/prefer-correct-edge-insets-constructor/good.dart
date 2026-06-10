// Good examples for prefer-correct-edge-insets-constructor rule
// Using simpler constructors when appropriate

void testSymmetricConstructor() {
  final padding1 = EdgeInsets.symmetric(vertical: 8);
  final padding2 = EdgeInsets.symmetric(horizontal: 4);
  final padding3 = EdgeInsets.symmetric(vertical: 10, horizontal: 16);
}

void testAllConstructor() {
  final padding = EdgeInsets.all(4);
  final padding2 = EdgeInsets.all(16);
}

void testOnlyWithAsymmetricValues() {
  final padding = EdgeInsets.only(
    top: 8,
    bottom: 12,
    left: 4,
    right: 6,
  );

  final padding2 = EdgeInsets.only(top: 10, left: 5);
}

void testZeroConstructors() {
  final padding1 = EdgeInsets.zero;
  final padding2 = EdgeInsets.only(left: 8, right: 16);
}
