// @dart = 3.13

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

class ConstValue {
  const ConstValue.named();
}

@meta.immutable
class CallShapedFieldInitializer {
  final ConstValue value = ConstValue.named();
  CallShapedFieldInitializer();
}

@meta.immutable
abstract class ImplicitConstructorBase {}
class ImplicitSuper extends ImplicitConstructorBase {
  final int value;
  ImplicitSuper(this.value);
}

@meta.immutable
class BlockBody {
  final int value;
  const BlockBody.named(this.value);
  BlockBody(this.value) {}
}

@meta.immutable
class NonLengthProperty {
  final bool value;
  const NonLengthProperty.named(this.value);
  NonLengthProperty(int x) : value = x.isEven;
  NonLengthProperty.list(int x) : value = [x].isEmpty;
  NonLengthProperty.bang(bool? x) : value = x!;
}

@meta.immutable
abstract class Marker {}
class InterfaceOnly implements Marker {
  final int value;
  InterfaceOnly(this.value);
}

@meta.immutable
class const ConstPrimary(final int value) {}

class UnannotatedPrimary(final int value) {}

@meta.immutable
class MutablePrimary(var int value) {}

@meta.immutable
class MutableBodyField() {
  int value = 0;
}

@meta.immutable
class PrimaryBlock() {
  this {}
}

@meta.immutable
abstract class AbstractPrimaryBlock() {
  this {}
}

class PrimaryNonConstSuper(final int value) extends NonConstBase {}

class PrimaryImplicitSuper(final int value) extends ImplicitConstructorBase {}

class PrimaryInterfaceOnly(final int value) implements Marker {}

mixin EmptyMixin {}

@meta.immutable
class PrimaryWithMixin() with EmptyMixin {}

@meta.immutable
mixin class ImmutableMixin {}

class PrimaryMixinAnnotationOnly() with ImmutableMixin {}

@meta.immutable
extension type const ConstRepresentation(List<int> value) {}

extension type UnannotatedRepresentation(List<int> value) {}

@meta.immutable
extension type const NamedConstRepresentation<T>.named(T value) {}

@meta.immutable
mixin class const ConstPrimaryMixin() {}

mixin class UnannotatedPrimaryMixin() {}

@meta.immutable
extension type const ConstOrdinaryRepresentation(int value) {
  const ConstOrdinaryRepresentation.other(this.value);
  ConstOrdinaryRepresentation.block(this.value) {}
}

@meta.immutable
extension type BlockRepresentationBody(int value) {
  this {}
  BlockRepresentationBody.other(this.value);
}

@meta.immutable
class NonConstCollectionField {
  final List<int> values = [];
  final List<int> nonEmpty = [1];
  final Set<int> emptySet = <int>{};
  NonConstCollectionField();
}

@meta.immutable
class NonConstCollectionInitializers {
  final Object value;
  NonConstCollectionInitializers.list() : value = [1];
  NonConstCollectionInitializers.map() : value = <int, int>{};
  NonConstCollectionInitializers.set() : value = <int>{};
}
