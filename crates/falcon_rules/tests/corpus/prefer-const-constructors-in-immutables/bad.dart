import 'package:meta/meta.dart';
class ConstValue {
  const ConstValue.named();
}

@immutable
class CallShapedConstInitializer {
  final ConstValue value = ConstValue.named();
  CallShapedConstInitializer(); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
class Value {
  final int value;
  Value(this.value); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
abstract class ImmutableBase {
  const ImmutableBase.named();
}
class Inherited extends ImmutableBase {
  final int value;
  Inherited(this.value) : super.named(); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
class Redirecting {
  final int value;
  const Redirecting.target(this.value);
  Redirecting(int value) : this.target(value); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
abstract class Interface {
  factory Interface() = Implementation; /* expect: prefer-const-constructors-in-immutables */
}
class Implementation implements Interface {
  const Implementation();
}
