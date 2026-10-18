// @dart=2.19

// Switch expressions are unavailable before Dart 3.
int legacy(int value) {
  switch (value) {
    case 1:
      return 1;
    default:
      return 0;
  }
}
