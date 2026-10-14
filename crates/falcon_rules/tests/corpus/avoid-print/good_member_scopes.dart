class StaticPrinter {
  static void print(Object? value) {}
  void run() { print('own static method'); }
}

class GetterPrinter {
  void Function(Object?) get print => (value) {};
  void run() { print('own getter'); }
}

class StaticGetterPrinter {
  static void Function(Object?) get print => (value) {};
  void run() { print('own static getter'); }
}

class SetterParameter {
  set output(void Function(Object?) print) { print('setter parameter'); }
}

extension ExtensionPrinter on String {
  void print(Object? value) {}
  void run() { print('own extension member'); }
}

extension on int {
  void print(Object? value) {}
  void run() { print('own anonymous extension member'); }
}
