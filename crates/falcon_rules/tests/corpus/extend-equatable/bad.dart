import 'package:equatable/equatable.dart';
class FirstState {} /* expect: extend-equatable */
class SecondState {} /* expect: extend-equatable */
abstract class ThirdState {} /* expect: extend-equatable */
class FourthState extends Base {} /* expect: extend-equatable */
class FifthState with Other {} /* expect: extend-equatable */
class Base {}
mixin Other {}
