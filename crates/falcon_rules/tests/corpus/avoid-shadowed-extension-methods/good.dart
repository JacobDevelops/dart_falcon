class Target {
  void run() {}
  static void staticOnly() {}
}

class Unrelated {
  void differentOwner() {}
}

extension Renamed on Target {
  void customRun() {}
  void differentOwner() {}
  void staticOnly() {}
  static void run() {}
}

extension NullableTarget on Target? {
  void run() {}
}

extension DynamicTarget on dynamic {
  void run() {}
}

extension First on Target {
  void extensionOnly() {}
}

extension Second on Target {
  void extensionOnly() {}
}

extension Unbounded<T> on T {
  void run() {}
}

class OrdinarySubclass extends Target {
  void run() {}
}

extension StaticIntMember on int {
  void parse() {}
}

extension StaticBigIntMember on BigInt {
  void two() {}
}

extension StaticDurationMember on Duration {
  void zero() {}
}
