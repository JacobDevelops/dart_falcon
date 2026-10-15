import 'base.dart';

void main() {
  Derived().inherited();
}

class Derived extends ImportedBase {
  @override
  void inherited() {}

  void unused() {} /* expect: unreachable-from-main */
}
