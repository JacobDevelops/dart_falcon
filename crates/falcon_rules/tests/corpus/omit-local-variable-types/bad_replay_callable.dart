class Callable { void call() {} }
void f(Callable value) {
  void Function() callback = value; /* expect: omit-local-variable-types */
  callback();
}
