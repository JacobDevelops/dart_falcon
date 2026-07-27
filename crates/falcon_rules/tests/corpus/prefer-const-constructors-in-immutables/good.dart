import 'package:meta/meta.dart' as meta;
@meta.immutable
class Value {
  final int value;
  const Value(this.value);
}

@meta.immutable
abstract class NonConstBase {
  final DateTime created;
  NonConstBase() : created = DateTime.now();
}
class CannotInheritConst extends NonConstBase {
  final int value;
  CannotInheritConst(this.value);
}

@meta.immutable
class RuntimeInitializer {
  final DateTime value;
  RuntimeInitializer() : value = DateTime.now();
}

@meta.immutable
class RuntimeFieldInitializer {
  final DateTime value = DateTime.now();
  RuntimeFieldInitializer();
}

@meta.immutable
class ShadowedConstConstructor {
  final Value value;
  ShadowedConstConstructor(Value Function() Value) : value = Value();
}

@meta.immutable
class GenericCalleeShadow<ConstValue> {
  final Object value;
  GenericCalleeShadow() : value = ConstValue.named();
}

@meta.immutable
class MemberCalleeShadow {
  static final ConstValue = () => const Value(0);
  final Object value;
  MemberCalleeShadow() : value = ConstValue();
}

@meta.immutable
class FactoryBody {
  const FactoryBody._();
  factory FactoryBody() => const FactoryBody._();
}
