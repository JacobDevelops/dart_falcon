import 'package:equatable/equatable.dart';
import 'support.dart';
class Immutable extends Equatable {
  final int value = 1;
  List<Object?> get props => [value];
}
class Static extends Parent { static int value = 1; }
class LateFinal extends Middle { late final int value; }
class Constants extends Mixed { static const int value = 1; }
class GetterOnly extends Parent { int get value => 1; }
class OwnEquality extends Parent {
  int value = 1;
  bool operator ==(Object other) => identical(this, other);
  int get hashCode => 0;
}
class Unrelated { int value = 1; }
class InterfaceOnly implements Equatable {
  int value = 1;
  List<Object?> get props => [value];
}
