import 'constants.dart';
import 'constants.dart' as po;

final imported = otherConst; /* expect: prefer-const-declarations */
final interpolation = '$otherConst'; /* expect: prefer-const-declarations */
final enumValue = OE.a; /* expect: prefer-const-declarations */
final staticConstant = OC.sc; /* expect: prefer-const-declarations */
final prefixedConstant = po.OC.sc; /* expect: prefer-const-declarations */
final enumValues = po.OE.values; /* expect: prefer-const-declarations */
final importedFunction = po.otherFn; /* expect: prefer-const-declarations */
final staticFunction = po.Parser.parse; /* expect: prefer-const-declarations */
final genericFunction = po.max<int>; /* expect: prefer-const-declarations */
final importedType = po.OC; /* expect: prefer-const-declarations */
final importedConstructor = po.OC.new; /* expect: prefer-const-declarations */
final identicalResult = identical(1, 2); /* expect: prefer-const-declarations */

class A {
  const A();
}

class C<T> {
  static C<X> m<X>() => C<X>();
}

final constructorEquality = A.new == A.new; /* expect: prefer-const-declarations */
final constructor = A.new; /* expect: prefer-const-declarations */
final coreType = int; /* expect: prefer-const-declarations */
final genericType = List<int>; /* expect: prefer-const-declarations */

void generic<T>() {
  final constructor = C<T>.new; /* expect: prefer-const-declarations */
  final typeLiteral = List<T>; /* expect: prefer-const-declarations */
  final genericMethod = C.m<T>;
}

final zeroDivision = 1 ~/ 0.0;
final failedCast = 1 as String;
final nullableCast = null as int?; /* expect: prefer-const-declarations */

void shadowed(dynamic po, dynamic identical) {
  final staticConstant = po.OC.sc;
  final function = po.otherFn;
  final invocation = identical(1, 2);
}
