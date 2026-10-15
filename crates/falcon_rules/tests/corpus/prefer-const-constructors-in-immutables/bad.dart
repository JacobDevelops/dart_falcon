// @dart = 3.13

import 'package:meta/meta.dart';
class ConstValue {
  const ConstValue.named();
}

@immutable
class ConstFieldInitializer {
  final ConstValue value = const ConstValue.named();
  ConstFieldInitializer(); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
class LaxOperands {
  final Object value;
  LaxOperands(Object a, Object b, List<int> l) : value = (a == b, '$a', l.length, a is int, int); /* expect: prefer-const-constructors-in-immutables */
  external LaxOperands.external(); /* expect: prefer-const-constructors-in-immutables */
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

@immutable
class Primary() {} /* expect: prefer-const-constructors-in-immutables */

@immutable
class PrimaryField(final int value) {} /* expect: prefer-const-constructors-in-immutables */

@immutable
class NamedPrimary<T>.named(final T value) {} /* expect: prefer-const-constructors-in-immutables */

@immutable
abstract class WidgetBase {
  const WidgetBase({Object? key});
}

abstract class PrimaryWidget({super.key}) extends WidgetBase {} /* expect: prefer-const-constructors-in-immutables */

abstract class PrimaryWidgetWithoutKey() extends WidgetBase {} /* expect: prefer-const-constructors-in-immutables */

abstract class PrimaryWidgetBody() extends WidgetBase { /* expect: prefer-const-constructors-in-immutables */
  @deprecated
  this;
}

@immutable
class PrimaryInitializer(int value) { /* expect: prefer-const-constructors-in-immutables */
  final int field;
  this : field = value, assert(value >= 0);
}

class PrimaryNamedSuper(final int value) extends ImmutableBase { /* expect: prefer-const-constructors-in-immutables */
  this : super.named();
}

@immutable
extension type Representation(List<int> value) {} /* expect: prefer-const-constructors-in-immutables */

@immutable
extension type NamedRepresentation<T>.named(T value) {} /* expect: prefer-const-constructors-in-immutables */

@immutable
class PrimaryRuntimeInitializer() { /* expect: prefer-const-constructors-in-immutables */
  final DateTime value;
  this : value = DateTime.now();
}

@immutable
class PrimaryRuntimeFieldInitializer() { /* expect: prefer-const-constructors-in-immutables */
  final DateTime value = DateTime.now();
}

@immutable
mixin class PrimaryMixin() {} /* expect: prefer-const-constructors-in-immutables */

@immutable
extension type const ConstPrimaryOrdinary(int value) {
  ConstPrimaryOrdinary.other(this.value); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
extension type NonConstPrimaryOrdinary(int value) { /* expect: prefer-const-constructors-in-immutables */
  NonConstPrimaryOrdinary.other(this.value);
}

@immutable
extension type EmptyRepresentationBody(int value) { /* expect: prefer-const-constructors-in-immutables */
  this;
}

@immutable
class TearoffInitializer {
  final Object callback;
  TearoffInitializer() : callback = TearoffInitializer.target; /* expect: prefer-const-constructors-in-immutables */
  const TearoffInitializer.target() : callback = 0;
  factory TearoffInitializer.redirect() = TearoffInitializer.target; /* expect: prefer-const-constructors-in-immutables */
}

@Immutable()
class AnnotationConstructor {
  AnnotationConstructor(); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
extension type const ConstRepresentation(int value) {
  ConstRepresentation.named(this.value); /* expect: prefer-const-constructors-in-immutables */
}

@immutable
extension type RuntimeRepresentation(int value) {} /* expect: prefer-const-constructors-in-immutables */
