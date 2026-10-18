import 'package:entity_base/entity_base.dart';
class PackageChild extends Entity {
  int value = 1; /* expect: avoid-mutable-equatable-subclass-fields */
}
