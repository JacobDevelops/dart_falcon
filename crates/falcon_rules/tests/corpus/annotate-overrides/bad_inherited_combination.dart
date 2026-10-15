class A {}
class B {}
abstract class Wide { Object make(); }
abstract class Narrow { A make(); }
class MostSpecific implements Wide, Narrow {
  A make() => A(); /* expect: annotate-overrides */
}

abstract class First { A make(); }
abstract class Second { B make(); }
abstract class Bottom { Never make(); }
class ThirdCandidate implements First, Second, Bottom {
  Never make() => throw StateError(''); /* expect: annotate-overrides */
}

class Repaired implements First, Second {
  Never make() => throw StateError('');
}
class InheritsRepair extends Repaired {
  Never make() => throw StateError(''); /* expect: annotate-overrides */
}

abstract class RequiredCall { A call(A value); }
abstract class OptionalCall { A call([Object? value]); }
class OptionalMostSpecific implements RequiredCall, OptionalCall {
  A call([Object? value]) => A(); /* expect: annotate-overrides */
}

abstract class RequiredNamed { A call({required A value}); }
abstract class OptionalNamed { A call({Object? value}); }
class NamedMostSpecific implements RequiredNamed, OptionalNamed {
  A call({Object? value}) => A(); /* expect: annotate-overrides */
}

abstract class GenericWide { Object? make<T>(); }
abstract class GenericNarrow { U make<U>(); }
class GenericMostSpecific implements GenericWide, GenericNarrow {
  Never make<V>() => throw StateError(''); /* expect: annotate-overrides */
}

class Superclass { Object make() => A(); }
mixin Replacement { A make() => A(); }
class AppliedMixin extends Superclass with Replacement {
  A make() => A(); /* expect: annotate-overrides */
}

abstract class WideGetter { Object get value; }
abstract class NarrowGetter { A get value; }
class GetterMostSpecific implements WideGetter, NarrowGetter {
  A get value => A(); /* expect: annotate-overrides */
}

abstract class NarrowSetter { set value(A value); }
abstract class WideSetter { set value(Object value); }
class SetterMostSpecific implements NarrowSetter, WideSetter {
  set value(Object value) {} /* expect: annotate-overrides */
}

abstract class VoidReturn { void make(); }
class ValueAndVoid implements VoidReturn, Narrow {
  A make() => A(); /* expect: annotate-overrides */
}
