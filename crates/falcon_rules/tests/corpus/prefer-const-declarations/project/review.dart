import 'constants.dart' deferred as lazy;
import 'user_identical.dart';
import 'constants.dart' as constants;

const num number = 1;
const Object text = 'a';
const double doubleValue = 1;

final narrowedNumber = number as int; /* expect: prefer-const-declarations */
final narrowedText = text as String; /* expect: prefer-const-declarations */
final coercedDouble = doubleValue as double; /* expect: prefer-const-declarations */
final importedDouble = constants.doubleValue as double; /* expect: prefer-const-declarations */
final nanRemainder = 5 % 0.0; /* expect: prefer-const-declarations */
final nullObject = null as Object;
final intList = 1 as List;
final nullable = null as int?; /* expect: prefer-const-declarations */
final deferredType = lazy.OC;
final deferredFunction = lazy.otherFn;
final deferredStatic = lazy.OC.sc;
final userCall = identical(1, 2);

void generic<T>() {
  final test = 1 is T;
  final cast = 1 as T;
}

const all = 1;
class ConstructorNameScope {
  const ConstructorNameScope.all();
  static final cast = all as int; /* expect: prefer-const-declarations */
  static final text = '$all'; /* expect: prefer-const-declarations */
}

const shadow = 1;
class MethodNameScope {
  static void shadow() {}
  static const list = [shadow];
}
final invalidMethodListCast = MethodNameScope.list as List<int>;

class ImplicitConstructor {}
final implicitConstructorEquality = ImplicitConstructor.new == ImplicitConstructor.new; /* expect: prefer-const-declarations */
final implicitConstructorCast = ImplicitConstructor.new as Function; /* expect: prefer-const-declarations */

class GenericConstructor<T> {}
final deferredTypeLiteral = List<lazy.OC>; /* expect: prefer-const-declarations */
final deferredConstructorTearoff = GenericConstructor<lazy.OC>.new; /* expect: prefer-const-declarations */

abstract class ImplicitAbstract {}
final implicitAbstractTearoff = ImplicitAbstract.new; /* expect: prefer-const-declarations */
abstract class ExplicitAbstract {
  ExplicitAbstract();
}
final explicitAbstractTearoff = ExplicitAbstract.new; /* expect: prefer-const-declarations */

const namedString = 'x';
const Object integerObject = 1;
final namedStringLength = namedString.length; /* expect: prefer-const-declarations */
final beyondWordShift = 1 << 70; /* expect: prefer-const-declarations */
final negatedSignBit = -(1 << 63); /* expect: prefer-const-declarations */
final typeTestConditional = integerObject is String ? 1 : 2; /* expect: prefer-const-declarations */
final overflowingSum = 9223372036854775807 + 1; /* expect: prefer-const-declarations */
