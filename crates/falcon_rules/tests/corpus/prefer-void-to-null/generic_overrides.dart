import 'dart:async';

abstract class Base<T> {
  T method();
  T get value;
}

// Substitute T before deciding whether the overriding Null is unnecessary.
class VoidOverride extends Base<void> {
  @override
  Null method() => null; /* expect: prefer-void-to-null */
  @override
  Null get value => null; /* expect: prefer-void-to-null */
}

// Null in a type argument is unnecessary, but these override returns are not.
class NullOverride extends Base<Null> { /* expect: prefer-void-to-null */
  @override
  Null method() => null;
  @override
  Null get value => null;
}

// Future<void> is not void-like, even though these overrides are invalid Dart.
class FutureVoidOverride extends Base<Future<void>> {
  @override
  Null method() => null;
  @override
  Null get value => null;
}

// FutureOr<void>, unlike Future<void>, is void-like for this lint.
class FutureOrVoidOverride extends Base<FutureOr<void>> {
  @override
  Null method() => null; /* expect: prefer-void-to-null */
  @override
  Null get value => null; /* expect: prefer-void-to-null */
}
