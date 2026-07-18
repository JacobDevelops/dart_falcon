// Additional avoid-unused-parameters coverage: member methods on mixins and
// enhanced enums — declaration kinds folded in from the pyramid twin.

mixin Logging {
  void log(String message, String unused) { /* expect: avoid-unused-parameters */
    print(message);
  }
}

enum Level {
  low,
  high;

  bool exceeds(int threshold, int unused) { /* expect: avoid-unused-parameters */
    return index > threshold;
  }
}
