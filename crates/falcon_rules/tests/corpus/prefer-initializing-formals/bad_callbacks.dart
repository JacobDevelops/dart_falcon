// Callback copies require function subtyping, including variance and binders.
class Callback {
  final void Function() callback;
  Callback(void Function() callback) : callback = callback; /* expect: prefer-initializing-formals */
}
class Variance {
  final num Function(int) callback;
  Variance(int Function(num) callback) : callback = callback; /* expect: prefer-initializing-formals */
}
class NamedVariance {
  final void Function({required int x}) callback;
  NamedVariance(void Function({int x, int y}) callback) : callback = callback; /* expect: prefer-initializing-formals */
}
class Generic {
  final T Function<T>(T) callback;
  Generic(U Function<U>(U) callback) : callback = callback; /* expect: prefer-initializing-formals */
}
class OptionalPositional {
  final void Function(int) callback;
  OptionalPositional(void Function([num?]) callback) : callback = callback; /* expect: prefer-initializing-formals */
}
