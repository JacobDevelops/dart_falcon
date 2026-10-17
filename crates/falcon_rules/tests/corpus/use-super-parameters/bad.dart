// Parameters forwarded verbatim to super and used nowhere else.
class Base {
  Base(int a);
  Base.two(int a, int b);
  Base.named({int? x});
  Base.both(int a, {int? x});
}

class A extends Base {
  A(int a) : super(a); /* expect: use-super-parameters */
}

class B extends Base {
  B(int a, int b) : super.two(a, b); /* expect: use-super-parameters */
}

class C extends Base {
  C({int? x}) : super.named(x: x); /* expect: use-super-parameters */
}

class D extends Base {
  D(int a) : super(a); /* expect: use-super-parameters */
}

class E extends Base {
  E(int a) : super(a); /* expect: use-super-parameters */
}

class ObjectBase {
  ObjectBase(Object e);
}

class CatchBinding extends ObjectBase {
  CatchBinding(Object e) : super(e) { /* expect: use-super-parameters */
    try {} catch (e) {}
  }
}

class SkippedParameter extends Base {
  SkippedParameter(String unused, int value) : super(value); /* expect: use-super-parameters */
}

class Multiline extends Base {
  Multiline( /* expect: use-super-parameters */
    int value,
  ) : super(value);
}

class InitializerReference extends Base {
  final int saved;
  InitializerReference(int value) : saved = value, super(value); /* expect: use-super-parameters */
}

class Primary(int value) extends Base { /* expect: use-super-parameters */
  this : super(value);
}

class NamedConstructor extends Base {
  NamedConstructor.named(int value) : super(value); /* expect: use-super-parameters */
}

class FieldWithNamed extends Base {
  final int field;
  FieldWithNamed(this.field, {int? x}) : super.both(field, x: x); /* expect: use-super-parameters */
}

class SkippedParameters extends Base {
  SkippedParameters(String unused, int a, bool skipped, int b) : super.two(a, b); /* expect: use-super-parameters */
}
