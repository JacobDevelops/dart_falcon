import 'support.dart';
class OwnEquality extends Parent {
  int value = 1;
  bool operator ==(Object other) => other is OwnEquality && value == other.value;
  int get hashCode => value.hashCode;
}
class InheritedCustomEquality extends OwnEquality { int child = 2; }
