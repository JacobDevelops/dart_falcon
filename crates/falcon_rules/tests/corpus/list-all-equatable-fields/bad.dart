import 'package:equatable/equatable.dart';
import 'package:equatable/equatable.dart' as eq;
import 'support.dart';
class Direct extends Equatable {
  final int value = 1;
  List<Object?> get props => []; /* expect: list-all-equatable-fields */
}
class MixedValue with EquatableMixin {
  final int value = 1;
  List<Object?> get props => []; /* expect: list-all-equatable-fields */
}
class Indirect extends Middle {
  final int child = 1;
  List<Object?> get props => [...super.props]; /* expect: list-all-equatable-fields */
}
class Prefixed extends eq.Equatable {
  final int x = 1, y = 2;
  List<Object?> get props => [x]; /* expect: list-all-equatable-fields */
}
class Private extends Mixed {
  final int _value = 1;
  List<Object?> get props { return []; } /* expect: list-all-equatable-fields */
}
class Cascaded extends Parent {
  final int value = 1;
  List<Object?> get props => super.props..addAll([]); /* expect: list-all-equatable-fields */
}
