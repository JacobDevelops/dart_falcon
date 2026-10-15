import 'dart:async';
import 'constants.dart' as constants;
import 'constants.dart' deferred as deferred;

const double declaredDouble = 1;
const double? nullableDouble = 1;
final doubleAsInt = declaredDouble as int;
final doubleAsDouble = declaredDouble as double; /* expect: prefer-const-declarations */
final nullableDoubleAsInt = nullableDouble as int;
final importedDoubleAsInt = constants.doubleValue as int;
final importedDoubleAsDouble = constants.doubleValue as double; /* expect: prefer-const-declarations */

const shadow = 1;
class StaticShadow {
  static const shadow = 1.5;
  static final cast = shadow as int;
}
class MethodShadow {
  static void shadow() {}
  static final cast = shadow as int;
}

enum Color { red }
final enumCast = Color.red as Color; /* expect: prefer-const-declarations */
final inferredListCast = const [1] as List<int>; /* expect: prefer-const-declarations */
final recordCast = (1, 2) as (int, int); /* expect: prefer-const-declarations */
final comparableCast = 1 as Comparable<num>; /* expect: prefer-const-declarations */

class Box<T> {
  final T value;
  const Box(this.value);
}
final rawGenericCast = const Box(1) as Box; /* expect: prefer-const-declarations */
final futureOrCast = 1 as FutureOr<int>; /* expect: prefer-const-declarations */
final deferredNullableCast = null as deferred.OC?;
final deferredAliasCast = 1 as deferred.I;
final deferredTypeTest = 1 is deferred.OC;
final incorrectGenericCast = const Box(declaredDouble) as Box<int>;
final iterableCast = const <int>[] as Iterable<int>; /* expect: prefer-const-declarations */

const T = 1;
void typeParameterShadow<T>() {
  final cast = T as int;
  const double? localDouble = 1;
  final localCast = localDouble as int;
}

const List<int> typedList = [1, 2];
final listInterpolation = '$typedList';
const Color shorthand = .red;
final shorthandInterpolation = '$shorthand';
final shorthandCast = shorthand as int;
const Object boxedInt = 1;
final primitiveInterpolation = '$boxedInt'; /* expect: prefer-const-declarations */
void localCollections() {
  const List<int> localList = [1];
  final interpolation = '$localList';
}
class Holder {
  static const Map<String, int> map = {'a': 1};
  static final interpolation = '$map';
}
