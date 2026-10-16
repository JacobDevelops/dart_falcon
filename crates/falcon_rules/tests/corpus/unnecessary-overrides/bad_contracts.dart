import 'package:meta/meta.dart';

class Parent {
  void defaults([int value = 1]) {}
  void covariantParameter(covariant num value) {}
  @protected
  void guarded() {}
  @protected
  int get paired => 0;
  set paired(int value) {}
  set setterOnly(int value) {}
}
class Child extends Parent {
  @override
  void defaults([int value = 1]) => super.defaults(value); /* expect: unnecessary-overrides */
  @override
  void covariantParameter(covariant num value) => super.covariantParameter(value); /* expect: unnecessary-overrides */
  @override
  @protected
  void guarded() => super.guarded(); /* expect: unnecessary-overrides */
  @override
  set paired(int value) => super.paired = value; /* expect: unnecessary-overrides */
  @override
  set setterOnly(int value) => super.setterOnly = value; /* expect: unnecessary-overrides */
}

class CovarianceBase {
  void interfaces(num value) {}
  void mixed(covariant num value) {}
}
class CovariantInterface {
  void interfaces(covariant num value) {}
}
class PlainInterface {
  void interfaces(num value) {}
}
abstract class CombinedInterface implements PlainInterface, CovariantInterface {}
class Intermediate extends CovarianceBase implements CombinedInterface {}
class InterfaceChild extends Intermediate {
  @override
  void interfaces(num value) => super.interfaces(value); /* expect: unnecessary-overrides */
}
mixin PlainMixin {
  void mixed(num value) {}
}
class Mixed extends CovarianceBase with PlainMixin {}
class MixedChild extends Mixed {
  @override
  void mixed(num value) => super.mixed(value); /* expect: unnecessary-overrides */
}

const defaultName = 'name';
T defaultIdentity<T>(T value) => value;

class DefaultSyntaxBase {
  void arithmetic([int value = 1 + 2]) {}
  void adjacent([String value = 'a' 'b']) {}
  void interpolated([String value = '${defaultName}']) {}
  void symbol([Symbol value = #one.two]) {}
  void collection([Object value = const <String, int>{'a': 1}]) {}
  void record([Object value = (1, label: 2)]) {}
  void functionType([Object value = defaultIdentity<void Function(int)>]) {}
  void recordConstant([Object value = const (1, 2)]) {}
  void recordSingleton([Object value = (1,)]) {}
  void annotation([Object value = defaultIdentity<void Function(@deprecated int)>]) {}
}

class DefaultSyntaxChild extends DefaultSyntaxBase {
  @override
  void arithmetic([int value = 1 /* inside */ + 2]) => super.arithmetic(value); /* expect: unnecessary-overrides */
  @override
  void adjacent([String value = 'a' /* between */ 'b']) => super.adjacent(value); /* expect: unnecessary-overrides */
  @override
  void interpolated([String value = '${ /* before */ defaultName /* after */ }']) => super.interpolated(value); /* expect: unnecessary-overrides */
  @override
  void symbol([Symbol value = #one /* between */ . two]) => super.symbol(value); /* expect: unnecessary-overrides */
  @override
  void collection([Object value = const < String, int > { 'a' : 1, }]) => super.collection(value); /* expect: unnecessary-overrides */
  @override
  void record([Object value = ( 1 , label : 2 , )]) => super.record(value); /* expect: unnecessary-overrides */
  @override
  void functionType([Object value = defaultIdentity < void Function ( int ) >]) => super.functionType(value); /* expect: unnecessary-overrides */
  @override
  void recordConstant([Object value = (1, 2)]) => super.recordConstant(value); /* expect: unnecessary-overrides */
  @override
  void recordSingleton([Object value = (1)]) => super.recordSingleton(value); /* expect: unnecessary-overrides */
  @override
  void annotation([Object value = defaultIdentity<void Function( @deprecated /* after */ int )>]) => super.annotation(value); /* expect: unnecessary-overrides */
}

// An undeclared mixin member combines with, rather than replacing, the
// equal-typed superclass member. Its covariant interface flag is not selected.
mixin InheritedCovarianceMixin implements CovariantInterface {}
class InheritedMixed extends CovarianceBase with InheritedCovarianceMixin {}
class InheritedMixedChild extends InheritedMixed {
  @override
  void interfaces(num value) => super.interfaces(value); /* expect: unnecessary-overrides */
}

// Mixin super calls use the on-constraint interface, not its contextual copy.
mixin ContextConstraint on Intermediate {
  @override
  void interfaces(num value) => super.interfaces(value); /* expect: unnecessary-overrides */
}

// A later covariance contribution recomputes all flags on the inherited copy.
class PairBase {
  void pair(num left, num right) {}
}
abstract class LeftCovariance {
  void pair(covariant num left, num right);
}
abstract class RightCovariance {
  void pair(num left, covariant num right);
}
class FirstPair extends PairBase implements LeftCovariance {}
class ThroughPair extends FirstPair {}
class ChangedPair extends ThroughPair implements RightCovariance {}
class ChangedPairChild extends ChangedPair {
  @override
  void pair(num left, covariant num right) => super.pair(left, right); /* expect: unnecessary-overrides */
}

// Abstract mixin declarations do not replace an applying superclass's body.
class AbstractMixinBase {
  void mixedAbstract([int value = 2]) {}
}
abstract class AbstractMixinInterface {
  void mixedAbstract([int value = 2]);
}
mixin AbstractContractMixin implements AbstractMixinInterface {
  @override
  void mixedAbstract([int value = 2]);
}
class AbstractMixinApplication extends AbstractMixinBase with AbstractContractMixin {}
class AbstractMixinChild extends AbstractMixinApplication {
  @override
  void mixedAbstract([int value = 2]) => super.mixedAbstract(value); /* expect: unnecessary-overrides */
}

class ObjectForwarder {
  @override
  String toString() => super.toString(); /* expect: unnecessary-overrides */
}

mixin class MixinClassForwarder {
  @override
  String toString() => super.toString(); /* expect: unnecessary-overrides */
}

class OperatorBase {
  int operator +(int other) => other;
  int operator -(int other) => other;
  int operator -() => 0;
  int operator ~() => 0;
}

class OperatorChild extends OperatorBase {
  @override
  int operator +(int other) => super + other; /* expect: unnecessary-overrides */
  @override
  int operator -(int other) => (super) - (other); /* expect: unnecessary-overrides */
  @override
  int operator -() => -(super); /* expect: unnecessary-overrides */
  @override
  int operator ~() { return ~super; } /* expect: unnecessary-overrides */
}

class SynthesizedDefaultBase {
  void m([int? v = 1]) {}
}
abstract class SynthesizedDefaultInterface {
  void m([covariant int? v = 1]);
}
class SynthesizedDefaultContext extends SynthesizedDefaultBase
    implements SynthesizedDefaultInterface {}
class SynthesizedDefaultChild extends SynthesizedDefaultContext {
  @override
  void m([covariant int? v]) => super.m(v); /* expect: unnecessary-overrides */
}

class BodyCommentBase {
  void m() {}
  int value() => 0;
}
class BodyCommentChild extends BodyCommentBase {
  @override
  void m() { super.m(); /* after invocation */ } /* expect: unnecessary-overrides */
  @override
  int value() => (/* before parentheses */ (super)).value(); /* expect: unnecessary-overrides */
}

class ObjectEqualityForwarder {
  @override
  bool operator ==(Object other) => super == other; /* expect: unnecessary-overrides */
}
