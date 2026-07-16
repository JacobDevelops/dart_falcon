// Setters never return a value, so any return type is redundant.
class A {
  int _v = 0;
  void set value(int v) { _v = v; } /* expect: avoid-return-types-on-setters */
  int _w = 0;
  int set width(int v) { _w = v; } /* expect: avoid-return-types-on-setters */
}

class B {
  String _n = '';
  String set name(String v) { _n = v; } /* expect: avoid-return-types-on-setters */
  static void set flag(bool v) {} /* expect: avoid-return-types-on-setters */
  external void set ext(int v); /* expect: avoid-return-types-on-setters */
}

mixin M {
  void set data(int v) {} /* expect: avoid-return-types-on-setters */
}
