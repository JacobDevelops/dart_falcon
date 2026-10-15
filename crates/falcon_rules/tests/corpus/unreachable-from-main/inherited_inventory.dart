import 'dart:collection';

void main() {
  print(InheritedList);
  print(LocalView);
  print(1.usedExtension());
}

abstract class InheritedList extends ListBase<int> {
  int unused() => 1; /* expect: unreachable-from-main */
  bool get isNotEmpty => true;
}

class LocalView {}

extension UsedExtension on int {
  int usedExtension() => this;
  int unusedExtension() => this; /* expect: unreachable-from-main */
}

extension UnusedExtension on LocalView { /* expect: unreachable-from-main */
  int unusedMember() => 1;
}
