import 'package:entity_base/entity_base.dart';
import 'support.dart';
abstract class AbstractField extends Parent { abstract int value; }
class ExternalField extends Parent { external int value; }
class CustomEquality extends Parent {
  bool operator ==(Object other) => identical(this, other);
  int get hashCode => 0;
}
class CustomChild extends CustomEquality { int value = 1; }
class PackageImmutable extends Entity { final int value = 1; }
