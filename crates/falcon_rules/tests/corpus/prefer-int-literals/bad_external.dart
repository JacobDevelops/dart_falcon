import 'package:duplicate/barrel.dart';
import 'support/vector.dart' as support;

void main() {
  Vector(1.0); /* expect: prefer-int-literals */
  Vector(1.5);
  support.LocalVector(1.0); /* expect: prefer-int-literals */
  support.LocalVector(1.5);
}
