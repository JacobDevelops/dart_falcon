// Test cases for prefer-correct-edge-insets-constructor rule
// Flags EdgeInsets.only() where a simpler constructor should be used

void testSymmetricVertical() {
  final padding = EdgeInsets.only(top: 8, bottom: 8); /* expect: prefer-correct-edge-insets-constructor */
}

void testSymmetricHorizontal() {
  final padding = EdgeInsets.only(left: 4, right: 4); /* expect: prefer-correct-edge-insets-constructor */
}

void testAllEqual() {
  final padding = EdgeInsets.only(left: 4, top: 4, right: 4, bottom: 4); /* expect: prefer-correct-edge-insets-constructor */

  final padding2 = EdgeInsets.only( /* expect: prefer-correct-edge-insets-constructor */
    left: 16,
    top: 16,
    right: 16,
    bottom: 16,
  );
}

void testMultipleViolations() {
  final pad1 = EdgeInsets.only(top: 10, bottom: 10); /* expect: prefer-correct-edge-insets-constructor */
  final pad2 = EdgeInsets.only(left: 8, right: 8); /* expect: prefer-correct-edge-insets-constructor */
  final pad3 = EdgeInsets.only(left: 12, top: 12, right: 12, bottom: 12); /* expect: prefer-correct-edge-insets-constructor */
}

class MyWidget {
  final padding = EdgeInsets.only(top: 20, bottom: 20); /* expect: prefer-correct-edge-insets-constructor */
}
