// Positive cases live in project/lib: the official lint only runs in a
// package's lib/ directory.
abstract class Base { void method(int value) {} }
class Child extends Base {
  void method(int renamed) {}
}
