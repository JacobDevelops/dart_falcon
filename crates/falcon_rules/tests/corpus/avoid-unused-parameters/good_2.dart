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
