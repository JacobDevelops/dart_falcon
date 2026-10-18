// @dart = 3.13
import 'support.dart';
class Primary(var int value) extends Parent {} /* expect: avoid-mutable-equatable-subclass-fields */
