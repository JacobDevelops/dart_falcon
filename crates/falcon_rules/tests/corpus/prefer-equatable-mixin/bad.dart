import 'package:equatable/equatable.dart';
import 'package:equatable/equatable.dart' as eq;
class First extends Equatable {} /* expect: prefer-equatable-mixin */
class Second extends eq.Equatable {} /* expect: prefer-equatable-mixin */
abstract class Third extends Equatable {} /* expect: prefer-equatable-mixin */
class Fourth extends Equatable {} /* expect: prefer-equatable-mixin */
class Fifth extends eq.Equatable {} /* expect: prefer-equatable-mixin */
