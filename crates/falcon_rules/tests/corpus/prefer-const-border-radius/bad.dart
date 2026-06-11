// Test cases for prefer-const-border-radius rule
// Flags BorderRadius.only(...) where all radii are equal

void testAllRadiiEqual() {
  final border1 = BorderRadius.only(
    topLeft: Radius.circular(8),
    topRight: Radius.circular(8),
    bottomLeft: Radius.circular(8),
    bottomRight: Radius.circular(8),
  ); /* expect: prefer-const-border-radius */

  final border2 = BorderRadius.only(
    topLeft: Radius.circular(16),
    topRight: Radius.circular(16),
    bottomLeft: Radius.circular(16),
    bottomRight: Radius.circular(16),
  ); /* expect: prefer-const-border-radius */
}

void testInlineAllRadiiEqual() {
  final border = BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10)); /* expect: prefer-const-border-radius */
}

class MyWidget {
  final border = BorderRadius.only( /* expect: prefer-const-border-radius */
    topLeft: Radius.circular(12),
    topRight: Radius.circular(12),
    bottomLeft: Radius.circular(12),
    bottomRight: Radius.circular(12),
  );
}

void testSmallRadius() {
  final border = BorderRadius.only(topLeft: Radius.circular(4), topRight: Radius.circular(4), bottomLeft: Radius.circular(4), bottomRight: Radius.circular(4)); /* expect: prefer-const-border-radius */
}
