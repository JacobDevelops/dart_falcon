import 'dart:developer' hide log;

class Base {
  void log(String message) {}
}
final class Child extends Base {
  void run() => super.log('x'); /* expect: avoid-unnecessary-super */
}
