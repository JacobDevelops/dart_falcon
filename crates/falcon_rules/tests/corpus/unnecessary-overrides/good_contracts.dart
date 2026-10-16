import 'package:meta/meta.dart';

class Base {
  @protected
  void exposed() {}
  dynamic value() => 1;
  void documented() {}
  void documentedBefore() {}
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class Derived extends Base {
  @override
  void exposed() => super.exposed();

  @override
  int value() => super.value();

  @override
  /// Documentation after metadata also preserves the override.
  void documented() => super.documented();

  /// A documented public contract.
  @override
  void documentedBefore() => super.documentedBefore();

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}

class NarrowBase {
  void add(covariant Function handler) {}
}
class NarrowDerived extends NarrowBase {
  @override
  void add(void Function(int) handler) => super.add(handler);
}

class ParameterBase {
  void renamed(int original) {}
  void kinds(int? value) {}
  void defaults([int value = 1]) {}
  set value(covariant num original) {}
}
class ParameterDerived extends ParameterBase {
  @override
  void renamed(int changed) => super.renamed(changed);
  @override
  void kinds([int? value]) => super.kinds(value);
  @override
  void defaults([int value = 2]) => super.defaults(value);
  @override
  set value(int changed) => super.value = changed;
}

class ContextBase {
  void contextual(num value) {}
}
class ContextInterface {
  void contextual(covariant num value) {}
}
class Context extends ContextBase implements ContextInterface {}
class ContextChild extends Context {
  @override
  void contextual(num value) => super.contextual(value);
}

const defaultName = 'name';
T defaultIdentity<T>(T value) => value;

class DefaultArguments {
  const DefaultArguments(int value, {int named = 0});
}

class DefaultSyntaxBase {
  void arithmetic([int value = 1 + 1]) {}
  void parentheses([int value = 1]) {}
  void radix([int value = 1]) {}
  void quotes([String value = 'text']) {}
  void adjacent([String value = 'a' 'b']) {}
  void interpolated([String value = '$defaultName']) {}
  void symbol([Symbol value = #one]) {}
  void functionType([Object value = defaultIdentity<void Function(int)>]) {}
  void parameterName([Object value = defaultIdentity<void Function(int)>]) {}
  void argumentOrder([Object value = const DefaultArguments(named: 1, 2)]) {}
  void recordNames([Object value = defaultIdentity<(int label,)>]) {}
  void annotation([Object value = defaultIdentity<void Function(@deprecated int)>]) {}
}

class DefaultSyntaxChild extends DefaultSyntaxBase {
  @override
  void arithmetic([int value = 2]) => super.arithmetic(value);
  @override
  void parentheses([int value = (1)]) => super.parentheses(value);
  @override
  void radix([int value = 0x1]) => super.radix(value);
  @override
  void quotes([String value = "text"]) => super.quotes(value);
  @override
  void adjacent([String value = 'ab']) => super.adjacent(value);
  @override
  void interpolated([String value = '${defaultName}']) => super.interpolated(value);
  @override
  void symbol([Symbol value = #two]) => super.symbol(value);
  @override
  void functionType([Object value = defaultIdentity<void Function([int])>]) => super.functionType(value);
  @override
  void parameterName([Object value = defaultIdentity<void Function(int value)>]) => super.parameterName(value);
  @override
  void argumentOrder([Object value = const DefaultArguments(2, named: 1)]) => super.argumentOrder(value);
  @override
  void recordNames([Object value = defaultIdentity<(int,)>]) => super.recordNames(value);
  @override
  void annotation([Object value = defaultIdentity<void Function(int)>]) => super.annotation(value);
}

// Covariance on a concrete inherited implementation survives another level.
class DeeperContext extends Context {}
class DeeperContextChild extends DeeperContext {
  @override
  void contextual(num value) => super.contextual(value);
}

// Abstract redeclarations do not replace the implementation called by super.
class ContractBase {
  void defaults([int value = 2]) {}
  void covariance(num value) {}
}
abstract class AbstractContract extends ContractBase {
  @override
  void defaults([int value = 1]);
  @override
  void covariance(covariant num value);
}
class ConcreteContract extends AbstractContract {
  @override
  void defaults([int value = 1]) => super.defaults(value);
  @override
  void covariance(covariant num value) => super.covariance(value);
}

class ConstraintDefaultBase {
  void mixedAbstract([int value = 1]) {}
}
class ActualDefaultBase extends ConstraintDefaultBase {
  @override
  void mixedAbstract([int value = 2]) {}
}
mixin AbstractConstraintDefault on ConstraintDefaultBase {
  @override
  void mixedAbstract([int value = 1]);
}
class AbstractConstraintApplication extends ActualDefaultBase with AbstractConstraintDefault {}
class AbstractConstraintChild extends AbstractConstraintApplication {
  @override
  void mixedAbstract([int value = 1]) => super.mixedAbstract(value);
}

class BodyCommentBase {
  void a() {}
  void b() {}
  int value() => 0;
  int get field => 0;
  set field(int value) {}
}
class BodyCommentChild extends BodyCommentBase {
  @override
  void a() {
    // Explanation before super.
    super.a();
  }
  @override
  void b() =>
      // Explanation in an arrow body.
      super.b();
  @override
  int value() {
    // Explanation before return.
    return super.value();
  }
  @override
  int get field => /* Explanation before super. */ super.field;
  @override
  set field(int value) {
    /* Explanation before super. */
    super.field = value;
  }
}

class ObjectForwarder {
  @override
  String toString() {
    // Explain the override.
    return super.toString();
  }
}

mixin class MixinClassForwarder {
  /// Preserve this documentation.
  @override
  String toString() => super.toString();
}

class OperatorBase {
  int operator +(int other) => other;
  int operator -(int other) => other;
  int operator -() => 0;
  int operator ~() => 0;
}
class OperatorChild extends OperatorBase {
  @override
  int operator +(int renamed) => super + renamed;
  @override
  int operator -(int other) => super + other;
  @override
  int operator -() => - /* Explanation before super. */ super;
  @override
  int operator ~() => ~super + 1;
}

class SynthesizedDefaultBase {
  void m([int v = 1]) {}
}
abstract class SynthesizedDefaultInterface {
  void m([covariant int v = 1]);
}
class SynthesizedDefaultContext extends SynthesizedDefaultBase
    implements SynthesizedDefaultInterface {}
class SynthesizedDefaultChild extends SynthesizedDefaultContext {
  @override
  void m([covariant int v = 1]) => super.m(v);
}

class ObjectEqualityForwarder {
  @override
  bool operator ==(Object renamed) => super == renamed;
}
