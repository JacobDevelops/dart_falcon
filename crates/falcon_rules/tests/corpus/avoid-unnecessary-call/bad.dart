typedef Action = void Function();
void inspect(void Function() action, int Function(int) convert, Action alias, Function unknownSignature) {
  action.call(); /* expect: avoid-unnecessary-call */
  convert.call(1); /* expect: avoid-unnecessary-call */
  alias.call(); /* expect: avoid-unnecessary-call */
  unknownSignature.call(); /* expect: avoid-unnecessary-call */
  (action).call(); /* expect: avoid-unnecessary-call */
}

class Callable {
  void call() {}
}
class InheritedCallable extends Callable {}
void instances(Callable callable, InheritedCallable inherited) {
  callable.call(); /* expect: avoid-unnecessary-call */
  inherited.call(); /* expect: avoid-unnecessary-call */
}
