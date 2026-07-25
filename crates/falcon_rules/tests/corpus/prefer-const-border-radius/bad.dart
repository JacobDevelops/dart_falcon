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

// Regression: the violation must still be found inside Dart 3 containers
// (pattern declaration, pattern assignment, labeled statement, switch
// expression, collection if/spread, record field, assert).
void containersRegression(int rcount) {
  final (ra, _) = (BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10)), 0); /* expect: prefer-const-border-radius */
  lbl: {
    final rb = BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10)); /* expect: prefer-const-border-radius */
    print(rb);
  }
  final rc = switch (rcount) {
    0 => BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10)), /* expect: prefer-const-border-radius */
    _ => null,
  };
  final rd = switch (BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10))) { /* expect: prefer-const-border-radius */
    _ => 0,
  };
  final re = [if (rcount > 0) BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10))]; /* expect: prefer-const-border-radius */
  final rf = [...[BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10))]]; /* expect: prefer-const-border-radius */
  final rg = (p: BorderRadius.only(topLeft: Radius.circular(10), topRight: Radius.circular(10), bottomLeft: Radius.circular(10), bottomRight: Radius.circular(10)), q: 0); /* expect: prefer-const-border-radius */
  print([ra, rc, rd, re, rf, rg]);
}
