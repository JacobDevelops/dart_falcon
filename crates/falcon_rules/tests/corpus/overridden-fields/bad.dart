class Base { int value = 0; }
class Child extends Base {
  int value = 1; /* expect: overridden-fields */
}
