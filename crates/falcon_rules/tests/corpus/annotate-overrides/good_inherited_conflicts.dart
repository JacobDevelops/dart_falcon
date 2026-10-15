class A {}
class B {}
abstract class First { A make(); }
abstract class Second { B make(); }
class Compatible implements First, Second {
  // A valid declared override does not supply a combined inherited signature.
  Never make() => throw StateError('');
}

abstract class ListView<E> {}
abstract class QueueView<E> { QueueView<T> cast<T>(); }
mixin ListMethods<E> { ListView<T> cast<T>() => throw StateError(''); }
class QueueListLike<E> with ListMethods<E> implements QueueView<E>, ListView<E> {
  // SDK-free equivalent of collection's QueueList.cast: unrelated inherited
  // List<T> and Queue<T> results, despite the declared result implementing both.
  QueueListLike<T> cast<T>() => throw StateError('');
}

abstract class LeftCall { A call(A value); }
abstract class RightCall { A call(B value); }
class CombinedCall implements LeftCall, RightCall {
  A call(Object value) => A();
}

abstract class OneArgument { A call(A first); }
abstract class TwoArguments { A call(A first, A second); }
class CombinedArguments implements OneArgument, TwoArguments {
  A call(A first, [A? second]) => A();
}

abstract class NamedX { A call({required A x}); }
abstract class NamedY { A call({required A y}); }
class CombinedNamed implements NamedX, NamedY {
  A call({A? x, A? y}) => A();
}

abstract class FirstGetter { A get value; }
abstract class SecondGetter { B get value; }
class CombinedGetter implements FirstGetter, SecondGetter {
  Never get value => throw StateError('');
}

abstract class FirstSetter { set value(A value); }
abstract class SecondSetter { set value(B value); }
class CombinedSetter implements FirstSetter, SecondSetter {
  set value(Object value) {}
}

abstract class GenericReturn<T> { T make(); }
abstract class OtherGenericReturn<T> { T make(); }
class DifferentInstantiations implements GenericReturn<A>, OtherGenericReturn<B> {
  Never make() => throw StateError('');
}
