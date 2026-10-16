class Base {
  const Base(int value);
}
class NamedPrimary.named(int value) extends Base { /* expect: use-super-parameters */
  this : super(value);
}
class const ConstPrimary.named(int value) extends Base { /* expect: use-super-parameters */
  this : super(value);
}
class const UnnamedConstPrimary(int value) extends Base { /* expect: use-super-parameters */
  this : super(value);
}
class CallbackBase {
  CallbackBase({void Function(int)? callback});
}
class CallbackChild extends CallbackBase {
  CallbackChild({void Function(int)? callback}) : super(callback: callback); /* expect: use-super-parameters */
}
class GenericBase {
  GenericBase({T Function<T>(T)? callback});
}
class GenericChild extends GenericBase {
  GenericChild({U Function<U>(U)? callback}) : super(callback: callback); /* expect: use-super-parameters */
}
