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

mixin class Tracker {
  void record(int value, int unused) { /* expect: avoid-unused-parameters */
    print(value);
  }
}

extension NumberChecks on int {
  bool over(int limit, int unused) { /* expect: avoid-unused-parameters */
    return this > limit;
  }
}

extension type Meters(int value) {
  bool exceeds(int limit, int unused) { /* expect: avoid-unused-parameters */
    return value > limit;
  }
}
