import 'package:equatable/equatable.dart';
import 'package:equatable/equatable.dart' as eq;
import 'support.dart';
class Direct extends Equatable {
  int value = 1; /* expect: avoid-mutable-equatable-subclass-fields */
  List<Object?> get props => [value];
}
class MixedValue with EquatableMixin {
  int value = 1; /* expect: avoid-mutable-equatable-subclass-fields */
  List<Object?> get props => [value];
}
class Indirect extends Middle {
  int child = 1; /* expect: avoid-mutable-equatable-subclass-fields */
}
class Prefixed extends eq.Equatable {
  late int value; /* expect: avoid-mutable-equatable-subclass-fields */
  List<Object?> get props => [value];
}
class Private extends Mixed {
  int _value = 1; /* expect: avoid-mutable-equatable-subclass-fields */
}
