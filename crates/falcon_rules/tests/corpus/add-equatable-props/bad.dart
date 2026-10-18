import 'support.dart' as fixtures;
import 'support.dart';
class Direct extends Parent {} /* expect: add-equatable-props */
class MixedValue extends Mixed {} /* expect: add-equatable-props */
class Indirect extends Parent { final int child = 1; } /* expect: add-equatable-props */
class Transitive extends Middle { final int child = 1; } /* expect: add-equatable-props */
class PrefixedChild extends fixtures.Parent { final int value = 1; } /* expect: add-equatable-props */
class MixedChild extends Mixed { final int first = 1; final int second = 2; } /* expect: add-equatable-props */
