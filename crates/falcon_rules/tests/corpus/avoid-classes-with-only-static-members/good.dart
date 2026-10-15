// A group of constants is allowed.
class Color {
  static const red = '#f00';
  static const green = '#0f0';
  static const blue = '#00f';
}

// Instance members make it a real class.
class Point {
  final int x;
  Point(this.x);
  static Point origin() => Point(0);
}

// Sealed classes are exempt.
sealed class Shape {
  static Shape? parse(String text) => null;
}

// A named constructor is not a default constructor.
class Logger {
  Logger.named();
  static void log(String message) {}
}

// A supertype other than Object may contribute instance members.
class Base {
  void run() {}
}

class Derived extends Base {
  static void helper() {}
}


class Impl implements Base {
  @override
  void run() {}
  static void helper() {}
}

// An empty class has no members at all.
class Marker {}

// Top-level functions instead of a static-only class.
DateTime mostRecent(List<DateTime> dates) => dates.first;

// A mixin class's instance members also provide a real interface.
mixin class InstanceMixinClass {
  void run() {}
}

class InheritsMixinClass extends InstanceMixinClass {
  static void helper() {}
}

// One surviving class-owned member still exempts the class.
class TwoMethods {
  void first() {}
  void second() {}
}
mixin ReplacesFirst {
  void first() {}
}
class PartiallyReplaced extends TwoMethods with ReplacesFirst {
  static void helper() {}
}

// Getter and setter ownership must remain separate.
class AccessorParent {
  int get value => 0;
  set value(int value) {}
}
mixin ReplacesGetter {
  int get value => 1;
}
class RetainsSetter extends AccessorParent with ReplacesGetter {
  static void helper() {}
}
