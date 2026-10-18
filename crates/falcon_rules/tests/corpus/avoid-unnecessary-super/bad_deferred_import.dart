import 'dart:developer' deferred as developer;

class Base {
  void log(String message) {}
}
final class Child extends Base {
  void run() => super.log('x'); /* expect: avoid-unnecessary-super */
}
