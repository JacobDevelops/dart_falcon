extension OldObjectCall on Object {
  @deprecated
  void call() {}
}
void uses(Function function, void Function() typedFunction, Object object) {
  function();
  function.call();
  function.call;
  typedFunction();
  typedFunction.call();
  typedFunction.call;
  object(); /* expect: deprecated-member-use-from-same-package */
  object.call(); /* expect: deprecated-member-use-from-same-package */
}
