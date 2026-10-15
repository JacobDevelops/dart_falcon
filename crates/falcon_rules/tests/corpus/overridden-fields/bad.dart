class Base { int value = 0; }
class Child extends Base {
  int value = 1; /* expect: overridden-fields */
}

mixin class Mixed { final int mixed = 0; }
class FromMixin with Mixed {
  final int mixed = 1; /* expect: overridden-fields */
}

class Covariant { covariant num value = 0; }
class FromCovariant extends Covariant {
  int value = 1; /* expect: overridden-fields */
}
