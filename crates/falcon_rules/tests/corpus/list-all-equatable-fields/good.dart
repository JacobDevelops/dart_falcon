import 'package:equatable/equatable.dart';
import 'support.dart';
class Complete extends Equatable {
  final int value = 1;
  List<Object?> get props => [value];
}
class ExplicitThis with EquatableMixin {
  final int value = 1;
  List<Object?> get props { return [this.value]; }
}
class StaticOnly extends Parent {
  static int ignored = 1;
  int get computed => 2;
  List<Object?> get props => [...super.props];
}
class UnknownCall extends Parent {
  final int value = 1;
  List<Object?> values() => [value];
  List<Object?> get props => values();
}
class UnknownSpread extends Parent {
  final int value = 1;
  List<Object?> values() => [value];
  List<Object?> get props => [...values()];
}
class Conditional extends Parent {
  final int value = 1;
  List<Object?> get props => [if (true) value];
}
class CompleteChild extends Middle {
  final int child = 1;
  List<Object?> get props => [...super.props, child];
}
class Unrelated {
  final int value = 1;
  List<Object?> get props => [];
}
class Cascaded extends Parent {
  final int value = 1;
  List<Object?> get props => super.props..addAll([value]);
}
class UnknownCascade extends Parent {
  final int value = 1;
  List<Object?> values() => [value];
  List<Object?> get props => super.props..addAll(values());
}
