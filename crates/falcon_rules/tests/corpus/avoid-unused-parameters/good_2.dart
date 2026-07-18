// Non-violating counterparts of bad_2.dart — every parameter is used.

mixin Logging {
  void log(String message) {
    print(message);
  }
}

enum Level {
  low,
  high;

  bool exceeds(int threshold) {
    return index > threshold;
  }
}

mixin class Tracker {
  void record(int value) {
    print(value);
  }
}

extension NumberChecks on int {
  bool over(int limit) {
    return this > limit;
  }
}

extension type Meters(int value) {
  bool exceeds(int limit) {
    return value > limit;
  }
}
