import 'dart:developer' as action;

class Base {
  void action() {}
  void print(String message) {}
}
final class Child extends Base {
  void importedPrefix() => super.action();
  void implicitCore() => super.print('x');
}
