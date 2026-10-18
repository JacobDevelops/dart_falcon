import 'package:entity_base/entity_base.dart';
class PackageChild extends Entity {
  final int value = 1;
  List<Object?> get props => []; /* expect: list-all-equatable-fields */
}
