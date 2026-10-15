/// First conversion interface.
abstract interface class First<E> {
  /// Converts the elements.
  First<T> cast<T>();
}

/// Second conversion interface.
abstract interface class Second<E> {
  /// Converts the elements.
  Second<T> cast<T>();
}

/// Its own valid override cannot combine incomparable inherited signatures.
abstract class Combined<E> implements First<E>, Second<E> {
  Combined<T> cast<T>(); /* expect: public-member-api-docs */
}

/// One inherited signature still exempts an undocumented override.
abstract class Single<E> implements First<E> {
  Single<T> cast<T>();
}

/// Supplies a concrete conversion implementation.
mixin FirstMixin<E> implements First<E> {
  First<T> cast<T>() => throw 0;
}

/// A mixin and an interface can also have incomparable return types.
class CombinedMixin<E> with FirstMixin<E> implements Second<E> {
  CombinedMixin<T> cast<T>() => throw 0; /* expect: public-member-api-docs */
}
