// Good examples for prefer-const-border-radius rule
// Using BorderRadius.circular() when appropriate

void testBorderRadiusCircular() {
  final border1 = BorderRadius.circular(8);
  final border2 = BorderRadius.circular(16);
  final border3 = BorderRadius.circular(12);
}

void testBorderRadiusOnlyWithDifferentRadii() {
  final border = BorderRadius.only(
    topLeft: Radius.circular(8),
    topRight: Radius.circular(16),
    bottomLeft: Radius.circular(8),
    bottomRight: Radius.circular(4),
  );
}

void testBorderRadiusByAll() {
  final border = BorderRadius.all(Radius.circular(8));
}

class MyWidget {
  final border1 = BorderRadius.circular(10);
  final border2 = BorderRadius.only(
    topLeft: Radius.circular(8),
    topRight: Radius.circular(12),
  );
  final border3 = BorderRadius.all(Radius.circular(16));
}
