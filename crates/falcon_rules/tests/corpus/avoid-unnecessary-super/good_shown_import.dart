import 'dart:developer' show log;

class Base {
  void log(String message) {}
}
final class Child extends Base {
  void run() => super.log('x');
}
final class OwnOverride extends Base {
  @override
  void log(String message) {}
  void run() => super.log('x');
}
